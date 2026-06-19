//! Transport-level active-profile snapshot + OAuth account mutators.
//!
//! K3 split: pulled out of `chat_lifecycle.rs` so the orchestrator stays
//! under the 600 LOC ceiling. Every function operates on `AppState`'s
//! cached active-profile fields and the on-disk profile JSON.

use spiritstream_core::models::{Profile, ProfileSettings};

use crate::AppState;

/// Hydrate session-scoped transport state (`active_profile_name`,
/// `active_profile_settings`, `active_profile_pii`) and push the
/// anonymous-mode policy into `ChatManager` after a profile activates.
/// The `profile_activated` event is emitted by `ProfileActivationService`
/// itself before this runs, so both transports see identical bus shape
/// without re-emission here.
pub(crate) async fn set_active_profile(state: &AppState, profile: &Profile) {
    {
        let mut guard = state.active_profile_name.lock().await;
        // Switching to a DIFFERENT profile is a fresh slate: drop any
        // deliberate-disconnect intent so the new profile's platforms
        // are free to auto-connect. A same-profile re-activation
        // (settings save) intentionally leaves intent untouched, so a
        // panic / Disconnect survives saving settings.
        let switched = guard.as_deref() != Some(profile.name.as_str());
        *guard = Some(profile.name.clone());
        if switched {
            state.chat_manager.clear_all_disconnect_intent().await;
        }
    }
    {
        let mut guard = state.active_profile_settings.lock().await;
        *guard = Some(profile.settings.clone());
    }
    {
        let mut guard = state.active_profile_pii.lock().await;
        *guard = Some((profile.pii_blocklist.clone(), profile.pii_fuzzy));
    }
    // Push the anonymous-mode policy into ChatManager so subsequent
    // inbound chat messages get pseudonymised before they reach the
    // log writer or the event stream. Core's `ProfileActivationService`
    // already set (and validated) this policy during `activate()`; this
    // re-push covers HTTP-only hydration paths (startup auto-load). An
    // invalid salt here is a bug upstream — log loudly, never ignore.
    if let Err(e) = state
        .chat_manager
        .set_anonymous_policy(profile.anonymous_logging, profile.anonymous_salt.clone())
    {
        log::error!(
            "failed to apply anonymous-mode policy for profile '{}': {e}",
            profile.name
        );
    }
}

/// Clear all session-scoped active-profile state — the transport-side
/// inverse of [`set_active_profile`], run after
/// `ProfileActivationService::deactivate` has scrubbed core state. Drops
/// the cached name / settings / PII snapshot so nothing from the signed-out
/// profile is served, and clears disconnect-intent so the next activation
/// starts from a clean slate.
pub(crate) async fn clear_active_profile(state: &AppState) {
    {
        let mut guard = state.active_profile_name.lock().await;
        *guard = None;
    }
    {
        let mut guard = state.active_profile_settings.lock().await;
        *guard = None;
    }
    {
        let mut guard = state.active_profile_pii.lock().await;
        *guard = None;
    }
    state.chat_manager.clear_all_disconnect_intent().await;
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

    let account = oauth_account_mut(&mut profile_settings.oauth, provider)?;
    account.access_token = access_token;
    if let Some(rt) = refresh_token {
        account.refresh_token = rt;
    }
    account.expires_at = expires_at;
    account.user_id = user_info.user_id.clone();
    account.username = user_info.username.clone();
    account.display_name = user_info.display_name.clone();

    seed_chat_identity_from_oauth(provider, &mut profile_settings, user_info);

    persist_active_profile_settings(state, profile_settings.clone()).await?;

    // Sign-in is a deliberate "I want this platform" — connect it now,
    // independent of streaming. Push the (possibly channel-defaulted)
    // chat settings into the manager, clear any stale disconnect intent
    // for this provider, then auto-connect (idempotent; skips already-
    // connected and intent-disconnected platforms).
    state
        .chat_manager
        .update_profile_chat_settings(profile_settings.chat.clone())
        .await;
    if let Some(platform) = chat_platform_for(provider) {
        state.chat_manager.clear_disconnect_intent(platform).await;
    }
    // Re-auth: force a reconnect so a live read-only session (e.g. Twitch
    // anonymous fallback) is swapped for a send-capable one with the new
    // token — auto_connect otherwise skips an "already connected" platform.
    tokio::spawn(crate::auto_connect_chat_platforms(state.clone(), true));

    Ok(())
}

