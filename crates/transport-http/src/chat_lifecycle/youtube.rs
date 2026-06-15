//! YouTube-specific chat lifecycle: the single forward connect path
//! (`connect_youtube_chat`) plus the background OAuth token-refresh task.
//! Pulled out of `chat_lifecycle.rs` (K3) so the orchestrator stays under the
//! 600 LOC ceiling.
//!
//! YouTube live chat exists — and is both readable AND postable — ONLY while a
//! broadcast is live. There is exactly one connect path here: it resolves the
//! currently-live broadcast and connects, failing loud as a calm `Disconnected`
//! ("go live first") when there's nothing live. No fallback chain, no OBS-gated
//! waiter, no idle polling.

use std::time::Duration;

use serde_json::json;

use spiritstream_core::models::{
    ChatConfig, ChatConnectionStatus, ChatCredentials, ChatPlatform, ChatSettings, ProfileSettings,
    YouTubeAuth,
};
use spiritstream_core::services::EventSink;

use crate::AppState;

use super::{
    connect_failure_reason, ensure_fresh_oauth_token, get_active_profile_settings,
    persist_active_profile_settings,
};

/// Build the YouTube `ChatConfig` from settings + profile. `token_override`
/// carries a just-refreshed access token when one is available. Returns
/// `None` when the channel or auth is missing — there's nothing to connect.
pub(crate) fn build_youtube_config(
    chat: &ChatSettings,
    s: &ProfileSettings,
    token_override: Option<&str>,
) -> Option<ChatConfig> {
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
            refresh_token: Some(s.oauth.youtube.refresh_token.clone()).filter(|t| !t.is_empty()),
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
}

/// Current YouTube connection status, if tracked.
async fn youtube_status(state: &AppState) -> Option<ChatConnectionStatus> {
    state
        .chat_manager
        .get_platform_status(ChatPlatform::YouTube)
        .await
        .map(|s| s.status)
}

/// Refresh the YouTube OAuth access token if it's near expiry, persisting a
/// rotated refresh token. Returns the token to use, or `None` for API-key mode
/// / when there's nothing to refresh (the stored token is used as-is).
async fn refresh_youtube_access_token(
    state: &AppState,
    chat_settings: &ChatSettings,
    profile_settings: &mut ProfileSettings,
) -> Option<String> {
    if chat_settings.youtube_use_api_key || profile_settings.oauth.youtube.access_token.is_empty() {
        return None;
    }
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
                    persist_active_profile_settings(state, profile_settings.clone()).await
                {
                    log::warn!("Failed to persist YouTube OAuth refresh: {err}");
                }
            }
            Some(fresh.access_token)
        }
        Err(e) => {
            log::warn!("YouTube token refresh failed, using existing token: {e}");
            None
        }
    }
}

/// THE single YouTube connect path. Refreshes the token, resolves the
/// CURRENTLY-LIVE broadcast, and connects (read + send). Triggered by: the
/// per-platform Connect button, profile activation / app restart, and
/// stream-start — never an idle poll.
///
/// YouTube chat only exists while live, so "not live" is a calm `Disconnected`
/// ("go live first"), never an `Error` that churns the reconnect loop.
///
/// `max_attempts` / `retry_delay` give a BOUNDED retry to absorb the few-second
/// gap between a stream starting to push and YouTube marking the broadcast
/// `active`. Ambient callers pass `1`; the stream-start trigger passes a small
/// budget (≈6 × 15s). `announce = true` (the Connect button) surfaces a final
/// failure as a `chat_auto_connect_failed` toast; `false` logs it quietly.
pub(crate) async fn connect_youtube_chat(
    state: &AppState,
    announce: bool,
    max_attempts: u32,
    retry_delay: Duration,
) {
    let chat_settings = state.chat_manager.profile_chat_settings().await;

    if chat_settings.youtube_channel_id.trim().is_empty() {
        log::info!("YouTube connect skipped: no channel configured");
        return;
    }
    if state
        .chat_manager
        .is_disconnect_intended(ChatPlatform::YouTube)
        .await
    {
        log::info!("YouTube connect skipped: disconnected by the user this session");
        return;
    }
    let Some(mut profile_settings) = get_active_profile_settings(state).await else {
        log::warn!("YouTube connect skipped: no active profile settings");
        return;
    };
    let has_oauth = !chat_settings.youtube_use_api_key
        && !profile_settings.oauth.youtube.access_token.is_empty();
    let has_api_key =
        chat_settings.youtube_use_api_key && !chat_settings.youtube_api_key.trim().is_empty();
    if !has_oauth && !has_api_key {
        log::info!("YouTube connect skipped: not signed in (no OAuth token, no API key)");
        return;
    }
    if youtube_status(state).await == Some(ChatConnectionStatus::Connected) {
        log::debug!("YouTube chat already connected, skipping connect");
        return;
    }

    let fresh_token =
        refresh_youtube_access_token(state, &chat_settings, &mut profile_settings).await;
    let Some(config) =
        build_youtube_config(&chat_settings, &profile_settings, fresh_token.as_deref())
    else {
        log::info!("YouTube connect skipped: missing auth/channel after token refresh");
        return;
    };

    for attempt in 1..=max_attempts {
        match state.chat_manager.connect(config.clone()).await {
            Ok(()) => {
                log::info!("YouTube chat connected to live chat");
                state
                    .event_bus
                    .emit("chat_auto_connected", json!({ "platform": "youtube" }));
                return;
            }
            Err(e) => {
                if e.to_string().to_lowercase().contains("already connected") {
                    log::debug!("YouTube chat already connected");
                    return;
                }
                // The connector maps a not-live broadcast AND quota exhaustion
                // to a calm DISCONNECTED (vs ERROR for real failures). `reason`
                // is the actionable message ("go live first" / "quota
                // exhausted"), not the opaque "internal error".
                let reason = connect_failure_reason(&e);
                let calm =
                    youtube_status(state).await == Some(ChatConnectionStatus::Disconnected);
                let quota = reason.to_lowercase().contains("quota");
                // Only the not-live registration lag is worth a bounded retry;
                // retrying an exhausted quota just burns more of it.
                if calm && !quota && attempt < max_attempts {
                    log::info!(
                        "YouTube not live yet (attempt {attempt}/{max_attempts}); retrying in {}s",
                        retry_delay.as_secs()
                    );
                    tokio::time::sleep(retry_delay).await;
                    continue;
                }
                if calm {
                    log::info!("YouTube connect not established: {reason}");
                } else {
                    log::warn!("Failed to connect YouTube chat: {reason}");
                }
                if announce {
                    state.event_bus.emit(
                        "chat_auto_connect_failed",
                        json!({ "platform": "youtube", "kind": e.kind(), "error": reason }),
                    );
                }
                return;
            }
        }
    }
}

/// Background task to refresh YouTube OAuth tokens and update the live chat connector.
pub(crate) async fn start_youtube_token_refresh_task(state: AppState) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(60));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        loop {
            interval.tick().await;

            // Refresh whenever there's a LIVE connection (Connected OR an Error
            // blip), not only when fully Connected. A 401 marks the poll
            // `Error`; gating on `Connected` meant the token never refreshed, so
            // the poll stayed wedged on the dead token forever (the deadlock).
            // Refreshing on Error lets the watch-channel token swap recover it.
            let has_live_connection = state
                .chat_manager
                .get_platform_status(ChatPlatform::YouTube)
                .await
                .map(|s| s.status != spiritstream_core::models::ChatConnectionStatus::Disconnected)
                .unwrap_or(false);

            if !has_live_connection {
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
