// Profile Settings Model
// Per-profile configuration for theme, integrations, and security settings

use serde::{Deserialize, Serialize};
use super::ObsIntegrationDirection;

// ============================================================================
// Default value functions
// ============================================================================

fn default_theme_id() -> String {
    "spirit-dark".to_string()
}

fn default_language() -> String {
    "en".to_string()
}

fn default_show_notifications() -> bool {
    true
}

fn default_encrypt_stream_keys() -> bool {
    true
}

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

fn default_discord_go_live_message() -> String {
    "**Stream is now live!** 🎮\n\nCome join the stream!".to_string()
}

fn default_discord_cooldown_enabled() -> bool {
    true
}

fn default_discord_cooldown_seconds() -> u32 {
    60
}

// ============================================================================
// Backend/Remote Access Settings
// ============================================================================

/// Backend server settings for remote access
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackendSettings {
    /// Enable remote access (non-localhost binding)
    #[serde(default)]
    pub remote_enabled: bool,

    /// Enable serving the web UI from the backend
    #[serde(default)]
    pub ui_enabled: bool,

    /// Host address to bind to
    #[serde(default = "default_backend_host")]
    pub host: String,

    /// Port to listen on
    #[serde(default = "default_backend_port")]
    pub port: u16,

    /// Authentication token for remote access
    #[serde(default)]
    pub token: String,
}

impl Default for BackendSettings {
    fn default() -> Self {
        Self {
            remote_enabled: false,
            ui_enabled: false,
            host: default_backend_host(),
            port: default_backend_port(),
            token: String::new(),
        }
    }
}

// ============================================================================
// OBS Integration Settings
// ============================================================================

/// OBS WebSocket integration settings
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObsSettings {
    /// OBS WebSocket host
    #[serde(default = "default_obs_host")]
    pub host: String,

    /// OBS WebSocket port
    #[serde(default = "default_obs_port")]
    pub port: u16,

    /// OBS WebSocket password (encrypted at rest)
    #[serde(default)]
    pub password: String,

    /// Whether to use authentication
    #[serde(default)]
    pub use_auth: bool,

    /// Integration direction (who controls whom)
    #[serde(default)]
    pub direction: ObsIntegrationDirection,

    /// Automatically connect on profile load
    #[serde(default)]
    pub auto_connect: bool,
}

impl Default for ObsSettings {
    fn default() -> Self {
        Self {
            host: default_obs_host(),
            port: default_obs_port(),
            password: String::new(),
            use_auth: false,
            direction: ObsIntegrationDirection::default(),
            auto_connect: false,
        }
    }
}

// ============================================================================
// Discord Integration Settings
// ============================================================================

/// Discord webhook integration settings
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscordSettings {
    /// Enable Discord webhook notifications
    #[serde(default)]
    pub webhook_enabled: bool,

    /// Discord webhook URL (encrypted at rest)
    #[serde(default)]
    pub webhook_url: String,

    /// Message to send when going live
    #[serde(default = "default_discord_go_live_message")]
    pub go_live_message: String,

    /// Enable cooldown between notifications
    #[serde(default = "default_discord_cooldown_enabled")]
    pub cooldown_enabled: bool,

    /// Cooldown duration in seconds
    #[serde(default = "default_discord_cooldown_seconds")]
    pub cooldown_seconds: u32,

    /// Path to image to include in notification
    #[serde(default)]
    pub image_path: String,
}

impl Default for DiscordSettings {
    fn default() -> Self {
        Self {
            webhook_enabled: false,
            webhook_url: String::new(),
            go_live_message: default_discord_go_live_message(),
            cooldown_enabled: default_discord_cooldown_enabled(),
            cooldown_seconds: default_discord_cooldown_seconds(),
            image_path: String::new(),
        }
    }
}

// ============================================================================
// Profile Settings (combines all per-profile settings)
// ============================================================================

/// Per-profile settings for UI, security, and integrations
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileSettings {
    // UI Settings
    /// Theme ID for this profile
    #[serde(default = "default_theme_id")]
    pub theme_id: String,

    /// Language code for this profile
    #[serde(default = "default_language")]
    pub language: String,

    /// Show desktop notifications
    #[serde(default = "default_show_notifications")]
    pub show_notifications: bool,

    // Security Settings
    /// Encrypt stream keys at rest for this profile
    #[serde(default = "default_encrypt_stream_keys")]
    pub encrypt_stream_keys: bool,

    // Integration Settings
    /// Backend/Remote access settings
    #[serde(default)]
    pub backend: BackendSettings,

    /// OBS WebSocket integration settings
    #[serde(default)]
    pub obs: ObsSettings,

    /// Discord webhook integration settings
    #[serde(default)]
    pub discord: DiscordSettings,
}

impl Default for ProfileSettings {
    fn default() -> Self {
        Self {
            theme_id: default_theme_id(),
            language: default_language(),
            show_notifications: default_show_notifications(),
            encrypt_stream_keys: default_encrypt_stream_keys(),
            backend: BackendSettings::default(),
            obs: ObsSettings::default(),
            discord: DiscordSettings::default(),
        }
    }
}

impl ProfileSettings {
    /// Check if these settings are at their defaults (for migration detection)
    pub fn is_default(&self) -> bool {
        // Check if all fields are at default values
        // This is used to detect profiles that need migration from global settings
        self.theme_id == default_theme_id()
            && self.language == default_language()
            && self.show_notifications == default_show_notifications()
            && self.encrypt_stream_keys == default_encrypt_stream_keys()
            && self.backend.remote_enabled == false
            && self.backend.ui_enabled == false
            && self.backend.host == default_backend_host()
            && self.backend.port == default_backend_port()
            && self.backend.token.is_empty()
            && self.obs.host == default_obs_host()
            && self.obs.port == default_obs_port()
            && self.obs.password.is_empty()
            && self.obs.use_auth == false
            && self.obs.direction == ObsIntegrationDirection::default()
            && self.obs.auto_connect == false
            && self.discord.webhook_enabled == false
            && self.discord.webhook_url.is_empty()
            && self.discord.go_live_message == default_discord_go_live_message()
            && self.discord.cooldown_enabled == default_discord_cooldown_enabled()
            && self.discord.cooldown_seconds == default_discord_cooldown_seconds()
            && self.discord.image_path.is_empty()
    }
}
