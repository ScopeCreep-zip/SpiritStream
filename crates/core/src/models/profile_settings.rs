// Profile Settings Model
// Per-profile configuration for theme, integrations, and security settings

use super::ObsIntegrationDirection;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

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
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/types/src/generated/")]
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
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/types/src/generated/")]
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
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/types/src/generated/")]
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
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/types/src/generated/")]
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

    /// Kick channel name (also used as the broadcaster handle for
    /// chatroom-id lookup at activation time).
    #[serde(default)]
    pub kick_channel: String,

    /// TikTok username (without the leading `@`). Read-only —
    /// TikTok rejects third-party chat send.
    #[serde(default)]
    pub tiktok_username: String,

    /// Facebook Live video id for the current broadcast. Identity-
    /// revealing — connecting binds chat to the streamer's real-name
    /// Facebook account; the UI's connect path gates this behind an
    /// explicit confirm-token acknowledgement.
    #[serde(default)]
    pub facebook_live_video_id: String,

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

    /// Allow sending to Kick chat (requires OAuth + chat:write scope)
    #[serde(default)]
    pub kick_send_enabled: bool,

    /// Send messages to all enabled platforms
    #[serde(default)]
    pub send_all_enabled: bool,

    /// Crosspost inbound chat messages to other platforms
    #[serde(default)]
    pub crosspost_enabled: bool,

    /// Turn on follower-only chat when connecting (safety-wizard
    /// setting). Applied via Twitch Helix at connect time — requires
    /// the `moderator:manage:chat_settings` scope; other platforms
    /// surface a `follower_only_unsupported` event instead of silently
    /// ignoring the flag.
    #[serde(default)]
    pub follower_only_default: bool,

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
            kick_channel: String::new(),
            tiktok_username: String::new(),
            facebook_live_video_id: String::new(),
            youtube_api_key: String::new(),
            twitch_send_enabled: false,
            youtube_send_enabled: false,
            trovo_send_enabled: false,
            kick_send_enabled: false,
            send_all_enabled: true,
            crosspost_enabled: false,
            follower_only_default: false,
            youtube_use_api_key: false,
            visible_platforms: Vec::new(),
            visibility_panel_collapsed: default_chat_visibility_panel_collapsed(),
        }
    }
}

// ============================================================================
// OAuth Settings (per-profile)
// ============================================================================

/// Public-facing OAuth account status returned to the frontend. Strips
/// secrets (access_token / refresh_token / expires_at) — the wire shape
/// is "is the user signed in, and what's their display identity?", not
/// "give me their credentials". When `logged_in == false` the other
/// fields are empty strings (matching the `OAuthAccount` storage
/// pattern), so consumers gate everything on `logged_in`.
#[derive(Debug, Clone, Serialize, Deserialize, Default, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/types/src/generated/")]
pub struct OAuthAccountStatus {
    pub logged_in: bool,
    #[serde(default)]
    pub user_id: String,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub display_name: String,
}

/// OAuth account + token data for a provider
#[derive(Debug, Clone, Serialize, Deserialize, Default, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/types/src/generated/")]
pub struct OAuthAccount {
    /// Access token for API calls
    #[serde(default)]
    pub access_token: String,

    /// Refresh token (if available)
    #[serde(default)]
    pub refresh_token: String,

    /// Token expiration timestamp (Unix epoch seconds). Emitted as a JSON
    /// `number` so the JS side gets `Number` (safe through year ~285,000,000 AD)
    /// rather than `bigint`, which `JSON.stringify` can't serialize.
    #[serde(default)]
    #[ts(type = "number")]
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

/// OAuth configuration per profile
#[derive(Debug, Clone, Serialize, Deserialize, Default, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/types/src/generated/")]
pub struct OAuthSettings {
    #[serde(default)]
    pub twitch: OAuthAccount,
    #[serde(default)]
    pub youtube: OAuthAccount,
    #[serde(default)]
    pub kick: OAuthAccount,
    /// Facebook Page Access Token + user identity. Same shape as
    /// the other providers; `access_token` is used as the Graph API
    /// bearer for both reading live comments and sending them.
    #[serde(default)]
    pub facebook: OAuthAccount,
}

// ============================================================================
// Profile Settings (combines all per-profile settings)
// ============================================================================

/// Per-profile settings for UI, security, and integrations
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/types/src/generated/")]
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
