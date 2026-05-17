// OBS WebSocket Service
// Handles connection to OBS Studio via obs-websocket protocol

use obws::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use ts_rs::TS;

use crate::errors::{CoreError, ValidationIssue};
use crate::services::{Encryption, EventSink, FFmpegHandler, ProfileManager, SettingsManager};

/// OBS→SpiritStream cascade delay before starting the relay. Mirrors
/// the 2-second stabilization window the frontend used to apply
/// client-side; centralising it here so any client (Tauri / Docker /
/// CLI) gets the same behaviour without re-implementing the delay.
const OBS_TRIGGER_DELAY_MS: u64 = 2000;

/// Wrap any `EventSink + Clone` into an `Arc<dyn EventSink>` so it can
/// flow into `FFmpegHandler::start_all`, which takes a trait-object
/// sink. The wrapper itself implements `EventSink` by delegating to
/// the inner clone.
struct EventSinkClone<E: EventSink + Send + Sync + 'static>(E);

impl<E: EventSink + Send + Sync + 'static> EventSink for EventSinkClone<E> {
    fn emit(&self, event: &str, payload: serde_json::Value) {
        self.0.emit(event, payload);
    }
}

/// Server-side cascade dependencies. Set once at startup by
/// `ServiceRegistry::build` after every service is constructed. When
/// present, `start_event_listener` runs the OBS→SpiritStream trigger
/// cascade in core (read active profile → check direction → call
/// `FFmpegHandler::start_all`) instead of relying on a frontend
/// observer to do it. Frontend keeps only display state.
#[derive(Clone)]
pub struct ObsCascadeDeps {
    pub profiles: Arc<ProfileManager>,
    pub settings: Arc<SettingsManager>,
    pub ffmpeg: Arc<FFmpegHandler>,
}

fn obs_not_connected() -> CoreError {
    CoreError::ValidationFailed {
        reasons: vec![ValidationIssue {
            code: "obs_not_connected".into(),
            message: "Not connected to OBS".into(),
            path: None,
        }],
    }
}

// ============================================================================
// Types
// ============================================================================

/// OBS connection status
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export, export_to = "../../../packages/types/src/generated/")]
pub enum ObsConnectionStatus {
    #[default]
    Disconnected,
    Connecting,
    Connected,
    Error,
}

/// OBS streaming status
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export, export_to = "../../../packages/types/src/generated/")]
pub enum ObsStreamStatus {
    Inactive,
    Starting,
    Active,
    Stopping,
    #[default]
    Unknown,
}

/// Integration directionality - controls how stream state syncs
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export, export_to = "../../../packages/types/src/generated/")]
pub enum IntegrationDirection {
    /// OBS controls SpiritStream (OBS start -> SpiritStream start)
    ObsToSpiritstream,
    /// SpiritStream controls OBS (SpiritStream start -> OBS start)
    SpiritstreamToObs,
    /// Bidirectional sync (either can trigger the other)
    Bidirectional,
    /// No automatic sync
    #[default]
    Disabled,
}

