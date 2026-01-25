// OBS WebSocket Service
// Handles connection to OBS Studio via obs-websocket protocol

use obws::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};

use crate::services::{EventSink, Encryption};

// ============================================================================
// Types
// ============================================================================

/// OBS connection status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ObsConnectionStatus {
    Disconnected,
    Connecting,
    Connected,
    Error,
}

impl Default for ObsConnectionStatus {
    fn default() -> Self {
        Self::Disconnected
    }
}

/// OBS streaming status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ObsStreamStatus {
    Inactive,
    Starting,
    Active,
    Stopping,
    Unknown,
}

impl Default for ObsStreamStatus {
    fn default() -> Self {
        Self::Unknown
    }
}

/// Integration directionality - controls how stream state syncs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum IntegrationDirection {
    /// OBS controls SpiritStream (OBS start -> SpiritStream start)
    ObsToSpiritstream,
    /// SpiritStream controls OBS (SpiritStream start -> OBS start)
    SpiritstreamToObs,
    /// Bidirectional sync (either can trigger the other)
    Bidirectional,
    /// No automatic sync
    Disabled,
}

impl Default for IntegrationDirection {
    fn default() -> Self {
        Self::Disabled
    }
}

/// OBS WebSocket configuration (stored in settings)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObsConfig {
    /// WebSocket host (e.g., "localhost")
    pub host: String,
    /// WebSocket port (default: 4455 for OBS 28+)
    pub port: u16,
    /// Authentication password (encrypted at rest)
    pub password: String,
    /// Whether to use authentication
    pub use_auth: bool,
    /// Integration direction
    pub direction: IntegrationDirection,
    /// Auto-connect on startup
    pub auto_connect: bool,
}

impl ObsConfig {
    pub fn default_config() -> Self {
        Self {
            host: "localhost".to_string(),
            port: 4455,
            password: String::new(),
            use_auth: false,
            direction: IntegrationDirection::Disabled,
            auto_connect: false,
        }
    }
}

/// Current OBS state snapshot
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObsState {
    pub connection_status: ObsConnectionStatus,
    pub stream_status: ObsStreamStatus,
    pub error_message: Option<String>,
    pub obs_version: Option<String>,
    pub websocket_version: Option<String>,
}

// ============================================================================
// OBS WebSocket Handler
// ============================================================================

/// Manages OBS WebSocket connection and stream synchronization
pub struct ObsWebSocketHandler {
    state: Arc<RwLock<ObsState>>,
    client: Arc<RwLock<Option<Client>>>,
    config: Arc<RwLock<ObsConfig>>,
    shutdown_tx: broadcast::Sender<()>,
    app_data_dir: std::path::PathBuf,
}

impl ObsWebSocketHandler {
    /// Create a new OBS WebSocket handler
    pub fn new(app_data_dir: std::path::PathBuf) -> Self {
        let (shutdown_tx, _) = broadcast::channel(1);
        Self {
            state: Arc::new(RwLock::new(ObsState::default())),
            client: Arc::new(RwLock::new(None)),
            config: Arc::new(RwLock::new(ObsConfig::default_config())),
            shutdown_tx,
            app_data_dir,
        }
    }

    /// Update the configuration
    pub async fn set_config(&self, config: ObsConfig) {
        let mut cfg = self.config.write().await;
        *cfg = config;
    }

    /// Get the current configuration
    pub async fn get_config(&self) -> ObsConfig {
        self.config.read().await.clone()
    }

    /// Get the current state
    pub async fn get_state(&self) -> ObsState {
        self.state.read().await.clone()
    }

