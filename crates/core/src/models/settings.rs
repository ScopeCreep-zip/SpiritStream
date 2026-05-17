// Settings Model
// Application-wide configuration (global settings only).
// Profile-specific settings live in `ProfileSettings`.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

fn default_log_retention_days() -> u32 {
    30
}

/// OBS WebSocket integration direction
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export, export_to = "../../../packages/types/src/generated/")]
pub enum ObsIntegrationDirection {
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

/// Application settings (global, app-wide settings)
///
/// Global settings — the small set of app-level knobs that stay in
/// `settings.json`. Profile-specific values (theme, language, OBS,
/// Discord, backend, chat) live in `ProfileSettings` and travel with
/// the profile, not here.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/types/src/generated/")]
pub struct Settings {
    /// App-level behavior: start minimized to tray.
    #[serde(default)]
    pub start_minimized: bool,

    /// Operator override for the FFmpeg binary location. Empty string
    /// means "use the discovery order in `FFmpegLocator::discover`"
    /// (bundled sidecar → `$PATH`).
    #[serde(default)]
    pub ffmpeg_path: String,

    /// App-wide log management: days to retain logs (1..=365).
    #[serde(default = "default_log_retention_days")]
    pub log_retention_days: u32,

    /// Tracks which profile to load on startup.
    #[serde(default)]
    pub last_profile: Option<String>,

    /// Opt-in error reporting. **Default off.** When true,
    /// crash/error payloads are scrubbed (PII stripped, paths redacted)
    /// and POSTed to `error_reporting_endpoint`. There is no
    /// first-party telemetry server; self-hosters point at their own
    /// collector.
    #[serde(default)]
    pub error_reporting_enabled: bool,

    /// Endpoint URL when `error_reporting_enabled` is on.
    /// Empty by default — the user must paste their own collector URL
    /// before reports are sent, even after enabling.
    #[serde(default)]
    pub error_reporting_endpoint: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            start_minimized: false,
            ffmpeg_path: String::new(),
            log_retention_days: default_log_retention_days(),
            last_profile: None,
            error_reporting_enabled: false,
            error_reporting_endpoint: String::new(),
        }
    }
}
