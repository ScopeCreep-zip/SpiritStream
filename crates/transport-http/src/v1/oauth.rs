//! OAuth handlers — `/api/v1/oauth/*`.
//!
//! Per-provider (Twitch / YouTube) flow start, completion, refresh,
//! account inspection, disconnect, and total-forget. The handlers
//! delegate to `OAuthService` and to the `chat_lifecycle` profile
//! state helpers in the parent crate.

use axum::{
    extract::{Path as AxumPath, State},
    Json,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use spiritstream_core::services::{OAuthTokens, OAuthUserInfo};

use crate::AppState;

// --------------------------------------------------------------------------
// Wire-mirror types. utoipa is transport-only, so mirror every core
// payload we hand to / accept from the OAuth router instead of leaking
// `ToSchema` into the core crate.

/// `{"configured": bool}` — single-provider variant of the flags response.
#[derive(Serialize, Deserialize, ToSchema)]
pub struct OAuthConfiguredResponse {
    pub configured: bool,
}

/// Empty 200 ack body for handlers whose success payload is just
/// acknowledgement (disconnect / forget / config update). Serialises
/// as `{}`.
#[derive(Serialize, Deserialize, ToSchema)]
pub struct OAuthAckResponse {}

/// `POST /oauth/{provider}/flow` response — a backend-chosen variant.
/// `flow: "redirect"` carries the loopback fields; `flow: "device"`
/// carries the device-code fields. The frontend renders whichever
/// arrives and never decides which grant a provider uses.
#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct OAuthFlowResponse {
    /// `"redirect"` (loopback authorization-code) or `"device"` (RFC
    /// 8628 device code — Twitch's mandated desktop sign-in).
    pub flow: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub callback_port: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
    /// Redirect flows only: whether the server managed to open the
    /// system browser. When false (headless session, missing xdg-open,
    /// sandboxed desktop), the UI surfaces `auth_url` with a copy
    /// affordance instead of telling the user to "check your browser"
    /// for a tab that never opened.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub browser_opened: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verification_uri: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_in: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interval: Option<u64>,
}

impl OAuthFlowResponse {
    pub(super) fn device(
        user_code: String,
        verification_uri: String,
        expires_in: u64,
        interval: u64,
        browser_opened: bool,
    ) -> Self {
        Self {
            flow: "device".into(),
            auth_url: None,
            callback_port: None,
            state: None,
            // Whether the backend (running on the user's machine in the
            // desktop sidecar) managed to open the verification page.
            // The webview itself can't open external URLs — `shell:open`
            // is denied by capability, because it renders chat from
            // strangers — so the open happens server-side, exactly like
            // the loopback flow's `auth_url`.
            browser_opened: Some(browser_opened),
            user_code: Some(user_code),
            verification_uri: Some(verification_uri),
            expires_in: Some(expires_in),
            interval: Some(interval),
        }
    }
}

/// Mirror of [`OAuthUserInfo`] — returned from `POST /oauth/{provider}/complete`
/// after the token exchange + user-info fetch.
#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct OAuthUserInfoResponse {
    pub provider: String,
    pub user_id: String,
    pub username: String,
    pub display_name: String,
}

impl From<OAuthUserInfo> for OAuthUserInfoResponse {
    fn from(u: OAuthUserInfo) -> Self {
        Self {
            provider: u.provider,
            user_id: u.user_id,
            username: u.username,
            display_name: u.display_name,
        }
    }
}

/// Mirror of [`OAuthTokens`] — returned from `POST /oauth/{provider}/refresh`.
/// Token fields are camelCase on the wire (frontend reads `accessToken`,
/// `refreshToken`, `expiresIn`). The core `OAuthTokens` keeps snake_case
/// because it deserialises directly from provider responses (Twitch /
/// Google OAuth token endpoints all use snake_case); this wire mirror
/// renames at the API boundary.
#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct OAuthTokensResponse {
    pub access_token: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_in: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
}

impl From<OAuthTokens> for OAuthTokensResponse {
    fn from(t: OAuthTokens) -> Self {
        Self {
            access_token: t.access_token,
            refresh_token: t.refresh_token,
            expires_in: t.expires_in,
            token_type: t.token_type,
            scope: t.scope,
        }
    }
}

// --------------------------------------------------------------------------
// OAuth handlers.

#[utoipa::path(get, path = "/oauth/{provider}/configured", tag = "oauth",
    params(("provider" = String, Path, description = "OAuth provider")),
    responses((status = 200, body = OAuthConfiguredResponse)),
    security(("session_cookie" = []), ("bearer" = [])))]
