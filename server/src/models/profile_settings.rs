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

fn default_chat_visibility_panel_collapsed() -> bool {
    true
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
// Chat Integration Settings
// ============================================================================

/// Chat integration settings
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatSettings {
    /// Twitch channel name
    #[serde(default)]
    pub twitch_channel: String,

    /// YouTube channel ID
    #[serde(default)]
    pub youtube_channel_id: String,

    /// Trovo channel ID (numeric)
    #[serde(default)]
    pub trovo_channel_id: String,

    /// Stripchat username
    #[serde(default)]
    pub stripchat_username: String,

    /// YouTube API key (optional if using OAuth)
    #[serde(default)]
    pub youtube_api_key: String,

    /// Allow sending to Twitch chat
    #[serde(default)]
    pub twitch_send_enabled: bool,

    /// Allow sending to YouTube chat
    #[serde(default)]
    pub youtube_send_enabled: bool,

    /// Allow sending to Trovo chat
    #[serde(default)]
    pub trovo_send_enabled: bool,

    /// Allow sending to Stripchat chat
    #[serde(default)]
    pub stripchat_send_enabled: bool,

    /// Send messages to all enabled platforms
    #[serde(default)]
    pub send_all_enabled: bool,

    /// Crosspost inbound chat messages to other platforms
    #[serde(default)]
    pub crosspost_enabled: bool,

    /// Use API key instead of OAuth for YouTube chat
    #[serde(default)]
    pub youtube_use_api_key: bool,

    /// Visible chat platform cards (empty = auto)
    #[serde(default)]
    pub visible_platforms: Vec<String>,

    /// Collapse the visibility panel by default
    #[serde(default = "default_chat_visibility_panel_collapsed")]
    pub visibility_panel_collapsed: bool,
}

impl Default for ChatSettings {
    fn default() -> Self {
        Self {
            twitch_channel: String::new(),
            youtube_channel_id: String::new(),
            trovo_channel_id: String::new(),
            stripchat_username: String::new(),
            youtube_api_key: String::new(),
            twitch_send_enabled: false,
            youtube_send_enabled: false,
            trovo_send_enabled: false,
            stripchat_send_enabled: false,
            send_all_enabled: true,
            crosspost_enabled: false,
            youtube_use_api_key: false,
            visible_platforms: Vec::new(),
            visibility_panel_collapsed: default_chat_visibility_panel_collapsed(),
        }
    }
}

// ============================================================================
// OAuth Settings (per-profile)
// ============================================================================

/// OAuth account + token data for a provider
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OAuthAccount {
    /// Access token for API calls
    #[serde(default)]
    pub access_token: String,

    /// Refresh token (if available)
    #[serde(default)]
    pub refresh_token: String,

    /// Token expiration timestamp (Unix epoch seconds)
    #[serde(default)]
    pub expires_at: i64,

    /// Provider user/channel ID
    #[serde(default)]
    pub user_id: String,

    /// Provider username/handle (if available)
    #[serde(default)]
    pub username: String,

    /// Provider display name (if available)
    #[serde(default)]
    pub display_name: String,
}

impl Default for OAuthAccount {
    fn default() -> Self {
        Self {
            access_token: String::new(),
            refresh_token: String::new(),
            expires_at: 0,
            user_id: String::new(),
            username: String::new(),
            display_name: String::new(),
        }
    }
}

/// OAuth configuration per profile
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OAuthSettings {
    #[serde(default)]
    pub twitch: OAuthAccount,
    #[serde(default)]
    pub youtube: OAuthAccount,
}

impl Default for OAuthSettings {
    fn default() -> Self {
        Self {
            twitch: OAuthAccount::default(),
            youtube: OAuthAccount::default(),
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
    /// Chat integration settings
    #[serde(default)]
    pub chat: ChatSettings,

    /// OAuth tokens + account info (per profile)
    #[serde(default)]
    pub oauth: OAuthSettings,
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
            chat: ChatSettings::default(),
            oauth: OAuthSettings::default(),
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
            && !self.backend.remote_enabled
            && !self.backend.ui_enabled
            && self.backend.host == default_backend_host()
            && self.backend.port == default_backend_port()
            && self.backend.token.is_empty()
            && self.obs.host == default_obs_host()
            && self.obs.port == default_obs_port()
            && self.obs.password.is_empty()
            && !self.obs.use_auth
            && self.obs.direction == ObsIntegrationDirection::default()
            && !self.obs.auto_connect
            && !self.discord.webhook_enabled
            && self.discord.webhook_url.is_empty()
            && self.discord.go_live_message == default_discord_go_live_message()
            && self.discord.cooldown_enabled == default_discord_cooldown_enabled()
            && self.discord.cooldown_seconds == default_discord_cooldown_seconds()
            && self.discord.image_path.is_empty()
            && self.chat.twitch_channel.is_empty()
            && self.chat.youtube_channel_id.is_empty()
            && self.chat.trovo_channel_id.is_empty()
            && self.chat.stripchat_username.is_empty()
            && self.chat.youtube_api_key.is_empty()
            && !self.chat.twitch_send_enabled
            && !self.chat.youtube_send_enabled
            && !self.chat.trovo_send_enabled
            && !self.chat.stripchat_send_enabled
            && self.chat.send_all_enabled
            && !self.chat.crosspost_enabled
            && !self.chat.youtube_use_api_key
            && self.chat.visible_platforms.is_empty()
            && self.chat.visibility_panel_collapsed == default_chat_visibility_panel_collapsed()
            && self.oauth.twitch.access_token.is_empty()
            && self.oauth.twitch.refresh_token.is_empty()
            && self.oauth.twitch.expires_at == 0
            && self.oauth.twitch.user_id.is_empty()
            && self.oauth.twitch.username.is_empty()
            && self.oauth.twitch.display_name.is_empty()
            && self.oauth.youtube.access_token.is_empty()
            && self.oauth.youtube.refresh_token.is_empty()
            && self.oauth.youtube.expires_at == 0
            && self.oauth.youtube.user_id.is_empty()
            && self.oauth.youtube.username.is_empty()
            && self.oauth.youtube.display_name.is_empty()
    }

    /// Merge legacy settings into this profile, only filling values that are still at defaults.
    /// Returns true if any fields were updated.
    pub fn merge_missing(&mut self, legacy: &ProfileSettings) -> bool {
        let defaults = ProfileSettings::default();
        let mut changed = false;

        if self.theme_id == defaults.theme_id && legacy.theme_id != defaults.theme_id {
            self.theme_id = legacy.theme_id.clone();
            changed = true;
        }
        if self.language == defaults.language && legacy.language != defaults.language {
            self.language = legacy.language.clone();
            changed = true;
        }
        if self.show_notifications == defaults.show_notifications
            && legacy.show_notifications != defaults.show_notifications
        {
            self.show_notifications = legacy.show_notifications;
            changed = true;
        }
        if self.encrypt_stream_keys == defaults.encrypt_stream_keys
            && legacy.encrypt_stream_keys != defaults.encrypt_stream_keys
        {
            self.encrypt_stream_keys = legacy.encrypt_stream_keys;
            changed = true;
        }

        if self.backend.remote_enabled == defaults.backend.remote_enabled
            && legacy.backend.remote_enabled != defaults.backend.remote_enabled
        {
            self.backend.remote_enabled = legacy.backend.remote_enabled;
            changed = true;
        }
        if self.backend.ui_enabled == defaults.backend.ui_enabled
            && legacy.backend.ui_enabled != defaults.backend.ui_enabled
        {
            self.backend.ui_enabled = legacy.backend.ui_enabled;
            changed = true;
        }
        if self.backend.host == defaults.backend.host && legacy.backend.host != defaults.backend.host {
            self.backend.host = legacy.backend.host.clone();
            changed = true;
        }
        if self.backend.port == defaults.backend.port && legacy.backend.port != defaults.backend.port {
            self.backend.port = legacy.backend.port;
            changed = true;
        }
        if self.backend.token.is_empty() && !legacy.backend.token.is_empty() {
            self.backend.token = legacy.backend.token.clone();
            changed = true;
        }

        if self.obs.host == defaults.obs.host && legacy.obs.host != defaults.obs.host {
            self.obs.host = legacy.obs.host.clone();
            changed = true;
        }
        if self.obs.port == defaults.obs.port && legacy.obs.port != defaults.obs.port {
            self.obs.port = legacy.obs.port;
            changed = true;
        }
        if self.obs.password.is_empty() && !legacy.obs.password.is_empty() {
            self.obs.password = legacy.obs.password.clone();
            changed = true;
        }
        if self.obs.use_auth == defaults.obs.use_auth && legacy.obs.use_auth != defaults.obs.use_auth {
            self.obs.use_auth = legacy.obs.use_auth;
            changed = true;
        }
        if self.obs.direction == defaults.obs.direction && legacy.obs.direction != defaults.obs.direction {
            self.obs.direction = legacy.obs.direction;
            changed = true;
        }
        if self.obs.auto_connect == defaults.obs.auto_connect
            && legacy.obs.auto_connect != defaults.obs.auto_connect
        {
            self.obs.auto_connect = legacy.obs.auto_connect;
            changed = true;
        }

        if self.discord.webhook_enabled == defaults.discord.webhook_enabled
            && legacy.discord.webhook_enabled != defaults.discord.webhook_enabled
        {
            self.discord.webhook_enabled = legacy.discord.webhook_enabled;
            changed = true;
        }
        if self.discord.webhook_url.is_empty() && !legacy.discord.webhook_url.is_empty() {
            self.discord.webhook_url = legacy.discord.webhook_url.clone();
            changed = true;
        }
        if self.discord.go_live_message == defaults.discord.go_live_message
            && legacy.discord.go_live_message != defaults.discord.go_live_message
        {
            self.discord.go_live_message = legacy.discord.go_live_message.clone();
            changed = true;
        }
        if self.discord.cooldown_enabled == defaults.discord.cooldown_enabled
            && legacy.discord.cooldown_enabled != defaults.discord.cooldown_enabled
        {
            self.discord.cooldown_enabled = legacy.discord.cooldown_enabled;
            changed = true;
        }
        if self.discord.cooldown_seconds == defaults.discord.cooldown_seconds
            && legacy.discord.cooldown_seconds != defaults.discord.cooldown_seconds
        {
            self.discord.cooldown_seconds = legacy.discord.cooldown_seconds;
            changed = true;
        }
        if self.discord.image_path.is_empty() && !legacy.discord.image_path.is_empty() {
            self.discord.image_path = legacy.discord.image_path.clone();
            changed = true;
        }

        if self.chat.twitch_channel.is_empty() && !legacy.chat.twitch_channel.is_empty() {
            self.chat.twitch_channel = legacy.chat.twitch_channel.clone();
            changed = true;
        }
        if self.chat.youtube_channel_id.is_empty() && !legacy.chat.youtube_channel_id.is_empty() {
            self.chat.youtube_channel_id = legacy.chat.youtube_channel_id.clone();
            changed = true;
        }
        if self.chat.trovo_channel_id.is_empty() && !legacy.chat.trovo_channel_id.is_empty() {
            self.chat.trovo_channel_id = legacy.chat.trovo_channel_id.clone();
            changed = true;
        }
        if self.chat.stripchat_username.is_empty() && !legacy.chat.stripchat_username.is_empty() {
            self.chat.stripchat_username = legacy.chat.stripchat_username.clone();
            changed = true;
        }
        if self.chat.youtube_api_key.is_empty() && !legacy.chat.youtube_api_key.is_empty() {
            self.chat.youtube_api_key = legacy.chat.youtube_api_key.clone();
            changed = true;
        }
        if self.chat.twitch_send_enabled == defaults.chat.twitch_send_enabled
            && legacy.chat.twitch_send_enabled != defaults.chat.twitch_send_enabled
        {
            self.chat.twitch_send_enabled = legacy.chat.twitch_send_enabled;
            changed = true;
        }
        if self.chat.youtube_send_enabled == defaults.chat.youtube_send_enabled
            && legacy.chat.youtube_send_enabled != defaults.chat.youtube_send_enabled
        {
            self.chat.youtube_send_enabled = legacy.chat.youtube_send_enabled;
            changed = true;
        }
        if self.chat.trovo_send_enabled == defaults.chat.trovo_send_enabled
            && legacy.chat.trovo_send_enabled != defaults.chat.trovo_send_enabled
        {
            self.chat.trovo_send_enabled = legacy.chat.trovo_send_enabled;
            changed = true;
        }
        if self.chat.stripchat_send_enabled == defaults.chat.stripchat_send_enabled
            && legacy.chat.stripchat_send_enabled != defaults.chat.stripchat_send_enabled
        {
            self.chat.stripchat_send_enabled = legacy.chat.stripchat_send_enabled;
            changed = true;
        }
        if self.chat.send_all_enabled == defaults.chat.send_all_enabled
            && legacy.chat.send_all_enabled != defaults.chat.send_all_enabled
        {
            self.chat.send_all_enabled = legacy.chat.send_all_enabled;
            changed = true;
        }
        if self.chat.crosspost_enabled == defaults.chat.crosspost_enabled
            && legacy.chat.crosspost_enabled != defaults.chat.crosspost_enabled
        {
            self.chat.crosspost_enabled = legacy.chat.crosspost_enabled;
            changed = true;
        }
        if self.chat.youtube_use_api_key == defaults.chat.youtube_use_api_key
            && legacy.chat.youtube_use_api_key != defaults.chat.youtube_use_api_key
        {
            self.chat.youtube_use_api_key = legacy.chat.youtube_use_api_key;
            changed = true;
        }
        if self.chat.visible_platforms.is_empty() && !legacy.chat.visible_platforms.is_empty() {
            self.chat.visible_platforms = legacy.chat.visible_platforms.clone();
            changed = true;
        }
        if self.chat.visibility_panel_collapsed == defaults.chat.visibility_panel_collapsed
            && legacy.chat.visibility_panel_collapsed != defaults.chat.visibility_panel_collapsed
        {
            self.chat.visibility_panel_collapsed = legacy.chat.visibility_panel_collapsed;
            changed = true;
        }

        if self.oauth.twitch.access_token.is_empty()
            && !legacy.oauth.twitch.access_token.is_empty()
        {
            self.oauth.twitch.access_token = legacy.oauth.twitch.access_token.clone();
            changed = true;
        }
        if self.oauth.twitch.refresh_token.is_empty()
            && !legacy.oauth.twitch.refresh_token.is_empty()
        {
            self.oauth.twitch.refresh_token = legacy.oauth.twitch.refresh_token.clone();
            changed = true;
        }
        if self.oauth.twitch.expires_at == 0 && legacy.oauth.twitch.expires_at > 0 {
            self.oauth.twitch.expires_at = legacy.oauth.twitch.expires_at;
            changed = true;
        }
        if self.oauth.twitch.user_id.is_empty() && !legacy.oauth.twitch.user_id.is_empty() {
            self.oauth.twitch.user_id = legacy.oauth.twitch.user_id.clone();
            changed = true;
        }
        if self.oauth.twitch.username.is_empty() && !legacy.oauth.twitch.username.is_empty() {
            self.oauth.twitch.username = legacy.oauth.twitch.username.clone();
            changed = true;
        }
        if self.oauth.twitch.display_name.is_empty()
            && !legacy.oauth.twitch.display_name.is_empty()
        {
            self.oauth.twitch.display_name = legacy.oauth.twitch.display_name.clone();
            changed = true;
        }

        if self.oauth.youtube.access_token.is_empty()
            && !legacy.oauth.youtube.access_token.is_empty()
        {
            self.oauth.youtube.access_token = legacy.oauth.youtube.access_token.clone();
            changed = true;
        }
        if self.oauth.youtube.refresh_token.is_empty()
            && !legacy.oauth.youtube.refresh_token.is_empty()
        {
            self.oauth.youtube.refresh_token = legacy.oauth.youtube.refresh_token.clone();
            changed = true;
        }
        if self.oauth.youtube.expires_at == 0 && legacy.oauth.youtube.expires_at > 0 {
            self.oauth.youtube.expires_at = legacy.oauth.youtube.expires_at;
            changed = true;
        }
        if self.oauth.youtube.user_id.is_empty() && !legacy.oauth.youtube.user_id.is_empty() {
            self.oauth.youtube.user_id = legacy.oauth.youtube.user_id.clone();
            changed = true;
        }
        if self.oauth.youtube.username.is_empty() && !legacy.oauth.youtube.username.is_empty() {
            self.oauth.youtube.username = legacy.oauth.youtube.username.clone();
            changed = true;
        }
        if self.oauth.youtube.display_name.is_empty()
            && !legacy.oauth.youtube.display_name.is_empty()
        {
            self.oauth.youtube.display_name = legacy.oauth.youtube.display_name.clone();
            changed = true;
        }

        changed
    }
}

