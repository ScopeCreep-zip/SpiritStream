// Settings Model
// Application-wide configuration

use serde::{Deserialize, Serialize};

fn default_log_retention_days() -> u32 {
    30
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

/// Application settings
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    // General
    pub language: String,
    pub start_minimized: bool,
    pub show_notifications: bool,

    // FFmpeg
    pub ffmpeg_path: String,
    pub auto_download_ffmpeg: bool,

    // Data & Privacy
    pub encrypt_stream_keys: bool,

    // Log retention
    #[serde(default = "default_log_retention_days")]
    pub log_retention_days: u32,

    // UI theme
    #[serde(default)]
    pub theme_id: String,

    // Local host server (HTTP/WS)
    #[serde(default)]
    pub backend_remote_enabled: bool,
    #[serde(default)]
    pub backend_ui_enabled: bool,
    #[serde(default = "default_backend_host")]
    pub backend_host: String,
    #[serde(default = "default_backend_port")]
    pub backend_port: u16,
    #[serde(default)]
    pub backend_token: String,

    // OBS WebSocket integration
    #[serde(default = "default_obs_host")]
    pub obs_host: String,
    #[serde(default = "default_obs_port")]
    pub obs_port: u16,
    #[serde(default)]
    pub obs_password: String,
    #[serde(default)]
    pub obs_use_auth: bool,
    #[serde(default)]
    pub obs_direction: ObsIntegrationDirection,
    #[serde(default)]
    pub obs_auto_connect: bool,

    // Last used profile
    pub last_profile: Option<String>,

    // Discord webhook integration
    #[serde(default)]
    pub discord_webhook_enabled: bool,
    #[serde(default)]
    pub discord_webhook_url: String,
    #[serde(default = "default_discord_go_live_message")]
    pub discord_go_live_message: String,
    #[serde(default = "default_discord_cooldown_enabled")]
    pub discord_cooldown_enabled: bool,
    #[serde(default = "default_discord_cooldown_seconds")]
    pub discord_cooldown_seconds: u32,
    #[serde(default)]
    pub discord_image_path: String,

    // Chat platform integration
    #[serde(default)]
    pub chat_twitch_channel: String,
    #[serde(default)]
    pub chat_twitch_oauth_token: String,
    #[serde(default)]
    pub chat_youtube_channel_id: String,
    #[serde(default)]
    pub chat_youtube_api_key: String,
    #[serde(default)]
    pub chat_auto_connect: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            language: "en".to_string(),
            start_minimized: false,
            show_notifications: true,
            ffmpeg_path: String::new(),
            auto_download_ffmpeg: true,
            encrypt_stream_keys: false,
            log_retention_days: default_log_retention_days(),
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
            last_profile: None,
            discord_webhook_enabled: false,
            discord_webhook_url: String::new(),
            discord_go_live_message: default_discord_go_live_message(),
            discord_cooldown_enabled: true,
            discord_cooldown_seconds: default_discord_cooldown_seconds(),
            discord_image_path: String::new(),
            chat_twitch_channel: String::new(),
            chat_twitch_oauth_token: String::new(),
            chat_youtube_channel_id: String::new(),
            chat_youtube_api_key: String::new(),
            chat_auto_connect: false,
        }
    }
}
