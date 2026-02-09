// Settings Model
// Application-wide configuration (global settings only)
// Profile-specific settings are now in ProfileSettings

use serde::{Deserialize, Serialize};
use super::{ProfileSettings, BackendSettings, ObsSettings, DiscordSettings};

fn default_log_retention_days() -> u32 {
    30
}

// Legacy default functions (kept for migration compatibility)
fn default_backend_host() -> String {
    "127.0.0.1".to_string()
}

fn default_backend_port() -> u16 {
    8008
}

fn default_obs_host() -> String {
    "localhost".to_string()
}

fn default_obs_port() -> u16 {
    4455
}

fn default_discord_cooldown_enabled() -> bool {
    true
}

fn default_discord_cooldown_seconds() -> u32 {
    60
}

fn default_discord_go_live_message() -> String {
    "**Stream is now live!** 🎮\n\nCome join the stream!".to_string()
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
/// Legacy fields are kept for migration but not serialized when saving.
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

    // =========================================================================
    // LEGACY FIELDS (read for migration, not written back)
    // These have moved to ProfileSettings but are kept here for backward
    // compatibility when reading old settings.json files.
    // =========================================================================

    #[serde(default, skip_serializing)]
    pub language: String,

    #[serde(default, skip_serializing)]
    pub show_notifications: bool,

    #[serde(default, skip_serializing)]
    pub encrypt_stream_keys: bool,

    #[serde(default, skip_serializing)]
    pub theme_id: String,

    // Legacy backend settings
    #[serde(default, skip_serializing)]
    pub backend_remote_enabled: bool,

    #[serde(default, skip_serializing)]
    pub backend_ui_enabled: bool,

    #[serde(default = "default_backend_host", skip_serializing)]
    pub backend_host: String,

    #[serde(default = "default_backend_port", skip_serializing)]
    pub backend_port: u16,

    #[serde(default, skip_serializing)]
    pub backend_token: String,

    // Legacy OBS settings
    #[serde(default = "default_obs_host", skip_serializing)]
    pub obs_host: String,

    #[serde(default = "default_obs_port", skip_serializing)]
    pub obs_port: u16,

    #[serde(default, skip_serializing)]
    pub obs_password: String,

    #[serde(default, skip_serializing)]
    pub obs_use_auth: bool,

    #[serde(default, skip_serializing)]
    pub obs_direction: ObsIntegrationDirection,

    #[serde(default, skip_serializing)]
    pub obs_auto_connect: bool,

    // Legacy Discord settings
    #[serde(default, skip_serializing)]
    pub discord_webhook_enabled: bool,

    #[serde(default, skip_serializing)]
    pub discord_webhook_url: String,

    #[serde(default = "default_discord_go_live_message", skip_serializing)]
    pub discord_go_live_message: String,

    #[serde(default = "default_discord_cooldown_enabled", skip_serializing)]
    pub discord_cooldown_enabled: bool,

    #[serde(default = "default_discord_cooldown_seconds", skip_serializing)]
    pub discord_cooldown_seconds: u32,

    #[serde(default, skip_serializing)]
    pub discord_image_path: String,
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

            // Legacy fields (defaults for migration)
            language: "en".to_string(),
            show_notifications: true,
            encrypt_stream_keys: true,
            theme_id: "spirit-dark".to_string(),
            backend_remote_enabled: false,
            backend_ui_enabled: false,
            backend_host: default_backend_host(),
            backend_port: default_backend_port(),
            backend_token: String::new(),
            obs_host: default_obs_host(),
            obs_port: default_obs_port(),
            obs_password: String::new(),
            obs_use_auth: false,
            obs_direction: ObsIntegrationDirection::default(),
            obs_auto_connect: false,
            discord_webhook_enabled: false,
            discord_webhook_url: String::new(),
            discord_go_live_message: default_discord_go_live_message(),
            discord_cooldown_enabled: default_discord_cooldown_enabled(),
            discord_cooldown_seconds: default_discord_cooldown_seconds(),
            discord_image_path: String::new(),
        }
    }
}

impl Settings {
    /// Check if this settings file has legacy profile-specific fields that indicate
    /// user configuration that should be migrated to a profile.
    ///
    /// We specifically check for fields that indicate the user has customized something:
    /// - Non-empty passwords, tokens, URLs, paths (user must have entered these)
    /// - Non-default host/port values (user changed connection settings)
    /// - Enabled features like OBS auth or auto-connect
    /// - Theme or language different from defaults
    pub fn has_legacy_profile_settings(&self) -> bool {
        // Check for user-configured values (not just defaults)
        // Theme or language explicitly set
        (!self.theme_id.is_empty() && self.theme_id != "spirit-dark")
            || (!self.language.is_empty() && self.language != "en")
            // Backend configuration (non-empty token indicates user setup)
            || !self.backend_token.is_empty()
            || self.backend_remote_enabled
            || self.backend_ui_enabled
            // OBS configuration (non-empty password or auth enabled indicates user setup)
            || !self.obs_password.is_empty()
            || self.obs_use_auth
            || self.obs_auto_connect
            || self.obs_direction != ObsIntegrationDirection::default()
            || self.obs_host != default_obs_host()
            || self.obs_port != default_obs_port()
            // Discord configuration (non-empty webhook URL indicates user setup)
            || !self.discord_webhook_url.is_empty()
            || self.discord_webhook_enabled
            || !self.discord_image_path.is_empty()
            || self.discord_go_live_message != default_discord_go_live_message()
    }

    /// Extract legacy profile settings for migration to a profile
    pub fn to_legacy_profile_settings(&self) -> ProfileSettings {
        let mut settings = ProfileSettings::default();

        settings.theme_id = if self.theme_id.is_empty() {
            "spirit-dark".to_string()
        } else {
            self.theme_id.clone()
        };
        settings.language = if self.language.is_empty() {
            "en".to_string()
        } else {
            self.language.clone()
        };
        settings.show_notifications = self.show_notifications;
        settings.encrypt_stream_keys = self.encrypt_stream_keys;
        settings.backend = BackendSettings {
            remote_enabled: self.backend_remote_enabled,
            ui_enabled: self.backend_ui_enabled,
            host: self.backend_host.clone(),
            port: self.backend_port,
            token: self.backend_token.clone(),
        };
        settings.obs = ObsSettings {
            host: self.obs_host.clone(),
            port: self.obs_port,
            password: self.obs_password.clone(),
            use_auth: self.obs_use_auth,
            direction: self.obs_direction,
            auto_connect: self.obs_auto_connect,
        };
        settings.discord = DiscordSettings {
            webhook_enabled: self.discord_webhook_enabled,
            webhook_url: self.discord_webhook_url.clone(),
            go_live_message: self.discord_go_live_message.clone(),
            cooldown_enabled: self.discord_cooldown_enabled,
            cooldown_seconds: self.discord_cooldown_seconds,
            image_path: self.discord_image_path.clone(),
        };

        settings
    }
}