fn seed_chat_identity_from_oauth(
    provider: &str,
    profile_settings: &mut ProfileSettings,
    user_info: &spiritstream_core::services::OAuthUserInfo,
) {
    match provider {
        // Twitch sign-in already gives the exact chat login.
        "twitch" if profile_settings.chat.twitch_channel.trim().is_empty() => {
            profile_settings.chat.twitch_channel = user_info.username.clone();
        }
        // YouTube chat lookup needs a stable channel identifier. The OAuth
        // profile title is for display only; the channel id is the safe
        // backend default when the user hasn't already entered an `@handle`
        // or channel id themselves.
        "youtube" if profile_settings.chat.youtube_channel_id.trim().is_empty() => {
            profile_settings.chat.youtube_channel_id = user_info.user_id.clone();
        }
        _ => {}
    }
}

/// Provider name → the `ChatPlatform` whose chat we auto-connect on
/// sign-in. Facebook is intentionally absent — it never auto-connects
/// (identity-revealing connect needs an explicit confirm-token click).
fn chat_platform_for(provider: &str) -> Option<spiritstream_core::models::ChatPlatform> {
    use spiritstream_core::models::ChatPlatform;
    match provider {
        "twitch" => Some(ChatPlatform::Twitch),
        "youtube" => Some(ChatPlatform::YouTube),
        "kick" => Some(ChatPlatform::Kick),
        "trovo" => Some(ChatPlatform::Trovo),
        _ => None,
    }
}

/// Provider-name → mutable account slot. One lookup shared by the
/// update + clear paths so adding a provider is a one-line change
/// (the old per-provider copy-paste blocks silently skipped new
/// providers).
fn oauth_account_mut<'a>(
    oauth: &'a mut spiritstream_core::models::OAuthSettings,
    provider: &str,
) -> Result<&'a mut spiritstream_core::models::OAuthAccount, spiritstream_core::CoreError> {
    match provider {
        "twitch" => Ok(&mut oauth.twitch),
        "youtube" => Ok(&mut oauth.youtube),
        "kick" => Ok(&mut oauth.kick),
        "facebook" => Ok(&mut oauth.facebook),
        "trovo" => Ok(&mut oauth.trovo),
        _ => Err(spiritstream_core::CoreError::NotImplemented {
            feature: format!("Unknown provider: {provider}"),
        }),
    }
}

pub(crate) async fn clear_profile_oauth_account(
    state: &AppState,
    provider: &str,
) -> Result<(), spiritstream_core::CoreError> {
    let mut profile_settings = get_active_profile_settings(state)
        .await
        .ok_or(spiritstream_core::CoreError::NoActiveProfile)?;

    *oauth_account_mut(&mut profile_settings.oauth, provider)? = Default::default();

    persist_active_profile_settings(state, profile_settings).await
}

#[cfg(test)]
mod tests {
    use super::seed_chat_identity_from_oauth;
    use spiritstream_core::models::ProfileSettings;
    use spiritstream_core::services::OAuthUserInfo;

    fn user_info(provider: &str, user_id: &str, username: &str, display_name: &str) -> OAuthUserInfo {
        OAuthUserInfo {
            provider: provider.to_string(),
            user_id: user_id.to_string(),
            username: username.to_string(),
            display_name: display_name.to_string(),
        }
    }

    #[test]
    fn twitch_seed_uses_oauth_username_when_channel_blank() {
        let mut settings = ProfileSettings::default();
        seed_chat_identity_from_oauth(
            "twitch",
            &mut settings,
            &user_info("twitch", "123", "streamer_login", "Streamer"),
        );
        assert_eq!(settings.chat.twitch_channel, "streamer_login");
    }

    #[test]
    fn youtube_seed_uses_channel_id_when_field_blank() {
        let mut settings = ProfileSettings::default();
        seed_chat_identity_from_oauth(
            "youtube",
            &mut settings,
            &user_info("youtube", "UC123456789", "Channel Title", "Channel Title"),
        );
        assert_eq!(settings.chat.youtube_channel_id, "UC123456789");
    }

    #[test]
    fn youtube_seed_preserves_user_entered_handle() {
        let mut settings = ProfileSettings::default();
        settings.chat.youtube_channel_id = "@already-set".to_string();
        seed_chat_identity_from_oauth(
            "youtube",
            &mut settings,
            &user_info("youtube", "UC123456789", "Channel Title", "Channel Title"),
        );
        assert_eq!(settings.chat.youtube_channel_id, "@already-set");
    }
}
