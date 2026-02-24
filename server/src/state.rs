use governor::{
    clock::DefaultClock,
    state::{InMemoryState, NotKeyed},
    RateLimiter,
};
use serde::Serialize;
use serde_json::Value;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex as AsyncMutex;

use crate::events::EventBus;
use spiritstream_server::models::{ObsIntegrationDirection, Profile, ProfileSettings};
use spiritstream_server::services::{
    ChatManager, DiscordWebhookService, FFmpegDownloader, FFmpegHandler,
    OAuthService, ObsConfig, ObsWebSocketHandler,
    ProfileManager, SettingsManager, ThemeManager,
};

#[derive(Clone)]
pub(crate) struct AppState {
    pub profile_manager: Arc<ProfileManager>,
    pub settings_manager: Arc<SettingsManager>,
    pub ffmpeg_handler: Arc<FFmpegHandler>,
    pub ffmpeg_downloader: Arc<AsyncMutex<FFmpegDownloader>>,
    pub theme_manager: Arc<ThemeManager>,
    pub obs_handler: Arc<ObsWebSocketHandler>,
    pub discord_service: Arc<DiscordWebhookService>,
    pub chat_manager: Arc<ChatManager>,
    pub oauth_service: Arc<OAuthService>,
    pub event_bus: EventBus,
    pub log_dir: PathBuf,
    pub app_data_dir: PathBuf,
    pub auth_token: Option<String>,
    pub rate_limiter: Arc<RateLimiter<NotKeyed, InMemoryState, DefaultClock>>,
    pub active_profile_name: Arc<AsyncMutex<Option<String>>>,
    pub active_profile_settings: Arc<AsyncMutex<Option<ProfileSettings>>>,
    /// Allowed export directories for path validation
    pub home_dir: Option<PathBuf>,
    /// True when server is bound to loopback address (127.0.0.1 / ::1)
    pub is_localhost: bool,
}

#[derive(Serialize)]
pub(crate) struct InvokeResponse {
    pub ok: bool,
    pub data: Option<Value>,
    pub error: Option<String>,
}

pub(crate) struct FreshOAuthToken {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: i64,
    pub refreshed: bool,
}

/// Ensure an OAuth token is fresh, refreshing it via the OAuth service if expired.
/// Returns token details and whether a refresh occurred.
/// Adds a 5-minute buffer so we refresh tokens that will expire within the next 5 minutes.
pub(crate) async fn ensure_fresh_oauth_token(
    provider: &str,
    access_token: &str,
    refresh_token: &str,
    expires_at: i64,
    oauth_service: &OAuthService,
) -> Result<FreshOAuthToken, String> {
    if access_token.is_empty() {
        return Err(format!("No {} OAuth token available", provider));
    }

    // Check if token is expired or will expire within 5 minutes
    let now = chrono::Utc::now().timestamp();
    let needs_refresh = expires_at > 0 && now >= (expires_at - 300);

    if !needs_refresh {
        return Ok(FreshOAuthToken {
            access_token: access_token.to_string(),
            refresh_token: None,
            expires_at,
            refreshed: false,
        });
    }

    if refresh_token.is_empty() {
        return Err(format!("{} token expired and no refresh token available", provider));
    }

    log::info!("{} OAuth token expired (expired {}s ago), refreshing...", provider, now - expires_at);

    let tokens = oauth_service.refresh_token(provider, refresh_token).await?;
    let new_expires_at = tokens.expires_in.map(|s| now + s as i64).unwrap_or(0);
    log::info!(
        "{} OAuth token refreshed successfully (new expiry in {}s)",
        provider,
        tokens.expires_in.unwrap_or(0)
    );

    Ok(FreshOAuthToken {
        access_token: tokens.access_token,
        refresh_token: tokens.refresh_token,
        expires_at: new_expires_at,
        refreshed: true,
    })
}

/// Apply a refreshed OAuth token to profile settings and persist to disk.
/// Returns the access token (refreshed or original).
pub(crate) async fn apply_and_persist_oauth_refresh(
    state: &AppState,
    provider: &str,
    fresh: &FreshOAuthToken,
    profile_settings: &mut ProfileSettings,
) -> String {
    if fresh.refreshed {
        let account = match provider {
            "twitch" => &mut profile_settings.oauth.twitch,
            "youtube" => &mut profile_settings.oauth.youtube,
            _ => return fresh.access_token.clone(),
        };
        account.access_token = fresh.access_token.clone();
        if let Some(rt) = &fresh.refresh_token {
            account.refresh_token = rt.clone();
        }
        account.expires_at = fresh.expires_at;
        if let Err(err) = persist_active_profile_settings(state, profile_settings.clone()).await {
            log::warn!("Failed to persist {} OAuth refresh: {err}", provider);
        }
    }
    fresh.access_token.clone()
}

