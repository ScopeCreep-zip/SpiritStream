use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use chrono::{DateTime, Duration, Local, Timelike};
use serde_json::json;
use tokio::sync::broadcast;

use spiritstream_core::models::{
    ChatConfig, ChatCredentials, ChatPlatform, ChatSettings, Profile, ProfileSettings, TwitchAuth,
    YouTubeAuth,
};
use spiritstream_core::services::{ChatManager, EventSink, OAuthService};

use crate::events::EventBus;
use crate::AppState;

pub(crate) struct FreshOAuthToken {
    pub(crate) access_token: String,
    pub(crate) refresh_token: Option<String>,
    pub(crate) expires_at: i64,
    pub(crate) refreshed: bool,
}

/// Ensure an OAuth token is fresh, refreshing it via the OAuth service if expired.
/// Returns token details and whether a refresh occurred.
/// Adds a 5-minute buffer so we refresh tokens that will expire within the next 5 minutes.
///
/// Failure modes return structured `CoreError` variants:
/// - Missing access token / refresh token → `Unauthorized` (caller prompts re-login)
/// - Provider refresh call failure → propagated from `OAuthService::refresh_token`
///   (either `Unauthorized` for 4xx or `NetworkError` for 5xx / transport errors)
pub(crate) async fn ensure_fresh_oauth_token(
    provider: &str,
    access_token: &str,
    refresh_token: &str,
    expires_at: i64,
    oauth_service: &OAuthService,
) -> Result<FreshOAuthToken, spiritstream_core::CoreError> {
    if access_token.is_empty() {
        log::warn!("No {provider} OAuth token available");
        return Err(spiritstream_core::CoreError::Unauthorized);
    }

    // Check if token is expired or will expire within 5 minutes.
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
        log::warn!("{provider} token expired and no refresh token available");
        return Err(spiritstream_core::CoreError::Unauthorized);
    }

    log::info!(
        "{} OAuth token expired (expired {}s ago), refreshing...",
        provider,
        now - expires_at
    );

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

pub(crate) fn build_hour_keys(start: DateTime<Local>, end: DateTime<Local>) -> Vec<String> {
    let mut keys = Vec::new();

    let start_hour = start
        .with_minute(0)
        .and_then(|d| d.with_second(0))
        .and_then(|d| d.with_nanosecond(0))
        .unwrap_or(start);
    let end_hour = end
        .with_minute(0)
        .and_then(|d| d.with_second(0))
        .and_then(|d| d.with_nanosecond(0))
        .unwrap_or(end);

    let mut cursor = start_hour;
    while cursor <= end_hour {
        keys.push(cursor.format("%Y%m%d-%H").to_string());
        cursor += Duration::hours(1);
    }

    keys
}

/// Persist the transport-level active-profile snapshot and re-emit the
/// Hydrate session-scoped transport state (`active_profile_name`,
/// `active_profile_settings`, `active_profile_pii`) and push the
/// anonymous-mode policy into `ChatManager` after a profile activates.
/// The `profile_activated` event is emitted by `ProfileActivationService`
/// itself before this runs, so both transports see identical bus shape
/// without re-emission here.
pub(crate) async fn set_active_profile(state: &AppState, profile: &Profile) {
    {
        let mut guard = state.active_profile_name.lock().await;
        *guard = Some(profile.name.clone());
    }
    {
        let mut guard = state.active_profile_settings.lock().await;
        *guard = Some(profile.settings.clone());
    }
    {
        let mut guard = state.active_profile_pii.lock().await;
        *guard = Some((profile.pii_blocklist.clone(), profile.pii_fuzzy));
    }
    // Push the anonymous-mode policy into ChatManager so
    // subsequent inbound chat messages get pseudonymised before they
    // reach the log writer or the event stream.
    state
        .chat_manager
        .set_anonymous_policy(profile.anonymous_logging, profile.anonymous_salt.clone())
        .await;
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

pub(crate) async fn persist_active_profile_settings(
    state: &AppState,
    settings: ProfileSettings,
) -> Result<(), spiritstream_core::CoreError> {
    let name = get_active_profile_name(state)
        .await
        .ok_or(spiritstream_core::CoreError::NoActiveProfile)?;

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
    user_info: &spiritstream_core::services::OAuthUserInfo,
) -> Result<(), spiritstream_core::CoreError> {
    let mut profile_settings = get_active_profile_settings(state)
        .await
        .ok_or(spiritstream_core::CoreError::NoActiveProfile)?;

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
        "kick" => {
            profile_settings.oauth.kick.access_token = access_token;
            if let Some(rt) = refresh_token {
                profile_settings.oauth.kick.refresh_token = rt;
            }
            profile_settings.oauth.kick.expires_at = expires_at;
            profile_settings.oauth.kick.user_id = user_info.user_id.clone();
            profile_settings.oauth.kick.username = user_info.username.clone();
            profile_settings.oauth.kick.display_name = user_info.display_name.clone();
        }
        _ => {
            return Err(spiritstream_core::CoreError::NotImplemented {
                feature: format!("Unknown provider: {provider}"),
            })
        }
    }

    persist_active_profile_settings(state, profile_settings).await
}

