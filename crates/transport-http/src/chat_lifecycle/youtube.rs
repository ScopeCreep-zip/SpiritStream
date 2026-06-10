//! YouTube-specific chat lifecycle: stream-detection-gated connect with
//! retry loop + the background OAuth token refresh task. Pulled out of
//! `chat_lifecycle.rs` (K3) so the orchestrator stays under the 600 LOC
//! ceiling. Both fns are spawned as fire-and-forget tasks from
//! `auto_connect_chat_platforms` and `start_youtube_token_refresh_task`
//! at chat-lifecycle startup.

use std::time::Duration;

use serde_json::json;
use tokio::sync::broadcast;

use spiritstream_core::models::{
    ChatConfig, ChatCredentials, ChatPlatform, ChatSettings, ProfileSettings, YouTubeAuth,
};
use spiritstream_core::services::EventSink;

use crate::AppState;

use super::{
    ensure_fresh_oauth_token, get_active_profile_settings, persist_active_profile_settings,
};

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
        Duration::from_secs(300), // 5 min max wait for OBS
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
    tokio::time::sleep(Duration::from_secs(10)).await;

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
                }
                if lower.contains("no active live broadcast") || lower.contains("not live") {
                    if attempt < MAX_RETRIES {
                        log::info!(
                            "YouTube broadcast not live yet (attempt {}/{}), retrying in 15s...",
                            attempt + 1,
                            MAX_RETRIES + 1
                        );
                        tokio::time::sleep(Duration::from_secs(15)).await;
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

/// Background task to refresh YouTube OAuth tokens and update the live chat connector.
pub(crate) async fn start_youtube_token_refresh_task(state: AppState) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(60));
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
