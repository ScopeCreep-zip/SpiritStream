//! FFmpegHandler service — manages FFmpeg processes for streaming with
//! real-time stats.
//!
//! Split by concern:
//! - `validation`: encoding-config bounds (bitrate / fps / resolution /
//!   keyframe / container-codec compatibility) + ffmpeg error-string
//!   triage.
//! - `args`: ffmpeg argument builders + URL normalization / argument
//!   sanitization (stream-key redaction).
//! - `relay`: ProcessInfo / ActiveGroupConfig / RelayProcess types,
//!   port allocation, and the relay-process lifecycle helpers shared
//!   by `start` / `start_all` / `restart_group`.
//! - `lifecycle`: public `start` / `start_all` / `stop` / `stop_all` /
//!   `restart_group` / `retry_group` plus reconnection state reset.
//! - `stats`: per-group UDP bitrate meter + ffmpeg-stderr stats reader.

mod args;
mod lifecycle;
mod relay;
mod stats;
mod validation;

#[cfg(test)]
mod validate_tests;

pub use validation::{
    parse_bitrate_to_kbps, FPS_MAX, FPS_MIN, KEYFRAME_INTERVAL_MAX_SECS,
    KEYFRAME_INTERVAL_MIN_SECS, VIDEO_BITRATE_MAX_KBPS, VIDEO_BITRATE_MIN_KBPS,
};

use crate::errors::CoreError;
use crate::services::PlatformRegistry;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU16, AtomicUsize};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use relay::{ActiveGroupConfig, ProcessInfo, RelayProcess};

// =========================================================================
// Module-wide error helpers. All submodules import these via `super::*`.
// =========================================================================

pub(super) fn lock_poisoned<E: std::fmt::Display>(e: E) -> CoreError {
    CoreError::Internal {
        context: format!("Lock poisoned: {e}"),
    }
}

pub(super) fn ffmpeg_internal<S: Into<String>>(msg: S) -> CoreError {
    CoreError::Internal {
        context: msg.into(),
    }
}

// Windows: hide console windows for spawned processes
#[cfg(windows)]
pub(super) use std::os::windows::process::CommandExt;
#[cfg(windows)]
pub(super) const CREATE_NO_WINDOW: u32 = 0x08000000;

/// Placeholder used when `FFmpegLocator::discover` returns `None`.
/// `Command::new(FFMPEG_MISSING_SENTINEL)` fails with a clear OS-level
/// "ffmpeg: command not found" rather than panicking on an empty path.
const FFMPEG_MISSING_SENTINEL: &str = "ffmpeg";

// =========================================================================
// Reconnection
// =========================================================================

/// Reconnection configuration. Defaults exposed for tests / consumers via
/// `ReconnectionConfig::default()`; no runtime override surface today.
#[derive(Debug, Clone)]
pub(super) struct ReconnectionConfig {
    pub(super) max_retries: u32,
    pub(super) initial_delay_secs: u64,
    pub(super) max_delay_secs: u64,
}

impl Default for ReconnectionConfig {
    fn default() -> Self {
        Self {
            max_retries: 5,
            initial_delay_secs: 5,
            max_delay_secs: 120,
        }
    }
}

/// Per-group reconnection state. Carried in `ProcessInfo`; reset on
/// successful manual start; incremented + sleep-backoff on `retry_group`.
#[derive(Debug, Clone)]
pub(super) struct ReconnectionState {
    pub(super) attempt: u32,
    pub(super) last_attempt: Option<Instant>,
}

impl ReconnectionState {
    pub(super) fn new() -> Self {
        Self {
            attempt: 0,
            last_attempt: None,
        }
    }

    pub(super) fn increment(&mut self) {
        self.attempt += 1;
        self.last_attempt = Some(Instant::now());
    }

    pub(super) fn reset(&mut self) {
        self.attempt = 0;
        self.last_attempt = None;
    }

    pub(super) fn should_retry(&self, config: &ReconnectionConfig) -> bool {
        self.attempt < config.max_retries
    }

    pub(super) fn next_delay(&self, config: &ReconnectionConfig) -> Duration {
        // Exponential backoff: initial * 2^attempt, capped at max_delay.
        let delay_secs = config.initial_delay_secs * (1 << self.attempt.min(6));
        Duration::from_secs(delay_secs.min(config.max_delay_secs))
    }
}

// =========================================================================
// FFmpegHandler
// =========================================================================