pub(crate) async fn clear_profile_oauth_account(
    state: &AppState,
    provider: &str,
) -> Result<(), spiritstream_core::CoreError> {
    let mut profile_settings = get_active_profile_settings(state)
        .await
        .ok_or(spiritstream_core::CoreError::NoActiveProfile)?;

    match provider {
        "twitch" => {
            profile_settings.oauth.twitch = Default::default();
        }
        "youtube" => {
            profile_settings.oauth.youtube = Default::default();
        }
        "kick" => {
            profile_settings.oauth.kick = Default::default();
        }
        _ => {
            return Err(spiritstream_core::CoreError::NotImplemented {
                feature: format!("Unknown provider: {provider}"),
            })
        }
    }

    persist_active_profile_settings(state, profile_settings).await
}

/// Auto-connect all configured chat platforms when a stream starts.
/// Runs as a fire-and-forget background task -- errors are logged, never block the stream.
pub(crate) async fn auto_connect_chat_platforms(state: AppState) {
    let chat_settings = state.chat_manager.profile_chat_settings().await;
    if chat_settings.twitch_channel.trim().is_empty()
        && chat_settings.youtube_channel_id.trim().is_empty()
        && chat_settings.trovo_channel_id.trim().is_empty()
        && chat_settings.kick_channel.trim().is_empty()
    {
        return;
    }

    let mut profile_settings = match get_active_profile_settings(&state).await {
        Some(settings) => settings,
        None => {
            log::warn!("Chat auto-connect skipped: no active profile settings");
            return;
        }
    };

    // Twitch: refresh token if needed, then connect immediately (IRC works even when offline)
    if !chat_settings.twitch_channel.is_empty() {
        let already_connected = state
            .chat_manager
            .get_platform_status(ChatPlatform::Twitch)
            .await
            .map(|s| s.status == spiritstream_core::models::ChatConnectionStatus::Connected)
            .unwrap_or(false);

        if !already_connected {
            if !profile_settings.oauth.twitch.access_token.is_empty() {
                // Refresh token if expired
                match ensure_fresh_oauth_token(
                    "twitch",
                    &profile_settings.oauth.twitch.access_token,
                    &profile_settings.oauth.twitch.refresh_token,
                    profile_settings.oauth.twitch.expires_at,
                    &state.oauth_service,
                )
                .await
                {
                    Ok(fresh) => {
                        if fresh.refreshed {
                            profile_settings.oauth.twitch.access_token = fresh.access_token.clone();
                            if let Some(rt) = fresh.refresh_token {
                                profile_settings.oauth.twitch.refresh_token = rt;
                            }
                            profile_settings.oauth.twitch.expires_at = fresh.expires_at;
                            if let Err(err) =
                                persist_active_profile_settings(&state, profile_settings.clone())
                                    .await
                            {
                                log::warn!("Failed to persist Twitch OAuth refresh: {err}");
                            }
                        }
                        connect_twitch_chat(
                            &state.chat_manager,
                            &chat_settings,
                            &profile_settings,
                            &state.event_bus,
                        )
                        .await;
                    }
                    Err(e) => {
                        log::warn!("Twitch token refresh failed, trying with existing token: {e}");
                        connect_twitch_chat(
                            &state.chat_manager,
                            &chat_settings,
                            &profile_settings,
                            &state.event_bus,
                        )
                        .await;
                    }
                }
            } else {
                connect_twitch_chat(
                    &state.chat_manager,
                    &chat_settings,
                    &profile_settings,
                    &state.event_bus,
                )
                .await;
            }
        } else {
            log::debug!("Twitch chat already connected, skipping auto-connect");
        }
    }

    // Trovo: read-only websocket chat (requires SPIRITSTREAM_TROVO_CLIENT_ID + channel ID)
    if !chat_settings.trovo_channel_id.is_empty() {
        let already_connected = state
            .chat_manager
            .get_platform_status(ChatPlatform::Trovo)
            .await
            .map(|s| s.status == spiritstream_core::models::ChatConnectionStatus::Connected)
            .unwrap_or(false);

        if !already_connected {
            connect_trovo_chat(&state.chat_manager, &chat_settings, &state.event_bus).await;
        } else {
            log::debug!("Trovo chat already connected, skipping auto-connect");
        }
    }

    // Kick: Pusher-protocol websocket. Anonymous read; OAuth bearer needed for send.
    if !chat_settings.kick_channel.is_empty() {
        let already_connected = state
            .chat_manager
            .get_platform_status(ChatPlatform::Kick)
            .await
            .map(|s| s.status == spiritstream_core::models::ChatConnectionStatus::Connected)
            .unwrap_or(false);

        if !already_connected {
            // Refresh the Kick OAuth token if it's about to expire; do this
            // before capturing the bearer for the connector so a stale token
            // doesn't get baked in for the session.
            if !profile_settings.oauth.kick.access_token.is_empty() {
                if let Ok(fresh) = ensure_fresh_oauth_token(
                    "kick",
                    &profile_settings.oauth.kick.access_token,
                    &profile_settings.oauth.kick.refresh_token,
                    profile_settings.oauth.kick.expires_at,
                    &state.oauth_service,
                )
                .await
                {
                    if fresh.refreshed {
                        profile_settings.oauth.kick.access_token = fresh.access_token.clone();
                        if let Some(rt) = fresh.refresh_token {
                            profile_settings.oauth.kick.refresh_token = rt;
                        }
                        profile_settings.oauth.kick.expires_at = fresh.expires_at;
                        if let Err(err) =
                            persist_active_profile_settings(&state, profile_settings.clone())
                                .await
                        {
                            log::warn!("Failed to persist Kick OAuth refresh: {err}");
                        }
                    }
                }
            }
            connect_kick_chat(
                &state.chat_manager,
                &chat_settings,
                &profile_settings,
                &state.event_bus,
            )
            .await;
        } else {
            log::debug!("Kick chat already connected, skipping auto-connect");
        }
    }

    // YouTube: connect with retry (broadcast won't be live until OBS starts streaming)
    if !chat_settings.youtube_channel_id.is_empty() {
        let has_oauth = !chat_settings.youtube_use_api_key
            && !profile_settings.oauth.youtube.access_token.is_empty();
        let has_api_key =
            chat_settings.youtube_use_api_key && !chat_settings.youtube_api_key.is_empty();

        if has_oauth || has_api_key {
            let already_connected = state
                .chat_manager
                .get_platform_status(ChatPlatform::YouTube)
                .await
                .map(|s| s.status == spiritstream_core::models::ChatConnectionStatus::Connected)
                .unwrap_or(false);

            if !already_connected {
                // Spawn as separate task -- retries can take up to 5 minutes
                tokio::spawn(connect_youtube_chat_with_retry(state.clone()));
            } else {
                log::debug!("YouTube chat already connected, skipping auto-connect");
            }
        }
    }
}