pub async fn v1_oauth_is_configured_proxy(
    State(state): State<AppState>,
    AxumPath(provider): AxumPath<String>,
) -> Result<Json<OAuthConfiguredResponse>, crate::ApiError> {
    let configured = state.oauth_service.is_configured(&provider).await;
    Ok(Json(OAuthConfiguredResponse { configured }))
}

#[utoipa::path(post, path = "/oauth/{provider}/flow", tag = "oauth",
    params(("provider" = String, Path, description = "OAuth provider")),
    responses(
        (status = 200, body = OAuthFlowResponse, description = "Auth URL + callback port issued."),
        (status = 409, body = ApiErrorBody, description = "No active profile to bind tokens to."),
        (status = 501, body = ApiErrorBody, description = "Unknown / unsupported provider."),
        (status = 500, body = ApiErrorBody, description = "Internal error starting callback server."),
    ),
    security(("session_cookie" = []), ("bearer" = [])))]
pub async fn v1_oauth_start_flow_proxy(
    State(state): State<AppState>,
    AxumPath(provider): AxumPath<String>,
) -> Result<Json<OAuthFlowResponse>, crate::ApiError> {
    use spiritstream_core::services::{OAuthCallback, OAuthCallbackServer};

    // "This build has no credentials for the provider" outranks "no
    // profile open": the former is unfixable from inside the app, so it
    // must be the error the user sees regardless of session state.
    if !state.oauth_service.is_configured(&provider).await {
        return Err(spiritstream_core::CoreError::OAuthProviderNotConfigured {
            provider: provider.clone(),
        }
        .into());
    }
    if crate::get_active_profile_name(&state).await.is_none() {
        return Err(spiritstream_core::CoreError::NoActiveProfile.into());
    }
    let result = match state.oauth_service.start_flow(&provider).await {
        Ok(result) => result,
        // Core's flow routing: a public client on a provider that
        // refuses PKCE token exchange (Twitch) signs in via the device
        // flow. The transport runs it transparently behind the SAME
        // endpoint — the response's `flow` field tells the UI what to
        // render.
        Err(spiritstream_core::CoreError::OAuthFlowRequiresDevice { .. }) => {
            return super::oauth_device::run_device_flow(&state, &provider)
                .await
                .map(Json);
        }
        Err(other) => return Err(other.into()),
    };

    let (callback_server, mut callback_rx) =
        OAuthCallbackServer::start(result.callback_port).await?;

    let oauth_service = state.oauth_service.clone();
    let state_clone = state.clone();
    let provider_name = provider.clone();

    tokio::spawn(async move {
        let timeout = tokio::time::sleep(std::time::Duration::from_secs(180));
        tokio::pin!(timeout);
        let callback = tokio::select! {
            res = &mut callback_rx => res.ok(),
            _ = &mut timeout => None,
        };
        match callback {
            Some(OAuthCallback::Success {
                code,
                state: cb_state,
            }) => {
                match oauth_service
                    .complete_flow(&provider_name, &code, &cb_state)
                    .await
                {
                    Ok(result) => {
                        let now = chrono::Utc::now().timestamp();
                        let expires_at = result
                            .tokens
                            .expires_in
                            .map(|e| now + e as i64)
                            .unwrap_or(0);
                        match crate::update_profile_oauth_account(
                            &state_clone,
                            &provider_name,
                            result.tokens.access_token.clone(),
                            result.tokens.refresh_token.clone(),
                            expires_at,
                            &result.user_info,
                        )
                        .await
                        {
                            Ok(()) => {
                                use spiritstream_core::services::EventSink;
                                state_clone
                                    .event_bus
                                    .emit("oauth_complete", serde_json::json!(result.user_info));
                            }
                            Err(err) => log::error!("Failed to save OAuth profile settings: {err}"),
                        }
                    }
                    Err(err) => log::error!("OAuth completion failed for {provider_name}: {err}"),
                }
            }
            Some(OAuthCallback::Error { error, description }) => {
                if let Some(d) = description {
                    log::warn!("OAuth callback error for {provider_name}: {error} ({d})");
                } else {
                    log::warn!("OAuth callback error for {provider_name}: {error}");
                }
            }
            None => log::warn!("OAuth callback timed out for {provider_name}"),
        }
        callback_server.shutdown();
    });

    let browser_opened = match opener::open(&result.auth_url) {
        Ok(()) => true,
        Err(e) => {
            log::warn!("Failed to open browser: {}. URL: {}", e, result.auth_url);
            false
        }
    };
    Ok(Json(OAuthFlowResponse {
        flow: "redirect".into(),
        auth_url: Some(result.auth_url),
        callback_port: Some(result.callback_port),
        state: Some(result.state),
        browser_opened: Some(browser_opened),
        user_code: None,
        verification_uri: None,
        expires_in: None,
        interval: None,
    }))
}

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct OAuthCompleteRequest {
    pub code: String,
    pub state: String,
}

