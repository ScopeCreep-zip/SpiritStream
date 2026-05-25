use obws::Client;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};

use super::cascade::ObsCascadeDeps;
use super::types::{IntegrationDirection, ObsConfig, ObsState};

/// Manages OBS WebSocket connection and stream synchronization. The
/// type lives here; method impls are spread across `connection`,
/// `commands`, and `cascade` submodules so each concern is in its own
/// file but all hang off `impl super::ObsWebSocketHandler`.
pub struct ObsWebSocketHandler {
    pub(super) state: Arc<RwLock<ObsState>>,
    pub(super) client: Arc<RwLock<Option<Client>>>,
    pub(super) config: Arc<RwLock<ObsConfig>>,
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
}

impl ObsWebSocketHandler {
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

    pub async fn get_config(&self) -> ObsConfig {
        self.config.read().await.clone()
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