pub(crate) async fn connect_twitch_chat(
    chat_manager: &Arc<ChatManager>,
    chat_settings: &ChatSettings,
    profile_settings: &ProfileSettings,
    event_bus: &EventBus,
) {
    let auth = if profile_settings.oauth.twitch.access_token.is_empty() {
        None
    } else {
        Some(TwitchAuth::AppOAuth {
            access_token: profile_settings.oauth.twitch.access_token.clone(),
            refresh_token: Some(profile_settings.oauth.twitch.refresh_token.clone())
                .filter(|s| !s.is_empty()),
            expires_at: if profile_settings.oauth.twitch.expires_at > 0 {
                Some(profile_settings.oauth.twitch.expires_at)
            } else {
                None
            },
        })
    };

    let config = ChatConfig {
        platform: ChatPlatform::Twitch,
        enabled: true,
        credentials: ChatCredentials::Twitch {
            channel: chat_settings.twitch_channel.clone(),
            auth,
        },
    };
    match chat_manager.connect(config).await {
        Ok(()) => {
            log::info!("Auto-connected to Twitch chat");
            event_bus.emit("chat_auto_connected", json!({ "platform": "twitch" }));
        }
        Err(e) => {
            if e.to_string().to_lowercase().contains("already connected") {
                log::debug!("Twitch chat already connected");
            } else {
                log::warn!("Failed to auto-connect Twitch chat: {e}");
                // Surface auto-connect failures to the UI so the
                // user gets a toast instead of silently missing chat. The
                // stream itself continues regardless.
                event_bus.emit(
                    "chat_auto_connect_failed",
                    json!({ "platform": "twitch", "kind": e.kind(), "error": e.to_string() }),
                );
            }
        }
    }
}

