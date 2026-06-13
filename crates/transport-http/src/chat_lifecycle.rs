use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use serde_json::json;
use tokio::sync::broadcast;

use spiritstream_core::models::{
    ChatConfig, ChatCredentials, ChatPlatform, ChatSettings, ProfileSettings, TwitchAuth,
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

// K3: profile-state mutators + YouTube-specific lifecycle live in
// submodules so this orchestrator stays under the 600 LOC ceiling.
// Re-exported with their original `pub(crate)` paths so existing
// call sites compile unchanged.
#[path = "chat_lifecycle/profile_state.rs"]
mod profile_state;
#[path = "chat_lifecycle/reconnect.rs"]
mod reconnect;
#[path = "chat_lifecycle/twitch.rs"]
mod twitch;
#[path = "chat_lifecycle/youtube.rs"]
mod youtube;
pub(crate) use profile_state::{
    clear_profile_oauth_account, get_active_profile_name, get_active_profile_settings,
    persist_active_profile_settings, set_active_profile, update_profile_oauth_account,
};
pub(crate) use reconnect::{
    refresh_and_connect_kick, refresh_and_connect_trovo, refresh_and_connect_twitch,
};
pub(crate) use twitch::start_twitch_token_refresh_task;
pub(crate) use youtube::{connect_youtube_chat_with_retry, start_youtube_token_refresh_task};

/// Auto-connect all configured chat platforms when a stream starts.
/// Runs as a fire-and-forget background task -- errors are logged, never block the stream.
pub(crate) async fn auto_connect_chat_platforms(state: AppState, force_readonly: bool) {
    let chat_settings = state.chat_manager.profile_chat_settings().await;
    // Facebook is intentionally excluded from this guard: it never auto-
    // connects, even with a persisted video_id. Identity-revealing connect
    // requires a per-session confirm-token + explicit user click — see
    // v1_chat_connect_proxy's enable_facebook_chat gate.
    if chat_settings.twitch_channel.trim().is_empty()
        && chat_settings.youtube_channel_id.trim().is_empty()
        && chat_settings.trovo_channel_id.trim().is_empty()
        && chat_settings.kick_channel.trim().is_empty()
        && chat_settings.tiktok_username.trim().is_empty()
    {
        return;
    }

    let profile_settings = match get_active_profile_settings(&state).await {
        Some(settings) => settings,
        None => {
            log::warn!("Chat auto-connect skipped: no active profile settings");
            return;
        }
    };

    // Twitch / Trovo / Kick: refresh the token then (re)connect. The
    // `is_disconnect_intended` gate keeps a deliberate Disconnect/panic from
    // being silently undone. `force_readonly` (only on re-auth) lets Twitch
    // swap a read-only session for a send-capable one; ambient calls leave a
    // read-only session alone to avoid churn. The per-platform refresh +
    // skip-logic lives in `chat_lifecycle/reconnect.rs`.
    if !chat_settings.twitch_channel.is_empty()
        && !state
            .chat_manager
            .is_disconnect_intended(ChatPlatform::Twitch)
            .await
    {
        refresh_and_connect_twitch(&state, &chat_settings, profile_settings.clone(), force_readonly)
            .await;
    }

    if !chat_settings.trovo_channel_id.is_empty()
        && !state
            .chat_manager
            .is_disconnect_intended(ChatPlatform::Trovo)
            .await
    {
        refresh_and_connect_trovo(&state, &chat_settings, profile_settings.clone()).await;
    }

    if !chat_settings.kick_channel.is_empty()
        && !state
            .chat_manager
            .is_disconnect_intended(ChatPlatform::Kick)
            .await
    {
        refresh_and_connect_kick(&state, &chat_settings, profile_settings.clone()).await;
    }

    // TikTok: read-only via reverse-engineered protobuf-over-WebSocket.
    // No auth required — connect by username, host must be live.
    if !chat_settings.tiktok_username.is_empty()
        && !state
            .chat_manager
            .is_disconnect_intended(ChatPlatform::TikTok)
            .await
    {
        let already_connected = state
            .chat_manager
            .get_platform_status(ChatPlatform::TikTok)
            .await
            .map(|s| s.status == spiritstream_core::models::ChatConnectionStatus::Connected)
            .unwrap_or(false);

        if !already_connected {
            connect_tiktok_chat(&state.chat_manager, &chat_settings, &state.event_bus).await;
        } else {
            log::debug!("TikTok chat already connected, skipping auto-connect");
        }
    }

    // Facebook: deliberately NOT auto-connected. The connect endpoint
    // requires a per-session `enable_facebook_chat` confirm-token (see
    // v1_chat_connect_proxy) so the streamer reads the identity warning
    // and acknowledges it before the connection opens. Auto-connecting
    // here on every profile activation would bypass that gate.

    // YouTube: connect with retry (broadcast won't be live until OBS starts streaming)
    if !chat_settings.youtube_channel_id.is_empty()
        && !state
            .chat_manager
            .is_disconnect_intended(ChatPlatform::YouTube)
            .await
    {
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
    // When true, REPLACE a live session (read-only → send-capable swap after
    // a token refresh) via `ChatManager::reconnect`; otherwise a plain
    // `connect` that no-ops on an already-connected platform.
    force_replace: bool,
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
    let result = if force_replace {
        chat_manager.reconnect(config).await
    } else {
        chat_manager.connect(config).await
    };
    match result {
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
    profile_settings: &ProfileSettings,
    trovo_client_id: String,
    event_bus: &EventBus,
) {
    // Mirror the Kick shape: send only when the user enabled it AND a
    // signed-in account exists. Reads stay client-id-only either way.
    let oauth_token = if chat_settings.trovo_send_enabled
        && !profile_settings.oauth.trovo.access_token.is_empty()
    {
        Some(profile_settings.oauth.trovo.access_token.clone())
    } else {
        None
    };

    let config = ChatConfig {
        platform: ChatPlatform::Trovo,
        enabled: true,
        credentials: ChatCredentials::Trovo {
            channel_id: chat_settings.trovo_channel_id.clone(),
            // Resolved by the caller from the OAuth config chain
            // (in-app setup → env → embedded); the connector fails
            // loud on a placeholder.
            client_id: Some(trovo_client_id),
            oauth_token,
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

/// Auto-connect TikTok chat — read-only protobuf-over-WebSocket.
/// `session_token: None` because the public stream requires no auth;
/// the connector resolves the room id internally via the upstream
/// `piratetok-live-rs` crate.
pub(crate) async fn connect_tiktok_chat(
    chat_manager: &Arc<ChatManager>,
    chat_settings: &ChatSettings,
    event_bus: &EventBus,
) {
    let config = ChatConfig {
        platform: ChatPlatform::TikTok,
        enabled: true,
        credentials: ChatCredentials::TikTok {
            username: chat_settings.tiktok_username.clone(),
            session_token: None,
        },
    };
    match chat_manager.connect(config).await {
        Ok(()) => {
            log::info!("Auto-connected to TikTok chat");
            event_bus.emit("chat_auto_connected", json!({ "platform": "tiktok" }));
        }
        Err(e) => {
            if e.to_string().to_lowercase().contains("already connected") {
                log::debug!("TikTok chat already connected");
            } else {
                log::warn!("Failed to auto-connect TikTok chat: {e}");
                event_bus.emit(
                    "chat_auto_connect_failed",
                    json!({ "platform": "tiktok", "kind": e.kind(), "error": e.to_string() }),
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

            // Chat is now decoupled from streaming: reconnect dropped
            // platforms whenever they error, whether or not a stream is
            // running. (Previously this returned early unless streaming.)
            let statuses = state.chat_manager.get_status().await;
            for status in statuses {
                if status.status != spiritstream_core::models::ChatConnectionStatus::Error {
                    continue;
                }

                // Never reconnect a platform the user deliberately
                // disconnected (Disconnect button / panic). A panic
                // leaves connectors `Disconnected` not `Error`, so this
                // is a belt-and-suspenders guard on top of that.
                if state
                    .chat_manager
                    .is_disconnect_intended(status.platform)
                    .await
                {
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
                        // Reconnect task only fires on `Error` status (the
                        // platform is down, not a live read-only session), so
                        // a plain connect is correct here — no force-replace.
                        connect_twitch_chat(
                            &state.chat_manager,
                            &chat_settings,
                            &profile_settings,
                            &state.event_bus,
                            false,
                        )
                        .await;
                    }
                    ChatPlatform::Trovo => {
                        if chat_settings.trovo_channel_id.is_empty() {
                            continue;
                        }
                        connect_trovo_chat(
                            &state.chat_manager,
                            &chat_settings,
                            &profile_settings,
                            state.oauth_service.get_config().await.get_trovo_client_id(),
                            &state.event_bus,
                        )
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
