use spiritstream_server::models::ChatPlatform;

use crate::state::{ensure_fresh_oauth_token, get_active_profile_settings, persist_active_profile_settings, AppState};

/// Background task to refresh YouTube OAuth tokens and update the live chat connector.
pub(crate) async fn start_youtube_token_refresh_task(
    state: AppState,
) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        loop {
            interval.tick().await;

            let is_connected = state.chat_manager
                .get_platform_status(ChatPlatform::YouTube)
                .await
                .map(|s| s.status == spiritstream_server::models::ChatConnectionStatus::Connected)
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
                        if let Err(err) = persist_active_profile_settings(&state, profile_settings.clone()).await {
                            log::warn!("Failed to persist YouTube OAuth refresh: {err}");
                        }
                    }
                    if fresh.access_token != previous_token {
                        if let Err(e) = state.chat_manager
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
