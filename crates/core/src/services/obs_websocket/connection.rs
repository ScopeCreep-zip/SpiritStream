use obws::Client;
use std::sync::Arc;

use crate::errors::CoreError;
use crate::services::{Encryption, EventSink};

use super::types::{ObsConfig, ObsConnectionStatus, ObsStreamStatus};

impl super::ObsWebSocketHandler {
    /// Connect with exponential-backoff auto-retry. Used by the
    /// startup auto-connect flow when the active profile's
    /// `obs.auto_connect` flag is on. Honors `shutdown_tx` so a manual
    /// `disconnect()` (or app shutdown) interrupts the retry loop
    /// immediately. Backend-side replacement for the frontend's
    /// previous `useObsEvents.ts` retry state machine — same backoff
    /// curve (200ms → 30s, multiplier 1.5).
    pub fn spawn_auto_connect<E: EventSink + Send + Sync + Clone + 'static>(
        self: Arc<Self>,
        event_sink: E,
    ) {
        const INITIAL_DELAY_MS: u64 = 2000;
        const MAX_DELAY_MS: u64 = 30000;
        const BACKOFF_MULT: f64 = 1.5;
        let mut shutdown_rx = self.shutdown_tx.subscribe();
        let handler = self.clone();
        tokio::spawn(async move {
            let mut delay_ms = INITIAL_DELAY_MS;
            loop {
                if shutdown_rx.try_recv().is_ok() {
                    log::debug!("OBS auto-connect loop interrupted by shutdown");
                    break;
                }
                let already = matches!(
                    handler.state.read().await.connection_status,
                    ObsConnectionStatus::Connected | ObsConnectionStatus::Connecting
                );
                if already {
                    break;
                }
                match handler.connect(event_sink.clone()).await {
                    Ok(()) => {
                        log::info!("OBS auto-connect: connected");
                        break;
                    }
                    Err(err) => {
                        log::info!(
                            "OBS auto-connect: attempt failed ({err}); retrying in {delay_ms}ms"
                        );
                        tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                        delay_ms =
                            ((delay_ms as f64) * BACKOFF_MULT).min(MAX_DELAY_MS as f64) as u64;
                    }
                }
            }
        });
    }

    pub async fn connect<E: EventSink + Send + Sync + Clone + 'static>(
        &self,
        event_sink: E,
    ) -> Result<(), CoreError> {
        let config = self.config.read().await.clone();

        // Guard against double-connect: if a previous `connect()` is still
        // mid-handshake (state == Connecting) or already finished (Connected),
        // bail out without re-emitting "connecting" or starting a second
        // handshake. The second call would otherwise emit a duplicate event
        // AND race the first call's `client_guard` write — fixing the latter
        // properly needs a cancellation token; for now we just prevent the
        // duplicate state transition.
        {
            let state_read = self.state.read().await;
            if matches!(
                state_read.connection_status,
                ObsConnectionStatus::Connecting | ObsConnectionStatus::Connected
            ) {
                return Ok(());
            }
        }

        {
            let mut state = self.state.write().await;
            state.connection_status = ObsConnectionStatus::Connecting;
            state.error_message = None;
        }

        event_sink.emit(
            "obs://status",
            serde_json::json!({
                "status": "connecting",
                "host": config.host,
                "port": config.port
            }),
        );

        let password = if config.use_auth && !config.password.is_empty() {
            if Encryption::is_stream_key_encrypted(&config.password) {
                Some(Encryption::decrypt_stream_key(
                    &config.password,
                    &self.app_data_dir,
                )?)
            } else {
                Some(config.password.clone())
            }
        } else {
            None
        };

        let connect_result = Client::connect(&config.host, config.port, password).await;

        match connect_result {
            Ok(client) => {
                log::info!(
                    "Connected to OBS WebSocket at {}:{}",
                    config.host,
                    config.port
                );

                let version_info = client.general().version().await.ok();

                {
                    let mut state = self.state.write().await;
                    state.connection_status = ObsConnectionStatus::Connected;
                    state.error_message = None;
                    if let Some(ref info) = version_info {
                        state.obs_version = Some(info.obs_version.to_string());
                        state.websocket_version = Some(info.obs_web_socket_version.to_string());
                    }
                }

                if let Ok(stream_status) = client.streaming().status().await {
                    let mut state = self.state.write().await;
                    state.stream_status = if stream_status.active {
                        ObsStreamStatus::Active
                    } else {
                        ObsStreamStatus::Inactive
                    };
                }

                {
                    let mut client_guard = self.client.write().await;
                    *client_guard = Some(client);
                }

                let state = self.state.read().await.clone();
                event_sink.emit(
                    "obs://status",
                    serde_json::json!({
                        "status": "connected",
                        "obsVersion": state.obs_version,
                        "websocketVersion": state.websocket_version,
                        "streamStatus": state.stream_status
                    }),
                );

                self.start_event_listener(event_sink).await;

                Ok(())
            }
            Err(e) => {
                let error_msg = format!("Failed to connect to OBS: {e}");
                log::error!("{error_msg}");

                {
                    let mut state = self.state.write().await;
                    state.connection_status = ObsConnectionStatus::Error;
                    state.error_message = Some(error_msg.clone());
                }

                event_sink.emit(
                    "obs://status",
                    serde_json::json!({
                        "status": "error",
                        "error": error_msg.clone(),
                    }),
                );

                Err(CoreError::Internal { context: error_msg })
            }
        }
    }

    pub async fn disconnect<E: EventSink>(&self, event_sink: E) -> Result<(), CoreError> {
        let _ = self.shutdown_tx.send(());

        {
            let mut client = self.client.write().await;
            *client = None;
        }

        {
            let mut state = self.state.write().await;
            state.connection_status = ObsConnectionStatus::Disconnected;
            state.stream_status = ObsStreamStatus::Unknown;
            state.error_message = None;
            state.obs_version = None;
            state.websocket_version = None;
        }

        log::info!("Disconnected from OBS WebSocket");

        event_sink.emit(
            "obs://status",
            serde_json::json!({
                "status": "disconnected"
            }),
        );

        Ok(())
    }

    /// Encrypt and save OBS password
    pub fn encrypt_password(&self, password: &str) -> Result<String, CoreError> {
        if password.is_empty() {
            return Ok(String::new());
        }
        Encryption::encrypt_stream_key(password, &self.app_data_dir)
    }

    /// Return the current OBS config with the stored password decrypted for the
    /// caller. Used by the typed REST handler so transports never touch
    /// `Encryption::*` directly — every CoreError flows through the single
    /// `ApiError(CoreError)` mapping on the HTTP side.
    pub async fn get_decrypted_config(&self) -> Result<ObsConfig, CoreError> {
        let mut config = self.config.read().await.clone();
        if !config.password.is_empty() && Encryption::is_stream_key_encrypted(&config.password) {
            config.password = Encryption::decrypt_stream_key(&config.password, &self.app_data_dir)?;
        }
        Ok(config)
    }
}
