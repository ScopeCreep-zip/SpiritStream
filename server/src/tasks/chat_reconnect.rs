use std::collections::HashMap;
use std::time::Instant;

use spiritstream_server::models::ChatPlatform;

use crate::chat_lifecycle::{connect_trovo_chat, connect_twitch_chat, connect_youtube_chat_with_retry};
use spiritstream_server::constants::{CHAT_RECONNECT_INTERVAL_SECS, CHAT_RECONNECT_COOLDOWN_SECS};
use crate::state::{get_active_profile_settings, AppState};

/// Background task to retry chat connections when a platform drops.
pub(crate) async fn start_chat_reconnect_task(
    state: AppState,
) {
    tokio::spawn(async move {
        let mut last_attempts: HashMap<ChatPlatform, Instant> = HashMap::new();
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(CHAT_RECONNECT_INTERVAL_SECS));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        loop {
            interval.tick().await;

            if state.ffmpeg_handler.active_count() == 0 {
                continue;
            }

            let statuses = state.chat_manager.get_status().await;
            for status in statuses {
                if status.status != spiritstream_server::models::ChatConnectionStatus::Error {
                    continue;
                }

                if last_attempts
                    .get(&status.platform)
                    .map(|last| last.elapsed() < std::time::Duration::from_secs(CHAT_RECONNECT_COOLDOWN_SECS))
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
                        connect_twitch_chat(&state.chat_manager, &chat_settings, &profile_settings, &state.event_bus).await;
                    }
                    ChatPlatform::Trovo => {
                        if chat_settings.trovo_channel_id.is_empty() {
                            continue;
                        }
                        connect_trovo_chat(&state.chat_manager, &chat_settings, &state.event_bus).await;
                    }
                    ChatPlatform::YouTube => {
                        let has_oauth = !chat_settings.youtube_use_api_key
                            && !profile_settings.oauth.youtube.access_token.is_empty();
                        let has_api_key = chat_settings.youtube_use_api_key
                            && !chat_settings.youtube_api_key.is_empty();
                        if chat_settings.youtube_channel_id.is_empty() || (!has_oauth && !has_api_key) {
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