#[utoipa::path(post, path = "/oauth/{provider}/complete", tag = "oauth",
    params(("provider" = String, Path, description = "OAuth provider")),
    request_body = OAuthCompleteRequest,
    responses(
        (status = 200, body = OAuthUserInfoResponse, description = "Tokens + user info persisted to active profile."),
        (status = 401, body = ApiErrorBody, description = "OAuth provider rejected the code or returned no user data."),
        (status = 409, body = ApiErrorBody, description = "No active profile to bind tokens to."),
        (status = 501, body = ApiErrorBody, description = "Unknown provider."),
        (status = 502, body = ApiErrorBody, description = "Network failure reaching the provider."),
        (status = 500, body = ApiErrorBody, description = "Internal error persisting tokens."),
    ),
    security(("session_cookie" = []), ("bearer" = [])))]
pub async fn v1_oauth_complete_flow_proxy(
    State(app_state): State<AppState>,
    AxumPath(provider): AxumPath<String>,
    axum::Json(req): axum::Json<OAuthCompleteRequest>,
) -> Result<Json<OAuthUserInfoResponse>, crate::ApiError> {
    if crate::get_active_profile_name(&app_state).await.is_none() {
        return Err(spiritstream_core::CoreError::NoActiveProfile.into());
    }
    let result = app_state
        .oauth_service
        .complete_flow(&provider, &req.code, &req.state)
        .await?;
    let now = chrono::Utc::now().timestamp();
    let expires_at = result
        .tokens
        .expires_in
        .map(|e| now + e as i64)
        .unwrap_or(0);
    crate::update_profile_oauth_account(
        &app_state,
        &provider,
        result.tokens.access_token.clone(),
        result.tokens.refresh_token.clone(),
        expires_at,
        &result.user_info,
    )
    .await?;
    use spiritstream_core::services::EventSink;
    app_state
        .event_bus
        .emit("oauth_complete", serde_json::json!(result.user_info));
    Ok(Json(result.user_info.into()))
}

/// Wire mirror of [`spiritstream_core::models::OAuthAccountStatus`]. The
/// core type itself can't derive `ToSchema` (utoipa is transport-only).
#[derive(Serialize, Deserialize, ToSchema, Default)]
#[serde(rename_all = "camelCase")]
pub struct OAuthAccountStatusResponse {
    pub logged_in: bool,
    pub user_id: String,
    pub username: String,
    pub display_name: String,
}

impl From<spiritstream_core::models::OAuthAccountStatus> for OAuthAccountStatusResponse {
    fn from(s: spiritstream_core::models::OAuthAccountStatus) -> Self {
        Self {
            logged_in: s.logged_in,
            user_id: s.user_id,
            username: s.username,
            display_name: s.display_name,
        }
    }
}

#[utoipa::path(get, path = "/oauth/{provider}/account", tag = "oauth",
    params(("provider" = String, Path, description = "OAuth provider")),
    responses((status = 200, body = OAuthAccountStatusResponse)),
    security(("session_cookie" = []), ("bearer" = [])))]
pub async fn v1_oauth_get_account_proxy(
    State(state): State<AppState>,
    AxumPath(provider): AxumPath<String>,
) -> Result<Json<OAuthAccountStatusResponse>, crate::ApiError> {
    use spiritstream_core::models::OAuthAccountStatus;
    let profile_settings = crate::get_active_profile_settings(&state).await;
    let account = match provider.as_str() {
        "twitch" => profile_settings
            .as_ref()
            .filter(|s| !s.oauth.twitch.username.is_empty())
            .map(|s| OAuthAccountStatus {
                logged_in: true,
                user_id: s.oauth.twitch.user_id.clone(),
                username: s.oauth.twitch.username.clone(),
                display_name: s.oauth.twitch.display_name.clone(),
            })
            .unwrap_or_default(),
        "youtube" => profile_settings
            .as_ref()
            .filter(|s| !s.oauth.youtube.user_id.is_empty())
            .map(|s| OAuthAccountStatus {
                logged_in: true,
                user_id: s.oauth.youtube.user_id.clone(),
                username: s.oauth.youtube.username.clone(),
                display_name: s.oauth.youtube.display_name.clone(),
            })
            .unwrap_or_default(),
        _ => OAuthAccountStatus::default(),
    };
    Ok(Json(account.into()))
}

