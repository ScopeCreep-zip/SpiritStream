mod app;
mod chat_lifecycle;
mod config;
mod constants;
mod events;
mod handlers;
mod logging;
mod routes;
mod security;
mod state;
mod tasks;
mod util;

use governor::{Quota, RateLimiter};
use std::net::SocketAddr;
use std::num::NonZeroU32;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex as AsyncMutex;

use spiritstream_server::services::{
    prune_logs, ChatManager, DiscordWebhookService, EventSink, FFmpegDownloader, FFmpegHandler,
    OAuthConfig, OAuthService, ObsWebSocketHandler, ProfileManager, SettingsManager, ThemeManager,
};

use crate::config::ServerConfig;
use crate::events::EventBus;
use crate::state::AppState;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load .env file if present (ignore if missing)
    dotenvy::dotenv().ok();

    // Create core managers early (needed for config resolution)
    let data_dir = std::env::var("SPIRITSTREAM_DATA_DIR").unwrap_or_else(|_| "data".to_string());
    let app_data_dir = PathBuf::from(&data_dir);
    std::fs::create_dir_all(&app_data_dir)?;

    let profile_manager = Arc::new(ProfileManager::new(app_data_dir.clone()));
    let settings_manager = Arc::new(SettingsManager::new(app_data_dir.clone()));

    // Resolve all config from env vars + settings
    let config = ServerConfig::from_env(&profile_manager, &settings_manager).await?;

    // Load settings for FFmpeg path and log pruning
    let settings = settings_manager.load().ok();
    let custom_ffmpeg_path = settings.as_ref().and_then(|s| {
        if s.ffmpeg_path.is_empty() { None } else { Some(s.ffmpeg_path.clone()) }
    });
    if let Some(s) = settings.as_ref() {
        let _ = prune_logs(&config.log_dir, s.log_retention_days);
    }

    let ffmpeg_handler = Arc::new(FFmpegHandler::new_with_custom_path(
        config.app_data_dir.clone(),
        custom_ffmpeg_path,
    ));

    let event_bus = EventBus::new();
    logging::init_logger(&config.log_dir, event_bus.clone())?;

    // Log themes directory configuration
    let themes_path = PathBuf::from(&config.themes_dir);
    let themes_exist = themes_path.exists();
    let env_was_set = std::env::var("SPIRITSTREAM_THEMES_DIR").is_ok();
    log::info!(
        "Themes directory: {} (exists={themes_exist}, env_set={env_was_set})",
        config.themes_dir
    );
    if !themes_exist {
        log::warn!("Themes directory does not exist - custom themes may not load");
    }

    let theme_manager = Arc::new(ThemeManager::new(config.app_data_dir.clone(), themes_path));

    // Sync themes and verify
    log::info!("Starting theme sync from {:?} to user data", config.themes_dir);
    theme_manager.sync_project_themes();
    let synced_themes = theme_manager.list_themes();
    log::info!(
        "Theme sync complete. Available themes ({}): {:?}",
        synced_themes.len(),
        synced_themes.iter().map(|t| &t.id).collect::<Vec<_>>()
    );

    let theme_event_sink: Arc<dyn EventSink> = Arc::new(event_bus.clone());
    theme_manager.start_watcher(theme_event_sink);

    // Initialize rate limiter
    let rate_limiter = Arc::new(RateLimiter::direct(Quota::per_minute(
        NonZeroU32::new(config.rate_limit).unwrap_or(NonZeroU32::new(100).unwrap()),
    )));

    // Get home directory for path validation
    let home_dir = dirs_next::home_dir();

    // Initialize services
    let obs_handler = Arc::new(ObsWebSocketHandler::new(config.app_data_dir.clone()));
    let discord_service = Arc::new(DiscordWebhookService::new());
    let chat_event_sink: Arc<dyn EventSink> = Arc::new(event_bus.clone());
    let chat_manager = Arc::new(ChatManager::new(chat_event_sink, config.log_dir.clone()));
    let oauth_service = Arc::new(OAuthService::new(OAuthConfig::default()));

    let state = AppState {
        profile_manager,
        settings_manager,
        ffmpeg_handler,
        ffmpeg_downloader: Arc::new(AsyncMutex::new(FFmpegDownloader::new())),
        theme_manager,
        obs_handler,
        discord_service,
        chat_manager,
        oauth_service,
        event_bus,
        log_dir: config.log_dir,
        app_data_dir: config.app_data_dir,
        auth_token: config.auth_token,
        rate_limiter,
        active_profile_name: Arc::new(AsyncMutex::new(None)),
        active_profile_settings: Arc::new(AsyncMutex::new(None)),
        home_dir,
        is_localhost: config.is_localhost,
    };

    // Start background tasks
    tasks::start_background_tasks(state.clone()).await;

    // Build router
    let app = app::build_router(state.clone(), config.ui_enabled, &config.ui_dir);

    let address = SocketAddr::new(util::parse_host(&config.host), config.port);
    log::info!("SpiritStream backend listening on http://{address}");
    if state.auth_token.is_some() {
        log::info!("  Authentication: enabled");
    } else {
        log::info!("  Authentication: disabled (no token configured)");
    }
    if !config.is_localhost && state.auth_token.is_none() {
        log::warn!(
            "WARNING: Server bound to {} without authentication token. \
             Set SPIRITSTREAM_API_TOKEN for security.",
            config.host
        );
    }

    let listener = tokio::net::TcpListener::bind(address).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