pub(crate) async fn connect_trovo_chat(
    chat_manager: &Arc<ChatManager>,
    chat_settings: &ChatSettings,
    event_bus: &EventBus,
) {
    let config = ChatConfig {
        platform: ChatPlatform::Trovo,
        enabled: true,
        credentials: ChatCredentials::Trovo {
            channel_id: chat_settings.trovo_channel_id.clone(),
        },
    };
    match chat_manager.connect(config).await {
        Ok(()) => {
            log::info!("Auto-connected to Trovo chat");
            event_bus.emit("chat_auto_connected", json!({ "platform": "trovo" }));
        }
        Err(e) => {
            if e.to_string().to_lowercase().contains("already connected") {
                log::debug!("Trovo chat already connected");
            } else {
                log::warn!("Failed to auto-connect Trovo chat: {e}");
                event_bus.emit(
                    "chat_auto_connect_failed",
                    json!({ "platform": "trovo", "kind": e.kind(), "error": e.to_string() }),
                );
            }
        }
    }
}

/// Auto-connect Kick chat. Anonymous Pusher subscription works
/// without auth; if `chat_settings.kick_send_enabled` is on and the
/// profile has a Kick OAuth account, the send-path credentials get
/// captured at `connect()` time so outbound chat works without a
/// reconnect dance. `oauth.kick.user_id` doubles as the
/// `broadcaster_user_id` Kick's REST POST expects (Kick's "me" user
/// IS the broadcaster from the sender's perspective).
pub(crate) async fn connect_kick_chat(
    chat_manager: &Arc<ChatManager>,
    chat_settings: &ChatSettings,
    profile_settings: &ProfileSettings,
    event_bus: &EventBus,
) {
    let (oauth_token, broadcaster_user_id) = if chat_settings.kick_send_enabled
        && !profile_settings.oauth.kick.access_token.is_empty()
        && !profile_settings.oauth.kick.user_id.is_empty()
    {
        let broadcaster = profile_settings.oauth.kick.user_id.parse::<u64>().ok();
        (
            Some(profile_settings.oauth.kick.access_token.clone()),
            broadcaster,
        )
    } else {
        (None, None)
    };

    let config = ChatConfig {
        platform: ChatPlatform::Kick,
        enabled: true,
        credentials: ChatCredentials::Kick {
            channel: chat_settings.kick_channel.clone(),
            oauth_token,
            broadcaster_user_id,
        },
    };
    match chat_manager.connect(config).await {
        Ok(()) => {
            log::info!("Auto-connected to Kick chat");
            event_bus.emit("chat_auto_connected", json!({ "platform": "kick" }));
        }
        Err(e) => {
            if e.to_string().to_lowercase().contains("already connected") {
                log::debug!("Kick chat already connected");
            } else {
                log::warn!("Failed to auto-connect Kick chat: {e}");
                event_bus.emit(
                    "chat_auto_connect_failed",
                    json!({ "platform": "kick", "kind": e.kind(), "error": e.to_string() }),
                );
            }
        }
    }
}