/// OBS WebSocket configuration (stored in settings)
#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/types/src/generated/")]
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
#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/types/src/generated/")]
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
    /// Loop-prevention flag for SpiritStream → OBS triggers. When this side
    /// initiates an OBS state change (`start_stream` / `stop_stream`), the
    /// subsequent inbound state event would normally bounce back through the
    /// "OBS → SpiritStream" trigger path and cause a feedback loop. We set
    /// this flag before driving OBS, then the OBS event listener atomically
    /// clears it; the orchestration layer reads it via
    /// `consume_triggered_by_us` to know to skip the trigger.
    triggered_by_us: Arc<std::sync::atomic::AtomicBool>,
    /// Cascade dependencies (`ProfileManager` / `SettingsManager` /
    /// `FFmpegHandler`). Set once at startup by
    /// `ServiceRegistry::build`. When present, the OBS event listener
    /// runs the OBS→SpiritStream trigger cascade in core — the
    /// frontend never decides "should we start streaming?", it just
    /// renders state changes.
    ///
    /// `std::sync::RwLock` (not `tokio::sync::RwLock`) so the setter
    /// can be called from synchronous code (`ServiceRegistry::build`)
    /// without an executor. Reads on the OBS poll loop are sparse
    /// enough that the lockless cost is irrelevant.
    cascade_deps: Arc<std::sync::RwLock<Option<ObsCascadeDeps>>>,
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
            triggered_by_us: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            cascade_deps: Arc::new(std::sync::RwLock::new(None)),
        }
    }

    /// Install the cascade deps so the OBS event listener can run the
    /// OBS→SpiritStream trigger in core. Called once by
    /// `ServiceRegistry::build` after all services exist.
    pub fn set_cascade_deps(&self, deps: ObsCascadeDeps) {
        if let Ok(mut guard) = self.cascade_deps.write() {
            *guard = Some(deps);
        }
    }

    /// Helper called by the `ObsTrigger` impl: drive OBS to start its
    /// stream when the active profile's direction allows SpiritStream
    /// → OBS triggering. Sets `triggered_by_us` first so the inbound
    /// state event doesn't bounce back through the OBS→SS cascade.
    async fn ss_trigger_obs(&self, start: bool) {
        let cascade = self.cascade_deps.read().ok().and_then(|g| g.clone());
        let Some(deps) = cascade else { return };
        let Ok(settings) = deps.settings.load() else {
            return;
        };
        let Some(active_name) = settings.last_profile.clone() else {
            return;
        };
        let profile = match deps
            .profiles
            .load_with_key_decryption(&active_name, None)
            .await
        {
            Ok(p) => p,
            Err(err) => {
                log::warn!("SS→OBS cascade: failed to load profile '{active_name}': {err}");
                return;
            }
        };
        let direction = profile.settings.obs.direction;
        let allowed = matches!(
            direction,
            crate::models::ObsIntegrationDirection::SpiritstreamToObs
                | crate::models::ObsIntegrationDirection::Bidirectional
        );
        if !allowed {
            return;
        }
        // Bail if not connected.
        let connected = matches!(
            self.state.read().await.connection_status,
            ObsConnectionStatus::Connected
        );
        if !connected {
            log::debug!("SS→OBS cascade: OBS not connected, skipping trigger");
            return;
        }
        let client_guard = self.client.read().await;
        let Some(ref client) = *client_guard else {
            return;
        };
        self.mark_triggered_by_us();
        let result = if start {
            client.streaming().start().await
        } else {
            client.streaming().stop().await
        };
        if let Err(err) = result {
            log::warn!(
                "SS→OBS cascade: OBS {} failed: {err}",
                if start { "start" } else { "stop" }
            );
        } else {
            log::info!(
                "SS→OBS cascade: OBS {} succeeded",
                if start { "started" } else { "stopped" }
            );
        }
    }

    /// Mark the next OBS state change as one we initiated, so the orchestration
    /// layer can skip its OBS→SpiritStream trigger path for that event.
    pub fn mark_triggered_by_us(&self) {
        self.triggered_by_us
            .store(true, std::sync::atomic::Ordering::SeqCst);
    }

    /// Atomically read-and-clear the triggered-by-us flag. The first observer
    /// of a triggered state event consumes the flag; subsequent observers see
    /// `false` and proceed with normal trigger evaluation.
    pub fn consume_triggered_by_us(&self) -> bool {
        self.triggered_by_us
            .swap(false, std::sync::atomic::Ordering::SeqCst)
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
        // Get config
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

        // Update state to connecting
        {
            let mut state = self.state.write().await;
            state.connection_status = ObsConnectionStatus::Connecting;
            state.error_message = None;
        }

        // Emit connection event
        event_sink.emit(
            "obs://status",
            serde_json::json!({
                "status": "connecting",
                "host": config.host,
                "port": config.port
            }),
        );

        // Decrypt password if encrypted
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

        // Connect to OBS - Client::connect(host, port, password)
        let connect_result = Client::connect(&config.host, config.port, password).await;

        match connect_result {
            Ok(client) => {
                log::info!(
                    "Connected to OBS WebSocket at {}:{}",
                    config.host,
                    config.port
                );

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
                event_sink.emit(
                    "obs://status",
                    serde_json::json!({
                        "status": "connected",
                        "obsVersion": state.obs_version,
                        "websocketVersion": state.websocket_version,
                        "streamStatus": state.stream_status
                    }),
                );

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

    /// Disconnect from OBS WebSocket
    pub async fn disconnect<E: EventSink>(&self, event_sink: E) -> Result<(), CoreError> {
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

        event_sink.emit(
            "obs://status",
            serde_json::json!({
                "status": "disconnected"
            }),
        );

        Ok(())
    }

    /// Start streaming in OBS
    pub async fn start_stream(&self) -> Result<(), CoreError> {
        let client = self.client.read().await;
        if let Some(ref client) = *client {
            // Flip the loop-prevention flag before driving OBS so the inbound
            // state event that follows doesn't trigger SpiritStream→OBS back.
            self.mark_triggered_by_us();
            client
                .streaming()
                .start()
                .await
                .map_err(|e| CoreError::Internal {
                    context: format!("Failed to start OBS stream: {e}"),
                })?;
            log::info!("Started OBS stream");
            Ok(())
        } else {
            Err(obs_not_connected())
        }
    }

    /// Stop streaming in OBS
    pub async fn stop_stream(&self) -> Result<(), CoreError> {
        let client = self.client.read().await;
        if let Some(ref client) = *client {
            self.mark_triggered_by_us();
            client
                .streaming()
                .stop()
                .await
                .map_err(|e| CoreError::Internal {
                    context: format!("Failed to stop OBS stream: {e}"),
                })?;
            log::info!("Stopped OBS stream");
            Ok(())
        } else {
            Err(obs_not_connected())
        }
    }

    /// Check if connected to OBS
    pub async fn is_connected(&self) -> bool {
        let state = self.state.read().await;
        state.connection_status == ObsConnectionStatus::Connected
    }

    /// Start listening for OBS events via polling.
    ///
    /// When `cascade_deps` is set, the listener runs the
    /// OBS→SpiritStream trigger cascade in core on every active↔inactive
    /// transition (read direction from active profile → optionally
    /// call `FFmpegHandler::start_all` / `stop_all`). The frontend
    /// only sees informational `obs://stream_state` events and an
    /// optional `stream_started_by_obs` / `stream_stopped_by_obs` —
    /// it never decides whether to start streaming.
    async fn start_event_listener<E: EventSink + Send + Sync + Clone + 'static>(
        &self,
        event_sink: E,
    ) {
        let state = self.state.clone();
        let client = self.client.clone();
        let triggered_by_us = self.triggered_by_us.clone();
        let cascade_deps = self.cascade_deps.clone();
        let mut shutdown_rx = self.shutdown_tx.subscribe();

        tokio::spawn(async move {
            loop {
                if shutdown_rx.try_recv().is_ok() {
                    log::debug!("OBS event listener shutting down");
                    break;
                }

                let client_guard = client.read().await;
                if let Some(ref obs_client) = *client_guard {
                    let sink = event_sink.clone();

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
                                drop(state_guard);

                                let was_self_triggered = triggered_by_us
                                    .swap(false, std::sync::atomic::Ordering::SeqCst);

                                sink.emit(
                                    "obs://stream_state",
                                    serde_json::json!({
                                        "status": new_status,
                                        "active": stream_status.active,
                                        "triggeredByUs": was_self_triggered,
                                    }),
                                );

                                // Cascade: when not self-triggered, ask
                                // the active profile's `obs.direction`
                                // whether to drive SpiritStream and run
                                // the action server-side.
                                if !was_self_triggered {
                                    let deps_snapshot =
                                        cascade_deps.read().ok().and_then(|g| g.clone());
                                    if let Some(deps) = deps_snapshot {
                                        Self::run_obs_to_ss_cascade(
                                            stream_status.active,
                                            deps,
                                            event_sink.clone(),
                                        )
                                        .await;
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            log::debug!("Failed to poll OBS stream status: {e}");
                        }
                    }
                } else {
                    break;
                }
                drop(client_guard);

                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            }
        });
    }

    /// Run the OBS→SpiritStream trigger cascade. Decides whether to
    /// start/stop SpiritStream based on the active profile's
    /// `obs.direction` and current FFmpeg state. Spawns a delayed task
    /// for the actual start so OBS has time to settle.
    async fn run_obs_to_ss_cascade<E: EventSink + Send + Sync + 'static>(
        obs_now_active: bool,
        deps: ObsCascadeDeps,
        event_sink: E,
    ) {
        // Read direction from the active profile.
        let Ok(settings) = deps.settings.load() else {
            return;
        };
        let Some(active_name) = settings.last_profile.clone() else {
            return;
        };
        let profile = match deps
            .profiles
            .load_with_key_decryption(&active_name, None)
            .await
        {
            Ok(p) => p,
            Err(err) => {
                log::warn!("OBS cascade: failed to load active profile '{active_name}': {err}");
                return;
            }
        };
        let direction = profile.settings.obs.direction;
        let allowed = matches!(
            direction,
            crate::models::ObsIntegrationDirection::ObsToSpiritstream
                | crate::models::ObsIntegrationDirection::Bidirectional
        );
        if !allowed {
            return;
        }

        if obs_now_active {
            // OBS started → start SpiritStream if not already.
            let already = deps.ffmpeg.active_count() > 0;
            if already {
                log::debug!("OBS cascade: SpiritStream already streaming, skipping start trigger");
                return;
            }
            let eligible: Vec<_> = profile
                .output_groups
                .iter()
                .filter(|g| !g.stream_targets.is_empty())
                .cloned()
                .collect();
            if eligible.is_empty() {
                log::warn!("OBS cascade: no eligible output groups, skipping start trigger");
                return;
            }
            let incoming_url = format!(
                "rtmp://{}:{}/{}",
                profile.input.bind_address, profile.input.port, profile.input.application
            );
            let count = eligible.len();
            // Mirror the frontend's old 2-second OBS-stabilize delay.
            let delay = std::time::Duration::from_millis(OBS_TRIGGER_DELAY_MS);
            let ffmpeg = deps.ffmpeg.clone();
            let sink_arc: Arc<dyn EventSink> = Arc::new(EventSinkClone(event_sink));
            tokio::spawn(async move {
                tokio::time::sleep(delay).await;
                match ffmpeg.start_all(&eligible, &incoming_url, sink_arc.clone()) {
                    Ok(_) => {
                        log::info!("OBS cascade: started SpiritStream ({count} groups)");
                        sink_arc.emit(
                            "stream_started_by_obs",
                            serde_json::json!({ "groupCount": count }),
                        );
                    }
                    Err(err) => {
                        log::error!("OBS cascade: failed to start SpiritStream: {err}");
                    }
                }
            });
        } else {
            // OBS stopped → stop SpiritStream if streaming.
            if deps.ffmpeg.active_count() == 0 {
                return;
            }
            match deps.ffmpeg.stop_all() {
                Ok(_) => {
                    log::info!("OBS cascade: stopped SpiritStream");
                    event_sink.emit("stream_stopped_by_obs", serde_json::json!({}));
                }
                Err(err) => {
                    log::error!("OBS cascade: failed to stop SpiritStream: {err}");
                }
            }
        }
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
    /// `Encryption::*` directly — see every CoreError flows through
    /// the single `ApiError(CoreError)` mapping on the HTTP side.
    pub async fn get_decrypted_config(&self) -> Result<ObsConfig, CoreError> {
        let mut config = self.config.read().await.clone();
        if !config.password.is_empty() && Encryption::is_stream_key_encrypted(&config.password) {
            config.password = Encryption::decrypt_stream_key(&config.password, &self.app_data_dir)?;
        }
        Ok(config)
    }

    /// Get integration direction
    pub async fn get_direction(&self) -> IntegrationDirection {
        self.config.read().await.direction
    }

    /// Check if OBS should trigger SpiritStream
    pub async fn should_obs_trigger_spiritstream(&self) -> bool {
        let direction = self.get_direction().await;
        matches!(
            direction,
            IntegrationDirection::ObsToSpiritstream | IntegrationDirection::Bidirectional
        )
    }

    /// Check if SpiritStream should trigger OBS
    pub async fn should_spiritstream_trigger_obs(&self) -> bool {
        let direction = self.get_direction().await;
        matches!(
            direction,
            IntegrationDirection::SpiritstreamToObs | IntegrationDirection::Bidirectional
        )
    }
}

/// `ObsTrigger` impl that delegates to `ObsWebSocketHandler`.
/// `FFmpegHandler` holds an `Arc<dyn ObsTrigger>` to keep its
/// dependency on the OBS service indirect — modular, mockable, and
/// reusable by future transports.
#[async_trait::async_trait]
impl crate::services::ObsTrigger for ObsWebSocketHandler {
    async fn trigger_start(&self) {
        self.ss_trigger_obs(true).await;
    }
    async fn trigger_stop(&self) {
        self.ss_trigger_obs(false).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn handler() -> ObsWebSocketHandler {
        ObsWebSocketHandler::new(std::path::PathBuf::from("/tmp/spiritstream-obs-test"))
    }

    /// `consume_triggered_by_us` is the loop-prevention seam: after
    /// SpiritStream drives OBS via `mark_triggered_by_us`, the inbound state
    /// event must see the flag set exactly once and then false on every
    /// subsequent read in the same cycle — otherwise a Bidirectional config
    /// loops forever.
    #[test]
    fn mark_then_consume_returns_true_then_false() {
        let h = handler();
        assert!(
            !h.consume_triggered_by_us(),
            "fresh handler must be unmarked"
        );
        h.mark_triggered_by_us();
        assert!(
            h.consume_triggered_by_us(),
            "first read after mark must observe true"
        );
        assert!(
            !h.consume_triggered_by_us(),
            "second read must atomically have cleared"
        );
    }

    /// Re-arming the flag must work for back-to-back triggers (the
    /// `start_stream` → `stop_stream` sequence within one session).
    #[test]
    fn mark_consume_mark_consume_rearms() {
        let h = handler();
        h.mark_triggered_by_us();
        assert!(h.consume_triggered_by_us());
        h.mark_triggered_by_us();
        assert!(h.consume_triggered_by_us());
        assert!(!h.consume_triggered_by_us());
    }

    /// Direction gating: each `IntegrationDirection` variant must permit only
    /// the documented trigger paths. Bidirectional permits both; Disabled
    /// permits neither. Single-direction variants permit exactly one path.
    #[tokio::test]
    async fn direction_gates_match_documented_transitions() {
        let h = handler();
        h.set_config(ObsConfig {
            host: "127.0.0.1".into(),
            port: 4455,
            password: String::new(),
            use_auth: false,
            direction: IntegrationDirection::Disabled,
            auto_connect: false,
        })
        .await;
        assert!(!h.should_obs_trigger_spiritstream().await);
        assert!(!h.should_spiritstream_trigger_obs().await);

        h.set_config(ObsConfig {
            direction: IntegrationDirection::ObsToSpiritstream,
            ..h.get_config().await
        })
        .await;
        assert!(h.should_obs_trigger_spiritstream().await);
        assert!(!h.should_spiritstream_trigger_obs().await);

        h.set_config(ObsConfig {
            direction: IntegrationDirection::SpiritstreamToObs,
            ..h.get_config().await
        })
        .await;
        assert!(!h.should_obs_trigger_spiritstream().await);
        assert!(h.should_spiritstream_trigger_obs().await);

        h.set_config(ObsConfig {
            direction: IntegrationDirection::Bidirectional,
            ..h.get_config().await
        })
        .await;
        assert!(h.should_obs_trigger_spiritstream().await);
        assert!(h.should_spiritstream_trigger_obs().await);
    }

    /// Loop scenario: simulate `start_stream` having flipped the flag (the
    /// real code path goes through OBS's RPC; this asserts the loop-prevention
    /// contract independently of the network IO). The orchestration layer's
    /// rule is: when an inbound state event lands with the flag set, that
    /// event was caused by us and must NOT bounce back through the trigger
    /// path; subsequent state events (no flag) proceed normally.
    #[test]
    fn loop_scenario_first_event_skips_trigger() {
        let h = handler();
        h.mark_triggered_by_us();
        // First inbound event after a SpiritStream→OBS trigger:
        let was_self = h.consume_triggered_by_us();
        assert!(
            was_self,
            "the post-mark event must be classified as self-triggered"
        );
        // Second event in the same chain (e.g., heartbeat / status poll):
        let was_self_again = h.consume_triggered_by_us();
        assert!(
            !was_self_again,
            "subsequent events must not be classified as self-triggered"
        );
    }
}
