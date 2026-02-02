// Settings Model
// Application-wide configuration (global settings only)
// Profile-specific settings are now in ProfileSettings

use serde::{Deserialize, Serialize};
use super::{ProfileSettings, BackendSettings, ObsSettings, DiscordSettings, ChatSettings};

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
    "**Stream is now live!** ðŸŽ®\n\nCome join the stream!".to_string()
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
/// Profile-specific settings (theme, language, integrations) have been moved to ProfileSettings.
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
    // GLOBAL OAUTH TOKENS (app-wide, not per-profile)
    // =========================================================================

    // Twitch OAuth account (from "Login with Twitch")
    #[serde(default)]
    pub twitch_oauth_access_token: String,
    #[serde(default)]
    pub twitch_oauth_refresh_token: String,
    #[serde(default)]
    pub twitch_oauth_expires_at: i64,
    #[serde(default)]
    pub twitch_oauth_user_id: String,
    #[serde(default)]
    pub twitch_oauth_username: String,
    #[serde(default)]
    pub twitch_oauth_display_name: String,

    // YouTube OAuth account (from "Sign in with Google")
    #[serde(default)]
    pub youtube_oauth_access_token: String,
    #[serde(default)]
    pub youtube_oauth_refresh_token: String,
    #[serde(default)]
    pub youtube_oauth_expires_at: i64,
    #[serde(default)]
    pub youtube_oauth_channel_id: String,
    #[serde(default)]
    pub youtube_oauth_channel_name: String,


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

    // Legacy chat settings
    #[serde(default, skip_serializing)]
    pub chat_twitch_channel: String,
    #[serde(default, skip_serializing)]
    pub chat_youtube_channel_id: String,
    #[serde(default, skip_serializing)]
    pub chat_youtube_api_key: String,
    #[serde(default, skip_serializing)]
    pub chat_twitch_send_enabled: bool,
    #[serde(default, skip_serializing)]
    pub chat_youtube_send_enabled: bool,
    #[serde(default, skip_serializing)]
    pub chat_send_all_enabled: bool,
    #[serde(default, skip_serializing)]
    pub chat_crosspost_enabled: bool,
    #[serde(default, skip_serializing)]
    pub youtube_use_api_key: bool,
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

            // Global OAuth tokens
            twitch_oauth_access_token: String::new(),
            twitch_oauth_refresh_token: String::new(),
            twitch_oauth_expires_at: 0,
            twitch_oauth_user_id: String::new(),
            twitch_oauth_username: String::new(),
            twitch_oauth_display_name: String::new(),

            youtube_oauth_access_token: String::new(),
            youtube_oauth_refresh_token: String::new(),
            youtube_oauth_expires_at: 0,
            youtube_oauth_channel_id: String::new(),
            youtube_oauth_channel_name: String::new(),

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
            chat_twitch_channel: String::new(),
            chat_youtube_channel_id: String::new(),
            chat_youtube_api_key: String::new(),
            chat_twitch_send_enabled: false,
            chat_youtube_send_enabled: false,
            chat_send_all_enabled: true,
            chat_crosspost_enabled: false,
            youtube_use_api_key: false,
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
            // Chat configuration (channel or send toggles indicate user setup)
            || !self.chat_twitch_channel.is_empty()
            || !self.chat_youtube_channel_id.is_empty()
            || !self.chat_youtube_api_key.is_empty()
            || self.chat_twitch_send_enabled
            || self.chat_youtube_send_enabled
            || self.chat_crosspost_enabled
            || self.youtube_use_api_key
    }

    /// Extract legacy profile settings for migration to a profile
    pub fn to_legacy_profile_settings(&self) -> ProfileSettings {
        ProfileSettings {
            theme_id: if self.theme_id.is_empty() {
                "spirit-dark".to_string()
            } else {
                self.theme_id.clone()
            },
            language: if self.language.is_empty() {
                "en".to_string()
            } else {
                self.language.clone()
            },
            show_notifications: self.show_notifications,
            encrypt_stream_keys: self.encrypt_stream_keys,
            backend: BackendSettings {
                remote_enabled: self.backend_remote_enabled,
                ui_enabled: self.backend_ui_enabled,
                host: self.backend_host.clone(),
                port: self.backend_port,
                token: self.backend_token.clone(),
            },
            obs: ObsSettings {
                host: self.obs_host.clone(),
                port: self.obs_port,
                password: self.obs_password.clone(),
                use_auth: self.obs_use_auth,
                direction: self.obs_direction,
                auto_connect: self.obs_auto_connect,
            },
            discord: DiscordSettings {
                webhook_enabled: self.discord_webhook_enabled,
                webhook_url: self.discord_webhook_url.clone(),
                go_live_message: self.discord_go_live_message.clone(),
                cooldown_enabled: self.discord_cooldown_enabled,
                cooldown_seconds: self.discord_cooldown_seconds,
                image_path: self.discord_image_path.clone(),
            },
            chat: ChatSettings {
                twitch_channel: self.chat_twitch_channel.clone(),
                youtube_channel_id: self.chat_youtube_channel_id.clone(),
                youtube_api_key: self.chat_youtube_api_key.clone(),
                twitch_send_enabled: self.chat_twitch_send_enabled,
                youtube_send_enabled: self.chat_youtube_send_enabled,
                send_all_enabled: self.chat_send_all_enabled,
                crosspost_enabled: self.chat_crosspost_enabled,
                youtube_use_api_key: self.youtube_use_api_key,
            },
        }
    }
}
