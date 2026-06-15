//! Twitch-specific chat lifecycle: the background OAuth token refresh task.
//!
//! Twitch user access tokens expire after ~4 hours. The device code flow
//! DOES issue a `refresh_token` (Twitch docs), which the app captures and
//! rotates on activation + connect — but without a proactive timer a session
//! running past 4 hours lets the token lapse and chat drops to anonymous
//! read-only. This mirrors `start_youtube_token_refresh_task`: refresh just
//! before expiry, persist the rotated (one-time-use) refresh token, and push
//! the fresh access token to the connector so the next (re)connect is valid.

use std::time::Duration;

use spiritstream_core::models::ChatPlatform;
use spiritstream_core::services::EventSink;

use crate::AppState;

use super::{
    ensure_fresh_oauth_token, get_active_profile_settings, persist_active_profile_settings,
};

/// Background loop that keeps the active profile's Twitch OAuth token fresh
/// while Twitch chat is connected. Fire-and-forget; errors are logged.
pub(crate) async fn start_twitch_token_refresh_task(state: AppState) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(60));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        // Tick counter drives Twitch's mandated token-validation cadence:
        // once on session start (tick 1) and hourly thereafter (every 60th
        // 60s tick). Separate from expiry-based refresh above — `/validate`
        // catches server-side invalidation (revoked access / password
        // change) that `expires_at` can't see.
        let mut ticks: u64 = 0;

        loop {
            interval.tick().await;
            ticks = ticks.saturating_add(1);

            let is_connected = state
                .chat_manager
                .get_platform_status(ChatPlatform::Twitch)
                .await
                .map(|s| s.status == spiritstream_core::models::ChatConnectionStatus::Connected)
                .unwrap_or(false);

            if !is_connected {
                continue;
            }

            let mut profile_settings = match get_active_profile_settings(&state).await {
                Some(s) => s,
                None => {
                    log::warn!("Twitch token refresh: no active profile settings");
                    continue;
                }
            };

            if profile_settings.oauth.twitch.access_token.is_empty()
                || profile_settings.oauth.twitch.refresh_token.is_empty()
                || profile_settings.oauth.twitch.expires_at <= 0
            {
                continue;
            }

            let previous_token = profile_settings.oauth.twitch.access_token.clone();
            match ensure_fresh_oauth_token(
                "twitch",
                &previous_token,
                &profile_settings.oauth.twitch.refresh_token,
                profile_settings.oauth.twitch.expires_at,
                &state.oauth_service,
            )
            .await
            {
                Ok(fresh) => {
                    if fresh.refreshed {
                        profile_settings.oauth.twitch.access_token = fresh.access_token.clone();
                        // Twitch refresh tokens are one-time-use — persist the
                        // rotated token immediately or the next refresh fails.
                        if let Some(rt) = fresh.refresh_token {
                            profile_settings.oauth.twitch.refresh_token = rt;
                        }
                        profile_settings.oauth.twitch.expires_at = fresh.expires_at;
                        if let Err(err) =
                            persist_active_profile_settings(&state, profile_settings.clone()).await
                        {
                            log::warn!("Failed to persist Twitch OAuth refresh: {err}");
                        }
                    }
                    if fresh.access_token != previous_token {
                        if let Err(e) = state
                            .chat_manager
                            .update_platform_token(ChatPlatform::Twitch, fresh.access_token)
                            .await
                        {
                            log::warn!("Failed to update Twitch chat token: {e}");
                        } else {
                            log::info!("Twitch chat token refreshed and updated");
                        }
                    }
                }
                Err(e) => {
                    log::warn!("Twitch token refresh failed: {e}");
                }
            }

            // Twitch policy: validate the token on session start and hourly.
            // A 401 means it was invalidated server-side even if `expires_at`
            // is in the future — per Twitch's docs the app MUST end every
            // session using the token. We disconnect Twitch chat and emit
            // `oauth_token_expired` so the UI surfaces the re-auth prompt
            // (reusing the same event the proactive-refresh failure path uses).
            if ticks == 1 || ticks % 60 == 0 {
                let token = profile_settings.oauth.twitch.access_token.clone();
                match state.oauth_service.validate_twitch_token(&token).await {
                    Ok(()) => {}
                    Err(spiritstream_core::CoreError::Unauthorized) => {
                        log::warn!(
                            "Twitch token failed /validate (401) — ending session per Twitch policy"
                        );
                        if let Err(e) = state.chat_manager.disconnect(ChatPlatform::Twitch).await {
                            log::warn!("Twitch disconnect after failed validation: {e}");
                        }
                        state.event_bus.emit(
                            "oauth_token_expired",
                            serde_json::json!({ "provider": "twitch" }),
                        );
                    }
                    Err(e) => {
                        log::debug!("Twitch token validation transient error (will retry): {e}");
                    }
                }
            }
        }
    });
}