/// Manages FFmpeg streaming processes. Method impls live in
/// `validation`, `args`, `relay`, `lifecycle`, and `stats` submodules —
/// each `impl super::FFmpegHandler { ... }` adds to the same surface.
pub struct FFmpegHandler {
    pub(in crate::services::ffmpeg_handler) ffmpeg_path: String,
    pub(in crate::services::ffmpeg_handler) processes:
        Arc<Mutex<HashMap<String, ProcessInfo>>>,
    pub(in crate::services::ffmpeg_handler) stopping_groups: Arc<Mutex<HashSet<String>>>,
    pub(in crate::services::ffmpeg_handler) disabled_targets: Arc<Mutex<HashSet<String>>>,
    pub(in crate::services::ffmpeg_handler) relay: Arc<Mutex<Option<RelayProcess>>>,
    pub(in crate::services::ffmpeg_handler) active_groups:
        Arc<Mutex<HashMap<String, ActiveGroupConfig>>>,
    /// Reference count for active groups using the relay. Prevents the
    /// race where the relay stops while groups are still active.
    pub(in crate::services::ffmpeg_handler) relay_refcount: Arc<AtomicUsize>,
    pub(in crate::services::ffmpeg_handler) platform_registry: PlatformRegistry,
    pub(in crate::services::ffmpeg_handler) reconnection_config: ReconnectionConfig,
    /// Port assignments for groups (`group_id` → `port_offset`). Simple
    /// sequential allocation instead of hash-based.
    pub(in crate::services::ffmpeg_handler) port_assignments: Arc<Mutex<HashMap<String, u16>>>,
    pub(in crate::services::ffmpeg_handler) next_port_offset: Arc<AtomicU16>,
    /// SpiritStream→OBS trigger handle. Set once at startup by
    /// `ServiceRegistry::build`. When present and the active profile's
    /// `obs.direction` allows the spiritstream→obs trigger, every
    /// successful `start_all` / `stop_all` fires a delayed OBS
    /// start/stop in core — the frontend never calls `api.obs.startStream`
    /// from its stream-control path.
    pub(in crate::services::ffmpeg_handler) obs_trigger:
        std::sync::RwLock<Option<Arc<dyn ObsTrigger>>>,
}

/// Trait the OBS handler implements for the SpiritStream→OBS cascade.
/// Kept as a trait (rather than a concrete `Arc<ObsWebSocketHandler>`)
/// so `FFmpegHandler` doesn't import the entire OBS module — tests
/// can plug a mock, and any future OBS-replacement transport satisfies
/// the same surface.
#[async_trait::async_trait]
pub trait ObsTrigger: Send + Sync {
    /// Trigger OBS to start streaming.
    async fn trigger_start(&self);
    /// Mirror of `trigger_start` for the stop path.
    async fn trigger_stop(&self);
}

impl FFmpegHandler {
    // Simplified sequential port allocation for relay fan-out. Since only
    // one profile is active at a time, simple sequential ports work:
    // each output group gets relay_port = BASE + (offset * 2) and
    // meter_port = BASE + (offset * 2) + 1.
    pub(super) const RELAY_HOST: &'static str = "localhost";
    pub(super) const RELAY_PORT_BASE: u16 = 20000;
    pub(super) const RELAY_TCP_OUT_QUERY: &'static str = "tcp_nodelay=1";
    pub(super) const RELAY_TCP_IN_QUERY: &'static str = "listen=1&tcp_nodelay=1";
    pub(super) const RELAY_RTMP_TIMEOUT_SECS: u32 = 604_800;
    pub(super) const RELAY_RTMP_TCP_NODELAY: &'static str = "1";
    pub(super) const RELAY_TEE_FIFO_OPTIONS: &'static str =
        "fifo_format=mpegts:queue_size=512:drop_pkts_on_overflow=1:attempt_recovery=1:recover_any_error=1";
    pub(super) const METER_HOST: &'static str = "127.0.0.1";
    pub(super) const METER_UDP_QUERY: &'static str = "pkt_size=1316";