    /// Connect to OBS WebSocket
    pub async fn connect<E: EventSink + Send + Sync + Clone + 'static>(
        &self,
        event_sink: E,
    ) -> Result<(), String> {
        // Get config
        let config = self.config.read().await.clone();

        // Update state to connecting
        {
            let mut state = self.state.write().await;
            state.connection_status = ObsConnectionStatus::Connecting;
            state.error_message = None;
        }

        // Emit connection event
        event_sink.emit("obs://status", serde_json::json!({
            "status": "connecting",
            "host": config.host,
            "port": config.port
        }));

        // Decrypt password if encrypted
        log::debug!("[OBS] connect - use_auth: {}, password length: {}, password encrypted: {}",
            config.use_auth,
            config.password.len(),
            config.password.starts_with("ENC::"));

        let password = if config.use_auth && !config.password.is_empty() {
            if Encryption::is_stream_key_encrypted(&config.password) {
                let decrypted = Encryption::decrypt_stream_key(&config.password, &self.app_data_dir)
                    .map_err(|e| format!("Failed to decrypt OBS password: {e}"))?;
                log::debug!("[OBS] connect - decrypted password length: {}", decrypted.len());
                Some(decrypted)
            } else {
                log::debug!("[OBS] connect - using plain text password");
                Some(config.password.clone())
            }
        } else {
            log::debug!("[OBS] connect - not using authentication (use_auth={}, password_empty={})",
                config.use_auth, config.password.is_empty());
            None
        };

        // Connect to OBS - Client::connect(host, port, password)
        log::debug!("[OBS] connect - calling Client::connect with password: {}",
            if password.is_some() { "Some(...)" } else { "None" });
        let connect_result = Client::connect(&config.host, config.port, password).await;

        match connect_result {
            Ok(client) => {
                log::info!("Connected to OBS WebSocket at {}:{}", config.host, config.port);

                // Get OBS version info
                let version_info = client.general().version().await.ok();

                // Update state
                {
                    let mut state = self.state.write().await;
                    state.connection_status = ObsConnectionStatus::Connected;
                    state.error_message = None;
                    if let Some(ref info) = version_info {
                        state.obs_version = Some(info.obs_version.to_string());
                        state.websocket_version = Some(info.obs_web_socket_version.to_string());
                    }
                }

                // Get initial stream status
                if let Ok(stream_status) = client.streaming().status().await {
                    let mut state = self.state.write().await;
                    state.stream_status = if stream_status.active {
                        ObsStreamStatus::Active
                    } else {
                        ObsStreamStatus::Inactive
                    };
                }

                // Store client
                {
                    let mut client_guard = self.client.write().await;
                    *client_guard = Some(client);
                }

                // Emit connected event
                let state = self.state.read().await.clone();
                event_sink.emit("obs://status", serde_json::json!({
                    "status": "connected",
                    "obsVersion": state.obs_version,
                    "websocketVersion": state.websocket_version,
                    "streamStatus": state.stream_status
                }));

                // Start event listener
                self.start_event_listener(event_sink).await;

                Ok(())
            }
            Err(e) => {
                let error_msg = format!("Failed to connect to OBS: {e}");
                log::error!("{error_msg}");

                // Update state
                {
                    let mut state = self.state.write().await;
                    state.connection_status = ObsConnectionStatus::Error;
                    state.error_message = Some(error_msg.clone());
                }

                // Emit error event
                event_sink.emit("obs://status", serde_json::json!({
                    "status": "error",
                    "error": error_msg
                }));

                Err(error_msg)
            }
        }
    }

    /// Disconnect from OBS WebSocket
    pub async fn disconnect<E: EventSink>(&self, event_sink: E) -> Result<(), String> {
        // Signal shutdown to event listener
        let _ = self.shutdown_tx.send(());

        // Clear client
        {
            let mut client = self.client.write().await;
            *client = None;
        }

        // Update state
        {
            let mut state = self.state.write().await;
            state.connection_status = ObsConnectionStatus::Disconnected;
            state.stream_status = ObsStreamStatus::Unknown;
            state.error_message = None;
            state.obs_version = None;
            state.websocket_version = None;
        }

        log::info!("Disconnected from OBS WebSocket");

        event_sink.emit("obs://status", serde_json::json!({
            "status": "disconnected"
        }));

        Ok(())
    }

    /// Start streaming in OBS
    pub async fn start_stream(&self) -> Result<(), String> {
        let client = self.client.read().await;
        if let Some(ref client) = *client {
            client.streaming().start()
                .await
                .map_err(|e| format!("Failed to start OBS stream: {e}"))?;
            log::info!("Started OBS stream");
            Ok(())
        } else {
            Err("Not connected to OBS".to_string())
        }
    }

    /// Stop streaming in OBS
    pub async fn stop_stream(&self) -> Result<(), String> {
        let client = self.client.read().await;
        if let Some(ref client) = *client {
            client.streaming().stop()
                .await
                .map_err(|e| format!("Failed to stop OBS stream: {e}"))?;
            log::info!("Stopped OBS stream");
            Ok(())
        } else {
            Err("Not connected to OBS".to_string())
        }
    }

    /// Check if connected to OBS
    pub async fn is_connected(&self) -> bool {
        let state = self.state.read().await;
        state.connection_status == ObsConnectionStatus::Connected
    }

    /// Start listening for OBS events via polling
    async fn start_event_listener<E: EventSink + Send + Sync + Clone + 'static>(
        &self,
        event_sink: E,
    ) {
        let state = self.state.clone();
        let client = self.client.clone();
        let mut shutdown_rx = self.shutdown_tx.subscribe();

        tokio::spawn(async move {
            loop {
                // Check for shutdown
                if shutdown_rx.try_recv().is_ok() {
                    log::debug!("OBS event listener shutting down");
                    break;
                }

                // Get client and poll stream status
                let client_guard = client.read().await;
                if let Some(ref obs_client) = *client_guard {
                    let sink = event_sink.clone();

                    // Poll stream status
                    match obs_client.streaming().status().await {
                        Ok(stream_status) => {
                            let new_status = if stream_status.active {
                                ObsStreamStatus::Active
                            } else {
                                ObsStreamStatus::Inactive
                            };

                            let mut state_guard = state.write().await;
                            if state_guard.stream_status != new_status {
                                state_guard.stream_status = new_status;

                                sink.emit("obs://stream_state", serde_json::json!({
                                    "status": new_status,
                                    "active": stream_status.active
                                }));
                            }
                        }
                        Err(e) => {
                            log::debug!("Failed to poll OBS stream status: {e}");
                        }
                    }
                } else {
                    // No client, exit loop
                    break;
                }
                drop(client_guard);

                // Sleep before next poll
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            }
        });
    }

    /// Encrypt and save OBS password
    pub fn encrypt_password(&self, password: &str) -> Result<String, String> {
        if password.is_empty() {
            return Ok(String::new());
        }
        Encryption::encrypt_stream_key(password, &self.app_data_dir)
    }

    /// Get integration direction
    pub async fn get_direction(&self) -> IntegrationDirection {
        self.config.read().await.direction
    }

    /// Check if OBS should trigger SpiritStream
    pub async fn should_obs_trigger_spiritstream(&self) -> bool {
        let direction = self.get_direction().await;
        matches!(direction, IntegrationDirection::ObsToSpiritstream | IntegrationDirection::Bidirectional)
    }

    /// Check if SpiritStream should trigger OBS
    pub async fn should_spiritstream_trigger_obs(&self) -> bool {
        let direction = self.get_direction().await;
        matches!(direction, IntegrationDirection::SpiritstreamToObs | IntegrationDirection::Bidirectional)
    }
}
