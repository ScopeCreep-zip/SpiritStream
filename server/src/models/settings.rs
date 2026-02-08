// Settings Model
// Application-wide configuration (global settings only)
// Profile-specific settings are now in ProfileSettings

use serde::{Deserialize, Serialize};

fn default_log_retention_days() -> u32 {
    30
}

/// OBS WebSocket integration direction
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
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
/// Profile-specific settings (theme, language, integrations) are stored in ProfileSettings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    // =========================================================================
    // GLOBAL SETTINGS (remain in settings.json)
    // =========================================================================

    /// App-level behavior: start minimized to tray
    #[serde(default)]
    pub start_minimized: bool,

    /// System-wide FFmpeg location
    #[serde(default)]
    pub ffmpeg_path: String,

    /// System-wide behavior: auto-download FFmpeg if missing
    #[serde(default = "default_true")]
    pub auto_download_ffmpeg: bool,

    /// App-wide log management: days to retain logs
    #[serde(default = "default_log_retention_days")]
    pub log_retention_days: u32,

    /// Tracks which profile to load on startup
    #[serde(default)]
    pub last_profile: Option<String>,
}

fn default_true() -> bool {
    true
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            // Global settings
            start_minimized: false,
            ffmpeg_path: String::new(),
            auto_download_ffmpeg: true,
            log_retention_days: default_log_retention_days(),
            last_profile: None,
        }
    }
}