    /// Create FFmpegHandler with optional custom FFmpeg path from settings.
    ///
    /// The path is resolved at construction by `FFmpegLocator::discover`,
    /// which applies the deterministic per-deployment order: explicit
    /// `custom_path` arg → `SPIRITSTREAM_FFMPEG_PATH` env (Tauri shell
    /// bundled sidecar) → `$PATH` lookup (Linux distro / Docker).
    /// If discovery fails the handler keeps the canonical missing-path
    /// string (`ffmpeg`) so spawn calls surface "FFmpeg missing" cleanly
    /// rather than crashing on empty.
    pub fn new_with_custom_path(
        _app_data_dir: std::path::PathBuf,
        custom_path: Option<String>,
    ) -> Result<Self, CoreError> {
        use crate::services::{FFmpegLocator, SettingsManager};
        let resolved = if let Some(p) = custom_path.as_deref().filter(|s| !s.is_empty()) {
            if std::path::Path::new(p).exists() {
                log::info!("Using custom FFmpeg path: {p}");
                Some(std::path::PathBuf::from(p))
            } else {
                log::warn!(
                    "Custom FFmpeg path {p} does not exist; refusing to fall through silently"
                );
                None
            }
        } else {
            FFmpegLocator::discover(None::<&SettingsManager>)
        };

        let ffmpeg_path = resolved
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| FFMPEG_MISSING_SENTINEL.to_string());

        Ok(Self {
            ffmpeg_path,
            processes: Arc::new(Mutex::new(HashMap::new())),
            stopping_groups: Arc::new(Mutex::new(HashSet::new())),
            disabled_targets: Arc::new(Mutex::new(HashSet::new())),
            relay: Arc::new(Mutex::new(None)),
            active_groups: Arc::new(Mutex::new(HashMap::new())),
            relay_refcount: Arc::new(AtomicUsize::new(0)),
            platform_registry: PlatformRegistry::new()?,
            reconnection_config: ReconnectionConfig::default(),
            port_assignments: Arc::new(Mutex::new(HashMap::new())),
            next_port_offset: Arc::new(AtomicU16::new(0)),
            obs_trigger: std::sync::RwLock::new(None),
        })
    }

    /// Install the SpiritStream→OBS trigger handle. Called once at
    /// startup by `ServiceRegistry::build` once the OBS handler exists.
    pub fn set_obs_trigger(&self, trigger: Arc<dyn ObsTrigger>) {
        if let Ok(mut guard) = self.obs_trigger.write() {
            *guard = Some(trigger);
        }
    }

    pub(super) fn fire_obs_trigger(&self, start: bool) {
        let trigger = match self.obs_trigger.read() {
            Ok(g) => g.clone(),
            Err(_) => None,
        };
        let Some(trigger) = trigger else { return };
        // The trigger runs after a small delay so SpiritStream can
        // settle its own pipeline first. Spawned on the runtime so the
        // synchronous `start_all` / `stop_all` callers don't block.
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(2000)).await;
            if start {
                trigger.trigger_start().await;
            } else {
                trigger.trigger_stop().await;
            }
        });
    }

    /// Active stream count.
    pub fn active_count(&self) -> usize {
        self.processes.lock().map(|procs| procs.len()).unwrap_or(0)
    }

    /// Check if a group is streaming.
    pub fn is_streaming(&self, group_id: &str) -> bool {
        self.processes
            .lock()
            .map(|procs| procs.contains_key(group_id))
            .unwrap_or(false)
    }

    /// List of active stream group IDs.
    pub fn get_active_group_ids(&self) -> Vec<String> {
        self.processes
            .lock()
            .map(|procs| procs.values().map(|info| info.group_id.clone()).collect())
            .unwrap_or_default()
    }

    /// Enable a specific stream target (removes from disabled set).
    pub fn enable_target(&self, target_id: &str) {
        let mut disabled = self.disabled_targets.lock().unwrap_or_else(|e| {
            log::warn!("Disabled targets mutex poisoned (enable_target), recovering: {e}");
            e.into_inner()
        });
        disabled.remove(target_id);
    }

    /// Disable a specific stream target (adds to disabled set).
    pub fn disable_target(&self, target_id: &str) {
        let mut disabled = self.disabled_targets.lock().unwrap_or_else(|e| {
            log::warn!("Disabled targets mutex poisoned (disable_target), recovering: {e}");
            e.into_inner()
        });
        disabled.insert(target_id.to_string());
    }

    /// Check if a target is currently disabled.
    pub fn is_target_disabled(&self, target_id: &str) -> bool {
        let disabled = self.disabled_targets.lock().unwrap_or_else(|e| {
            log::warn!("Disabled targets mutex poisoned (is_target_disabled), recovering: {e}");
            e.into_inner()
        });
        disabled.contains(target_id)
    }
}