pub(crate) async fn set_active_profile(state: &AppState, profile: &Profile) {
    {
        let mut guard = state.active_profile_name.lock().await;
        *guard = Some(profile.name.clone());
    }
    {
        let mut guard = state.active_profile_settings.lock().await;
        *guard = Some(profile.settings.clone());
    }

    state
        .chat_manager
        .update_profile_chat_settings(profile.settings.chat.clone())
        .await;

    let obs_settings = &profile.settings.obs;
    let direction = match obs_settings.direction {
        ObsIntegrationDirection::ObsToSpiritstream => {
            spiritstream_server::services::IntegrationDirection::ObsToSpiritstream
        }
        ObsIntegrationDirection::SpiritstreamToObs => {
            spiritstream_server::services::IntegrationDirection::SpiritstreamToObs
        }
        ObsIntegrationDirection::Bidirectional => {
            spiritstream_server::services::IntegrationDirection::Bidirectional
        }
        ObsIntegrationDirection::Disabled => {
            spiritstream_server::services::IntegrationDirection::Disabled
        }
    };

    let obs_config = ObsConfig {
        host: obs_settings.host.clone(),
        port: obs_settings.port,
        password: obs_settings.password.clone(),
        use_auth: obs_settings.use_auth,
        direction,
        auto_connect: obs_settings.auto_connect,
    };
    state.obs_handler.set_config(obs_config).await;
}

pub(crate) async fn get_active_profile_name(state: &AppState) -> Option<String> {
    let guard = state.active_profile_name.lock().await;
    guard.clone()
}

pub(crate) async fn get_active_profile_settings(state: &AppState) -> Option<ProfileSettings> {
    let guard = state.active_profile_settings.lock().await;
    guard.clone()
}

pub(crate) async fn set_active_profile_settings_only(state: &AppState, settings: ProfileSettings) {
    let mut guard = state.active_profile_settings.lock().await;
    *guard = Some(settings);
}

pub(crate) async fn persist_active_profile_settings(state: &AppState, settings: ProfileSettings) -> Result<(), String> {
    let name = get_active_profile_name(state)
        .await
        .ok_or_else(|| "No active profile loaded".to_string())?;

    // Update in-memory settings immediately so UI can reflect the change
    set_active_profile_settings_only(state, settings.clone()).await;

    let mut profile = state
        .profile_manager
        .load_with_key_decryption(&name, None)
        .await?;
    profile.settings = settings;

    state
        .profile_manager
        .save_with_key_encryption(&profile, None)
        .await?;

    Ok(())
}

pub(crate) async fn update_profile_oauth_account(
    state: &AppState,
    provider: &str,
    access_token: String,
    refresh_token: Option<String>,
    expires_at: i64,
    user_info: &spiritstream_server::services::OAuthUserInfo,
) -> Result<(), String> {
    let mut profile_settings = get_active_profile_settings(state)
        .await
        .ok_or_else(|| "No active profile loaded".to_string())?;

    match provider {
        "twitch" => {
            profile_settings.oauth.twitch.access_token = access_token;
            if let Some(rt) = refresh_token {
                profile_settings.oauth.twitch.refresh_token = rt;
            }
            profile_settings.oauth.twitch.expires_at = expires_at;
            profile_settings.oauth.twitch.user_id = user_info.user_id.clone();
            profile_settings.oauth.twitch.username = user_info.username.clone();
            profile_settings.oauth.twitch.display_name = user_info.display_name.clone();
        }
        "youtube" => {
            profile_settings.oauth.youtube.access_token = access_token;
            if let Some(rt) = refresh_token {
                profile_settings.oauth.youtube.refresh_token = rt;
            }
            profile_settings.oauth.youtube.expires_at = expires_at;
            profile_settings.oauth.youtube.user_id = user_info.user_id.clone();
            profile_settings.oauth.youtube.username = user_info.username.clone();
            profile_settings.oauth.youtube.display_name = user_info.display_name.clone();
        }
        _ => return Err(format!("Unknown provider: {provider}")),
    }

    persist_active_profile_settings(state, profile_settings).await
}

pub(crate) async fn clear_profile_oauth_account(state: &AppState, provider: &str) -> Result<(), String> {
    let mut profile_settings = get_active_profile_settings(state)
        .await
        .ok_or_else(|| "No active profile loaded".to_string())?;

    match provider {
        "twitch" => {
            profile_settings.oauth.twitch = Default::default();
        }
        "youtube" => {
            profile_settings.oauth.youtube = Default::default();
        }
        _ => return Err(format!("Unknown provider: {provider}")),
    }

    persist_active_profile_settings(state, profile_settings).await
}