/// Wait for stream data to flow, then connect YouTube chat.
///
/// SpiritStream starts its RTMP relay *before* OBS connects, so the YouTube
/// broadcast won't be "active" until OBS is streaming and YouTube has ingested
/// enough data. We subscribe to the EventBus, wait for the first `stream_stats`
/// event (proof that data is flowing from OBS), give YouTube time to register
/// the broadcast, then attempt to connect with a few retries.
pub(crate) async fn connect_youtube_chat_with_retry(state: AppState) {
    let build_config = |chat: &ChatSettings,
                        s: &ProfileSettings,
                        token_override: Option<&str>|
     -> Option<ChatConfig> {
        if chat.youtube_channel_id.trim().is_empty() {
            return None;
        }

        let auth = if chat.youtube_use_api_key {
            if chat.youtube_api_key.trim().is_empty() {
                return None;
            }
            YouTubeAuth::ApiKey {
                key: chat.youtube_api_key.clone(),
            }
        } else {
            let access_token = token_override.unwrap_or(&s.oauth.youtube.access_token);
            if access_token.is_empty() {
                return None;
            }
            YouTubeAuth::AppOAuth {
                access_token: access_token.to_string(),
                refresh_token: Some(s.oauth.youtube.refresh_token.clone())
                    .filter(|t| !t.is_empty()),
                expires_at: if s.oauth.youtube.expires_at > 0 {
                    Some(s.oauth.youtube.expires_at)
                } else {
                    None
                },
            }
        };

        Some(ChatConfig {
            platform: ChatPlatform::YouTube,
            enabled: true,
            credentials: ChatCredentials::YouTube {
                channel_id: chat.youtube_channel_id.clone(),
                auth,
            },
        })
    };

    let initial_chat = state.chat_manager.profile_chat_settings().await;
    if initial_chat.youtube_channel_id.trim().is_empty() {
        return;
    }

    // Wait for stream_stats (OBS connected, data flowing)
    log::info!("YouTube chat: waiting for stream data before connecting...");
    let mut rx = state.event_bus.subscribe();
    let got_stats = tokio::time::timeout(
        std::time::Duration::from_secs(300), // 5 min max wait for OBS
        async {
            loop {
                match rx.recv().await {
                    Ok(event) if event.event == "stream_stats" => return,
                    Ok(_) => continue,
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(_) => return, // channel closed
                }
            }
        },
    )
    .await;

    if got_stats.is_err() {
        log::warn!("YouTube chat: timed out waiting for stream data (OBS never connected?)");
        return;
    }
    if state.ffmpeg_handler.active_count() == 0 {
        log::info!("YouTube chat: stream stopped before OBS data arrived");
        return;
    }

    // Data is flowing. Give YouTube ~10s to register the broadcast.
    log::info!("Stream data detected -- waiting 10s for YouTube to register broadcast...");
    tokio::time::sleep(std::time::Duration::from_secs(10)).await;

    // Attempt connect with a few retries (15s apart)
    // Load fresh settings each attempt so we pick up refreshed tokens
    const MAX_RETRIES: u32 = 6; // 6 x 15s = 90s of retries after initial wait
    for attempt in 0..=MAX_RETRIES {
        if state.ffmpeg_handler.active_count() == 0 {
            log::info!("YouTube chat: stream stopped, cancelling connect");
            return;
        }

        let chat_settings = state.chat_manager.profile_chat_settings().await;
        if chat_settings.youtube_channel_id.trim().is_empty() {
            log::info!("YouTube chat: no channel configured, cancelling connect");
            return;
        }
        if chat_settings.youtube_use_api_key && chat_settings.youtube_api_key.trim().is_empty() {
            log::info!("YouTube chat: API key mode enabled but no key configured");
            return;
        }

        // Load fresh profile settings and refresh token if needed
        let mut profile_settings = match get_active_profile_settings(&state).await {
            Some(s) => s,
            None => {
                log::warn!("YouTube chat: no active profile settings");
                return;
            }
        };

        // Refresh YouTube OAuth token if expired (skip for API key mode)
        let fresh_token = if !chat_settings.youtube_use_api_key
            && !profile_settings.oauth.youtube.access_token.is_empty()
        {
            match ensure_fresh_oauth_token(
                "youtube",
                &profile_settings.oauth.youtube.access_token,
                &profile_settings.oauth.youtube.refresh_token,
                profile_settings.oauth.youtube.expires_at,
                &state.oauth_service,
            )
            .await
            {
                Ok(fresh) => {
                    if fresh.refreshed {
                        profile_settings.oauth.youtube.access_token = fresh.access_token.clone();
                        if let Some(rt) = fresh.refresh_token {
                            profile_settings.oauth.youtube.refresh_token = rt;
                        }
                        profile_settings.oauth.youtube.expires_at = fresh.expires_at;
                        if let Err(err) =
                            persist_active_profile_settings(&state, profile_settings.clone()).await
                        {
                            log::warn!("Failed to persist YouTube OAuth refresh: {err}");
                        }
                    }
                    Some(fresh.access_token)
                }
                Err(e) => {
                    log::warn!("YouTube token refresh failed: {e}");
                    None // try with existing token anyway
                }
            }
        } else {
            None
        };

        let config = match build_config(&chat_settings, &profile_settings, fresh_token.as_deref()) {
            Some(config) => config,
            None => {
                log::info!("YouTube chat: missing auth or channel info, cancelling connect");
                return;
            }
        };

        match state.chat_manager.connect(config).await {
            Ok(()) => {
                log::info!("Auto-connected to YouTube chat");
                state
                    .event_bus
                    .emit("chat_auto_connected", json!({ "platform": "youtube" }));
                return;
            }
            Err(e) => {
                let lower = e.to_string().to_lowercase();
                if lower.contains("already connected") {
                    log::debug!("YouTube chat already connected");
                    return;
                } else if lower.contains("no active live broadcast") || lower.contains("not live") {
                    if attempt < MAX_RETRIES {
                        log::info!(
                            "YouTube broadcast not live yet (attempt {}/{}), retrying in 15s...",
                            attempt + 1,
                            MAX_RETRIES + 1
                        );
                        tokio::time::sleep(std::time::Duration::from_secs(15)).await;
                    } else {
                        log::warn!("YouTube chat: broadcast never went live after all retries");
                        state.event_bus.emit(
                            "chat_auto_connect_failed",
                            json!({ "platform": "youtube", "kind": e.kind(), "error": e.to_string() }),
                        );
                    }
                } else {
                    log::warn!("Failed to auto-connect YouTube chat: {e}");
                    state.event_bus.emit(
                        "chat_auto_connect_failed",
                        json!({ "platform": "youtube", "kind": e.kind(), "error": e.to_string() }),
                    );
                    return;
                }
            }
        }
    }
}

