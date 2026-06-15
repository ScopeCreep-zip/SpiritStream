//! Refresh-then-(re)connect helpers shared by auto-connect and the manual
//! retry endpoint. Pulled out of `chat_lifecycle.rs` to keep it under the
//! 600 LOC ceiling.
//!
//! The bug these close: the manual retry path connected with the STORED
//! (possibly expired) token without refreshing, so a read-only Twitch
//! session never recovered. And auto-connect SKIPPED an already-connected
//! platform, so a re-auth never reached the live (read-only) IRC session.
//!
//! `force_readonly` is the safety valve: only explicit re-auth + manual
//! retry pass `true` (swap a read-only session for a send-capable one via
//! `ChatManager::reconnect`). Ambient auto-connect (settings save / profile
//! activation) passes `false` so a genuinely-dead sign-in doesn't churn the
//! connection on every save.

use spiritstream_core::models::{
    ChatConnectionStatus, ChatPlatform, ChatSettings, ProfileSettings,
};

use crate::AppState;

use super::{
    connect_kick_chat, connect_trovo_chat, connect_twitch_chat, ensure_fresh_oauth_token,
    persist_active_profile_settings,
};

/// True when the platform is connected; second bool is `can_send`.
async fn connection_state(state: &AppState, platform: ChatPlatform) -> (bool, bool) {
    match state.chat_manager.get_platform_status(platform).await {
        Some(s) => (s.status == ChatConnectionStatus::Connected, s.can_send),
        None => (false, false),
    }
}

/// Refresh the Twitch token (persisting a rotated refresh token), then
/// connect — or, for a connected-but-read-only session when `force_readonly`,
/// REPLACE it with a send-capable one.
pub(crate) async fn refresh_and_connect_twitch(
    state: &AppState,
    chat_settings: &ChatSettings,
    mut profile_settings: ProfileSettings,
    force_readonly: bool,
) {
    let (connected, can_send) = connection_state(state, ChatPlatform::Twitch).await;
    if connected && can_send {
        return; // already send-capable
    }
    if connected && !can_send && !force_readonly {
        return; // read-only; don't churn on ambient calls
    }

    if !profile_settings.oauth.twitch.access_token.is_empty() {
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
                        persist_active_profile_settings(state, profile_settings.clone()).await
                    {
                        log::warn!("Failed to persist Twitch OAuth refresh: {err}");
                    }
                }
            }
            Err(e) => log::warn!("Twitch token refresh failed, using existing token: {e}"),
        }
    }
    // Replace the live session only when it's currently up (read-only) — a
    // fresh connect otherwise.
    connect_twitch_chat(
        &state.chat_manager,
        chat_settings,
        &profile_settings,
        &state.event_bus,
        connected,
    )
    .await;
}

/// Refresh the Trovo token then connect. Trovo has no read-only-connected
/// fallback (the bearer is captured at connect), so there's no force-replace;
/// an already-connected Trovo is a no-op. The win is refreshing before a
/// manual reconnect so a stale token isn't baked in.
pub(crate) async fn refresh_and_connect_trovo(
    state: &AppState,
    chat_settings: &ChatSettings,
    mut profile_settings: ProfileSettings,
) {
    let (connected, _) = connection_state(state, ChatPlatform::Trovo).await;
    if connected {
        return;
    }
    if !profile_settings.oauth.trovo.access_token.is_empty() {
        if let Ok(fresh) = ensure_fresh_oauth_token(
            "trovo",
            &profile_settings.oauth.trovo.access_token,
            &profile_settings.oauth.trovo.refresh_token,
            profile_settings.oauth.trovo.expires_at,
            &state.oauth_service,
        )
        .await
        {
            if fresh.refreshed {
                profile_settings.oauth.trovo.access_token = fresh.access_token.clone();
                if let Some(rt) = fresh.refresh_token {
                    profile_settings.oauth.trovo.refresh_token = rt;
                }
                profile_settings.oauth.trovo.expires_at = fresh.expires_at;
                if let Err(err) =
                    persist_active_profile_settings(state, profile_settings.clone()).await
                {
                    log::warn!("Failed to persist Trovo OAuth refresh: {err}");
                }
            }
        }
    }
    connect_trovo_chat(
        &state.chat_manager,
        chat_settings,
        &profile_settings,
        state.oauth_service.get_config().await.get_trovo_client_id(),
        &state.event_bus,
    )
    .await;
}

/// Refresh the Kick token then connect. Same shape as Trovo.
pub(crate) async fn refresh_and_connect_kick(
    state: &AppState,
    chat_settings: &ChatSettings,
    mut profile_settings: ProfileSettings,
) {
    let (connected, _) = connection_state(state, ChatPlatform::Kick).await;
    if connected {
        return;
    }
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
                    persist_active_profile_settings(state, profile_settings.clone()).await
                {
                    log::warn!("Failed to persist Kick OAuth refresh: {err}");
                }
            }
        }
    }
    connect_kick_chat(
        &state.chat_manager,
        chat_settings,
        &profile_settings,
        &state.event_bus,
    )
    .await;
}
