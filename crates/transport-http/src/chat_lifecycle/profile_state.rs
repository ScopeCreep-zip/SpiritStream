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

    persist_active_profile_settings(state, profile_settings).await
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