/// Auto-disconnect all chat platforms when all streams stop.
pub(crate) async fn auto_disconnect_chat_platforms(
    chat_manager: Arc<ChatManager>,
    event_bus: EventBus,
) {
    if chat_manager.is_any_connected().await {
        match chat_manager.disconnect_all("streams_stopped").await {
            Ok(()) => {
                log::info!("Auto-disconnected all chat platforms");
                event_bus.emit("chat_auto_disconnected", json!({}));
            }
            Err(e) => {
                log::warn!("Failed to auto-disconnect chat: {e}");
            }
        }
    }
}

/// Background task to refresh YouTube OAuth tokens and update the live chat connector.
pub(crate) async fn start_youtube_token_refresh_task(state: AppState) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        loop {
            interval.tick().await;

            let is_connected = state
                .chat_manager
                .get_platform_status(ChatPlatform::YouTube)
                .await
                .map(|s| s.status == spiritstream_core::models::ChatConnectionStatus::Connected)
                .unwrap_or(false);

            if !is_connected {
                continue;
            }

            let mut profile_settings = match get_active_profile_settings(&state).await {
                Some(s) => s,
                None => {
                    log::warn!("YouTube token refresh: no active profile settings");
                    continue;
                }
            };

            if profile_settings.oauth.youtube.access_token.is_empty()
                || profile_settings.oauth.youtube.refresh_token.is_empty()
                || profile_settings.oauth.youtube.expires_at <= 0
            {
                continue;
            }

            let previous_token = profile_settings.oauth.youtube.access_token.clone();
            match ensure_fresh_oauth_token(
                "youtube",
                &previous_token,
                &profile_settings.oauth.youtube.refresh_token,
                profile_settings.oauth.youtube.expires_at,
                &state.oauth_service,
            )
            .await
            {
                Ok(fresh) => {
                    if fresh.refreshed {
                        profile_settings.oauth.youtube.access_token = fresh.access_token.clone();
                        if let Some(rt) = fresh.refresh_token {
                            profile_settings.oauth.youtube.refresh_token = rt;
                        }
                        profile_settings.oauth.youtube.expires_at = fresh.expires_at;
                        if let Err(err) =
                            persist_active_profile_settings(&state, profile_settings.clone()).await
                        {
                            log::warn!("Failed to persist YouTube OAuth refresh: {err}");
                        }
                    }
                    if fresh.access_token != previous_token {
                        if let Err(e) = state
                            .chat_manager
                            .update_platform_token(ChatPlatform::YouTube, fresh.access_token)
                            .await
                        {
                            log::warn!("Failed to update YouTube chat token: {e}");
                        } else {
                            log::info!("YouTube chat token refreshed and updated");
                        }
                    }
                }
                Err(e) => {
                    log::warn!("YouTube token refresh failed: {e}");
                }
            }
        }
    });
}

