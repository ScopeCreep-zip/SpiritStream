use obws::Client;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex as TokioMutex, RwLock};
use tokio::task::JoinHandle;

use super::cascade::ObsCascadeDeps;
use super::types::{IntegrationDirection, ObsConfig, ObsState};

/// Manages OBS WebSocket connection and stream synchronization. The
/// type lives here; method impls are spread across `connection`,
/// `commands`, and `cascade` submodules so each concern is in its own
/// file but all hang off `impl super::ObsWebSocketHandler`.
pub struct ObsWebSocketHandler {
    pub(super) state: Arc<RwLock<ObsState>>,
    /// Shared via `Arc<Client>` so the per-connection OBS event listener can
    /// own a clone for the lifetime of its `client.events()` stream while the
    /// command path (`start_stream`/`stop_stream`) still drives the same socket.
    pub(super) client: Arc<RwLock<Option<Arc<Client>>>>,
    pub(super) config: Arc<RwLock<ObsConfig>>,
    /// The relay's RTMP ingest URL (e.g. `rtmp://127.0.0.1:1935/live`) derived
    /// from the active profile's input. On connect, OBS's stream service is
    /// pointed here (via `SetStreamServiceSettings`) so OBS pushes to the relay
    /// using an explicit IPv4 loopback — sidestepping the `localhost`→`::1`
    /// resolution that makes an IPv4-only ingest refuse OBS's RTMP connection.
    pub(super) ingest_url: Arc<RwLock<Option<String>>>,
    pub(super) shutdown_tx: broadcast::Sender<()>,
    pub(super) app_data_dir: std::path::PathBuf,
    /// Loop-prevention flag for SpiritStream → OBS triggers. When this side
    /// initiates an OBS state change (`start_stream` / `stop_stream`), the
    /// subsequent inbound state event would normally bounce back through the
    /// "OBS → SpiritStream" trigger path and cause a feedback loop. We set
    /// this flag before driving OBS, then the OBS event listener atomically
    /// clears it; the orchestration layer reads it via
    /// `consume_triggered_by_us` to know to skip the trigger.
    pub(super) triggered_by_us: Arc<std::sync::atomic::AtomicBool>,
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
    pub(super) cascade_deps: Arc<std::sync::RwLock<Option<ObsCascadeDeps>>>,
    /// I1: handle to the background poll task spawned in
    /// `start_event_listener`. Pre-I1 we spawned and dropped the join
    /// handle, so the task outlived the handler in unit tests + on
    /// reconnect cycles where the same handler started a fresh poll
    /// loop without cancelling the old one. `Drop` aborts the handle
    /// so the runtime reclaims the task immediately.
    pub(super) listener_handle: Arc<TokioMutex<Option<JoinHandle<()>>>>,
    /// Handle to the self-healing auto-connect supervisor spawned by
    /// `spawn_auto_connect` (started at profile activation when OBS
    /// integration is in use). Single owner: a fresh `spawn_auto_connect`
    /// aborts the prior loop before starting, and `disconnect` aborts it
    /// so a manual disconnect doesn't immediately reconnect. `Drop` aborts
    /// it so the supervisor can't outlive the handler.
    pub(super) auto_connect_handle: Arc<TokioMutex<Option<JoinHandle<()>>>>,
}

impl Drop for ObsWebSocketHandler {
    fn drop(&mut self) {
        // Best-effort cancellation. `try_lock` because Drop isn't
        // async and we don't want to block on lock contention from a
        // dying handler. Send shutdown FIRST so a non-locked poll
        // task can observe the broadcast.
        let _ = self.shutdown_tx.send(());
        if let Ok(mut guard) = self.listener_handle.try_lock() {
            if let Some(handle) = guard.take() {
                handle.abort();
            }
        }
        if let Ok(mut guard) = self.auto_connect_handle.try_lock() {
            if let Some(handle) = guard.take() {
                handle.abort();
            }
        }
    }
}

impl ObsWebSocketHandler {
    pub fn new(app_data_dir: std::path::PathBuf) -> Self {
        let (shutdown_tx, _) = broadcast::channel(1);
        Self {
            state: Arc::new(RwLock::new(ObsState::default())),
            client: Arc::new(RwLock::new(None)),
            config: Arc::new(RwLock::new(ObsConfig::default_config())),
            ingest_url: Arc::new(RwLock::new(None)),
            shutdown_tx,
            app_data_dir,
            triggered_by_us: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            cascade_deps: Arc::new(std::sync::RwLock::new(None)),
            listener_handle: Arc::new(TokioMutex::new(None)),
            auto_connect_handle: Arc::new(TokioMutex::new(None)),
        }
    }

    /// Install the cascade deps so the OBS event listener can run the
    /// OBS→SpiritStream trigger in core. Called once by
    /// `ServiceRegistry::build` after all services exist.
    pub fn set_cascade_deps(&self, deps: ObsCascadeDeps) {
        match self.cascade_deps.write() {
            Ok(mut guard) => *guard = Some(deps),
            Err(e) => {
                log::error!("obs cascade_deps write lock poisoned during set_cascade_deps: {e}")
            }
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

    pub async fn set_config(&self, config: ObsConfig) {
        let mut cfg = self.config.write().await;
        *cfg = config;
    }

    /// Store the relay's RTMP ingest URL so the next connect points OBS's
    /// stream service at it. Set from the active profile's input.
    pub async fn set_ingest_url(&self, url: Option<String>) {
        *self.ingest_url.write().await = url;
    }

    pub async fn get_state(&self) -> ObsState {
        self.state.read().await.clone()
    }

    pub async fn is_connected(&self) -> bool {
        let state = self.state.read().await;
        state.connection_status == super::types::ObsConnectionStatus::Connected
    }

    pub async fn get_direction(&self) -> IntegrationDirection {
        self.config.read().await.direction
    }

    pub async fn should_obs_trigger_spiritstream(&self) -> bool {
        let direction = self.get_direction().await;
        matches!(
            direction,
            IntegrationDirection::ObsToSpiritstream | IntegrationDirection::Bidirectional
        )
    }

    pub async fn should_spiritstream_trigger_obs(&self) -> bool {
        let direction = self.get_direction().await;
        matches!(
            direction,
            IntegrationDirection::SpiritstreamToObs | IntegrationDirection::Bidirectional
        )
    }
}