#[utoipa::path(delete, path = "/oauth/{provider}/account", tag = "oauth",
    params(("provider" = String, Path, description = "OAuth provider")),
    responses((status = 200, body = OAuthAckResponse)),
    security(("session_cookie" = []), ("bearer" = [])))]
pub async fn v1_oauth_disconnect_proxy(
    State(state): State<AppState>,
    AxumPath(provider): AxumPath<String>,
) -> Result<Json<OAuthAckResponse>, crate::ApiError> {
    crate::clear_profile_oauth_account(&state, &provider).await?;
    Ok(Json(OAuthAckResponse {}))
}

/// Result of an OAuth `forget` call.
///
/// `localCleared` is always true on success — the on-device token /
/// refresh token / user-info is wiped from the profile regardless of
/// whether the upstream revoke succeeded. `revokeFailed` is `Some(msg)`
/// when the provider's revoke endpoint refused / errored; the frontend
/// surfaces that so the user knows to also revoke from the provider's
/// own settings page (the upstream token may still be valid until its
/// natural expiry). For harassment-prone users this distinction matters:
/// "I clicked forget and it succeeded" must not mean "attacker's stolen
/// session is now invalidated" if the revoke endpoint was unreachable.
#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct OAuthForgetResponse {
    pub local_cleared: bool,
    pub revoke_failed: Option<String>,
}

#[utoipa::path(post, path = "/oauth/{provider}/forget", tag = "oauth",
    params(("provider" = String, Path, description = "OAuth provider")),
    responses((status = 200, body = OAuthForgetResponse)),
    security(("session_cookie" = []), ("bearer" = [])))]
pub async fn v1_oauth_forget_proxy(
    State(state): State<AppState>,
    AxumPath(provider): AxumPath<String>,
) -> Result<Json<OAuthForgetResponse>, crate::ApiError> {
    let profile_settings = crate::get_active_profile_settings(&state)
        .await
        .ok_or(spiritstream_core::CoreError::NoActiveProfile)?;
    // All four providers must be reachable here — clear_profile_oauth_account
    // already handles each, so the forget surface must too. Pre-fix only
    // twitch + youtube matched, so kick + facebook tokens couldn't be
    // forgotten via this endpoint and stayed on disk until profile delete.
    let token = match provider.as_str() {
        "twitch" => profile_settings.oauth.twitch.access_token,
        "youtube" => profile_settings.oauth.youtube.access_token,
        "kick" => profile_settings.oauth.kick.access_token,
        "facebook" => profile_settings.oauth.facebook.access_token,
        _ => {
            return Err(spiritstream_core::CoreError::NotImplemented {
                feature: format!("Unknown provider: {provider}"),
            }
            .into())
        }
    };
    let mut revoke_failed: Option<String> = None;
    if !token.is_empty() {
        if let Err(e) = state
            .oauth_service
            .revoke_token(&provider, token.as_str())
            .await
        {
            log::warn!("Failed to revoke {provider} token: {e}");
            revoke_failed = Some(format!("{e}"));
        }
    }
    // Always clear local even if revoke failed — getting the token off
    // disk is more important than knowing whether the upstream side
    // also dropped it. The revoke_failed field tells the frontend to
    // surface "also revoke from the provider's settings page."
    crate::clear_profile_oauth_account(&state, &provider).await?;
    Ok(Json(OAuthForgetResponse {
        local_cleared: true,
        revoke_failed,
    }))
}

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct OAuthRefreshRequest {
    pub refresh_token: String,
}

#[utoipa::path(post, path = "/oauth/{provider}/refresh", tag = "oauth",
    params(("provider" = String, Path, description = "OAuth provider")),
    request_body = OAuthRefreshRequest,
    responses(
        (status = 200, body = OAuthTokensResponse, description = "Refreshed token payload."),
        (status = 401, body = ApiErrorBody, description = "Refresh token rejected by provider."),
        (status = 501, body = ApiErrorBody, description = "Unknown provider."),
        (status = 502, body = ApiErrorBody, description = "Network failure reaching the provider."),
        (status = 500, body = ApiErrorBody, description = "Internal error."),
    ),
    security(("session_cookie" = []), ("bearer" = [])))]
pub async fn v1_oauth_refresh_token_proxy(
    State(state): State<AppState>,
    AxumPath(provider): AxumPath<String>,
    axum::Json(req): axum::Json<OAuthRefreshRequest>,
) -> Result<Json<OAuthTokensResponse>, crate::ApiError> {
    let tokens = state
        .oauth_service
        .refresh_token(&provider, &req.refresh_token)
        .await?;
    Ok(Json(tokens.into()))
}