/// Background task to retry chat connections when a platform drops.
/// Subscribe to the server's own event bus and auto-retry failed streams.
/// Replaces the frontend's `handleAutoRetry` (in `useStreamStats.ts`) which
/// previously listened to `stream_error` and called `api.stream.retry()`.
/// Backend now owns both the policy (in `FFmpegHandler::retry_group`) and
/// the trigger. The frontend just renders `stream_retry_attempt` events.
pub(crate) async fn start_auto_retry_task(state: AppState) {
    let mut rx = state.event_bus.subscribe();
    tokio::spawn(async move {
        loop {
            let evt = match rx.recv().await {
                Ok(e) => e,
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => break,
            };
            if evt.event != "stream_error" {
                continue;
            }
            let Some(group_id) = evt
                .payload
                .get("groupId")
                .and_then(|v| v.as_str())
                .map(String::from)
            else {
                continue;
            };
            // Spawn the retry in a blocking task — retry_group_internal sleeps
            // for the backoff window which must not block the runtime.
            let handler = state.ffmpeg_handler.clone();
            let event_sink: Arc<dyn EventSink> = Arc::new(state.event_bus.clone());
            tokio::task::spawn_blocking(move || {
                if let Err(err) = handler.retry_group(&group_id, event_sink) {
                    log::warn!("[auto_retry] retry_group({group_id}) failed: {err}");
                }
            });
        }
    });
}

pub(crate) async fn start_chat_reconnect_task(state: AppState) {
    tokio::spawn(async move {
        let mut last_attempts: HashMap<ChatPlatform, Instant> = HashMap::new();
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(15));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        loop {
            interval.tick().await;

            if state.ffmpeg_handler.active_count() == 0 {
                continue;
            }

            let statuses = state.chat_manager.get_status().await;
            for status in statuses {
                if status.status != spiritstream_core::models::ChatConnectionStatus::Error {
                    continue;
                }

                if last_attempts
                    .get(&status.platform)
                    .map(|last| last.elapsed() < std::time::Duration::from_secs(30))
                    .unwrap_or(false)
                {
                    continue;
                }
                last_attempts.insert(status.platform, Instant::now());

                let profile_settings = match get_active_profile_settings(&state).await {
                    Some(s) => s,
                    None => {
                        log::warn!("Chat reconnect: no active profile settings");
                        continue;
                    }
                };
                let chat_settings = state.chat_manager.profile_chat_settings().await;

                match status.platform {
                    ChatPlatform::Twitch => {
                        if chat_settings.twitch_channel.is_empty() {
                            continue;
                        }
                        connect_twitch_chat(
                            &state.chat_manager,
                            &chat_settings,
                            &profile_settings,
                            &state.event_bus,
                        )
                        .await;
                    }
                    ChatPlatform::Trovo => {
                        if chat_settings.trovo_channel_id.is_empty() {
                            continue;
                        }
                        connect_trovo_chat(&state.chat_manager, &chat_settings, &state.event_bus)
                            .await;
                    }
                    ChatPlatform::YouTube => {
                        let has_oauth = !chat_settings.youtube_use_api_key
                            && !profile_settings.oauth.youtube.access_token.is_empty();
                        let has_api_key = chat_settings.youtube_use_api_key
                            && !chat_settings.youtube_api_key.is_empty();
                        if chat_settings.youtube_channel_id.is_empty()
                            || (!has_oauth && !has_api_key)
                        {
                            continue;
                        }
                        tokio::spawn(connect_youtube_chat_with_retry(state.clone()));
                    }
                    _ => {}
                }
            }
        }
    });
}
