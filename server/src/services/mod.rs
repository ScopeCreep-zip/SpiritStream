// SpiritStream Services
// Business logic layer

mod profile_manager;
mod ffmpeg_handler;
mod ffmpeg_downloader;
mod encryption;
mod settings_manager;
mod theme_manager;
mod embedded_themes;
mod platform_registry;
mod log_manager;
#[cfg(feature = "chat")]
mod chat_manager;
#[cfg(feature = "chat")]
pub mod chat;
mod events;
mod stats_reader;
mod path_validator;
#[cfg(feature = "obs")]
mod obs_websocket;
#[cfg(feature = "discord")]
mod discord_webhook;
mod oauth;

pub use profile_manager::ProfileManager;
pub use ffmpeg_handler::FFmpegHandler;
pub use ffmpeg_downloader::{FFmpegDownloader, DownloadProgress, FFmpegVersionInfo, DownloadError};
pub use encryption::{Encryption, RotationReport};
pub use settings_manager::SettingsManager;
pub use theme_manager::ThemeManager;
pub use embedded_themes::{get_embedded_theme_tokens, get_embedded_theme_list, is_embedded_theme};
pub use platform_registry::{PlatformRegistry, PlatformConfig, StreamKeyPlacement};
pub use log_manager::{prune_logs, read_recent_logs};
#[cfg(feature = "chat")]
pub use chat_manager::ChatManager;
pub use events::{EventSink, NoopEventSink, emit_event};
pub use path_validator::{validate_path_within, validate_path_within_any, validate_extension, sanitize_filename};
#[cfg(feature = "obs")]
pub use obs_websocket::{ObsWebSocketHandler, ObsConfig, ObsState, ObsConnectionStatus, ObsStreamStatus, IntegrationDirection};
#[cfg(feature = "discord")]
pub use discord_webhook::{DiscordWebhookService, WebhookResult};
pub use oauth::{OAuthService, OAuthConfig, OAuthTokens, OAuthFlowResult, OAuthUserInfo, OAuthCompleteResult, OAuthProvider, OAuthCallback, OAuthCallbackServer, TwitchUser, YouTubeChannel};
