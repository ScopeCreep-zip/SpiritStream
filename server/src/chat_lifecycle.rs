use serde_json::json;
use std::sync::Arc;
use tokio::sync::broadcast;

use crate::events::EventBus;
use crate::state::{apply_and_persist_oauth_refresh, ensure_fresh_oauth_token, get_active_profile_settings, AppState};
use spiritstream_server::models::{ChatConfig, ChatCredentials, ChatPlatform, ChatSettings, ProfileSettings, TwitchAuth, YouTubeAuth};
use spiritstream_server::services::{ChatManager, EventSink};

/// Auto-connect all configured chat platforms when a stream starts.
/// Runs as a fire-and-forget background task -- errors are logged, never block the stream.
pub(crate) async fn auto_connect_chat_platforms(
    state: AppState,
) {
    let chat_settings = state.chat_manager.profile_chat_settings().await;
    if chat_settings.twitch_channel.trim().is_empty()
        && chat_settings.youtube_channel_id.trim().is_empty()
        && chat_settings.trovo_channel_id.trim().is_empty()
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
        let already_connected = state.chat_manager
            .get_platform_status(ChatPlatform::Twitch)
            .await
            .map(|s| s.status == spiritstream_server::models::ChatConnectionStatus::Connected)
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
                        apply_and_persist_oauth_refresh(&state, "twitch", &fresh, &mut profile_settings).await;
                        connect_twitch_chat(&state.chat_manager, &chat_settings, &profile_settings, &state.event_bus).await;
                    }
                    Err(e) => {
                        log::warn!("Twitch token refresh failed, trying with existing token: {e}");
                        connect_twitch_chat(&state.chat_manager, &chat_settings, &profile_settings, &state.event_bus).await;
                    }
                }
            } else {
                connect_twitch_chat(&state.chat_manager, &chat_settings, &profile_settings, &state.event_bus).await;
            }
        } else {
            log::debug!("Twitch chat already connected, skipping auto-connect");
        }
    }

    // Trovo: read-only websocket chat (requires TROVO_CLIENT_ID + channel ID)
    if !chat_settings.trovo_channel_id.is_empty() {
        let already_connected = state.chat_manager
            .get_platform_status(ChatPlatform::Trovo)
            .await
            .map(|s| s.status == spiritstream_server::models::ChatConnectionStatus::Connected)
            .unwrap_or(false);

        if !already_connected {
            connect_trovo_chat(&state.chat_manager, &chat_settings, &state.event_bus).await;
        } else {
            log::debug!("Trovo chat already connected, skipping auto-connect");
        }
    }

    // YouTube: connect with retry (broadcast won't be live until OBS starts streaming)
    if !chat_settings.youtube_channel_id.is_empty() {
        let has_oauth = !chat_settings.youtube_use_api_key
            && !profile_settings.oauth.youtube.access_token.is_empty();
        let has_api_key = chat_settings.youtube_use_api_key && !chat_settings.youtube_api_key.is_empty();

        if has_oauth || has_api_key {
            let already_connected = state.chat_manager
                .get_platform_status(ChatPlatform::YouTube)
                .await
                .map(|s| s.status == spiritstream_server::models::ChatConnectionStatus::Connected)
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
            if e.to_lowercase().contains("already connected") {
                log::debug!("Twitch chat already connected");
            } else {
                log::warn!("Failed to auto-connect Twitch chat: {e}");
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
            if e.to_lowercase().contains("already connected") {
                log::debug!("Trovo chat already connected");
            } else {
                log::warn!("Failed to auto-connect Trovo chat: {e}");
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
pub(crate) async fn connect_youtube_chat_with_retry(
    state: AppState,
) {
    let build_config = |chat: &ChatSettings, s: &ProfileSettings, token_override: Option<&str>| -> Option<ChatConfig> {
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

    // Phase 1: Wait for stream_stats (OBS connected, data flowing)
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

    // Phase 2: Data is flowing. Give YouTube ~10s to register the broadcast.
    log::info!("Stream data detected -- waiting 10s for YouTube to register broadcast...");
    tokio::time::sleep(std::time::Duration::from_secs(10)).await;

    // Phase 3: Attempt connect with a few retries (15s apart)
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
            ).await {
                Ok(fresh) => {
                    let token = apply_and_persist_oauth_refresh(&state, "youtube", &fresh, &mut profile_settings).await;
                    Some(token)
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
                state.event_bus.emit("chat_auto_connected", json!({ "platform": "youtube" }));
                return;
            }
            Err(e) => {
                let lower = e.to_lowercase();
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
                    }
                } else {
                    log::warn!("Failed to auto-connect YouTube chat: {e}");
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
        match chat_manager.disconnect_all().await {
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
