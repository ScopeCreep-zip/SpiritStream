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

use spiritstream_core::services::{
    OAuthConfig, OAuthFlowResult, OAuthTokens, OAuthUserInfo,
};

use crate::AppState;

// --------------------------------------------------------------------------
// Wire-mirror types. utoipa is transport-only, so mirror every core
// payload we hand to / accept from the OAuth router instead of leaking
// `ToSchema` into the core crate.

/// `{"twitchConfigured": …, "youtubeConfigured": …}` — pre-flight check
/// the UI runs before showing "Sign in with Twitch / YouTube" buttons.
#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct OAuthConfiguredFlagsResponse {
    pub twitch_configured: bool,
    pub youtube_configured: bool,
    pub kick_configured: bool,
    pub facebook_configured: bool,
}

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

/// Mirror of [`OAuthConfig`] — accepted by `PUT /oauth/config` and
/// surfaced in OpenAPI instead of the previous `serde_json::Value`.
#[derive(Serialize, Deserialize, ToSchema, Default)]
#[serde(rename_all = "camelCase")]
pub struct OAuthConfigRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub twitch_client_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub twitch_client_secret: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub youtube_client_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub youtube_client_secret: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kick_client_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kick_client_secret: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub facebook_client_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub facebook_client_secret: Option<String>,
}

impl From<OAuthConfigRequest> for OAuthConfig {
    fn from(r: OAuthConfigRequest) -> Self {
        Self {
            twitch_client_id: r.twitch_client_id,
            twitch_client_secret: r.twitch_client_secret,
            youtube_client_id: r.youtube_client_id,
            youtube_client_secret: r.youtube_client_secret,
            kick_client_id: r.kick_client_id,
            kick_client_secret: r.kick_client_secret,
            facebook_client_id: r.facebook_client_id,
            facebook_client_secret: r.facebook_client_secret,
        }
    }
}

/// Mirror of [`OAuthFlowResult`] — `POST /oauth/{provider}/flow` returns
/// the authorization URL + bound callback port + PKCE state nonce.
#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct OAuthFlowResponse {
    pub auth_url: String,
    pub callback_port: u16,
    pub state: String,
}

impl From<OAuthFlowResult> for OAuthFlowResponse {
    fn from(r: OAuthFlowResult) -> Self {
        Self {
            auth_url: r.auth_url,
            callback_port: r.callback_port,
            state: r.state,
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

#[utoipa::path(get, path = "/oauth/config", tag = "oauth",
    responses((status = 200, body = OAuthConfiguredFlagsResponse)),
    security(("session_cookie" = []), ("bearer" = [])))]
pub async fn v1_oauth_get_config_proxy(
    State(_state): State<AppState>,
) -> Result<Json<OAuthConfiguredFlagsResponse>, crate::ApiError> {
    Ok(Json(OAuthConfiguredFlagsResponse {
        twitch_configured: true,
        youtube_configured: true,
        kick_configured: true,
        facebook_configured: true,
    }))
}

#[utoipa::path(put, path = "/oauth/config", tag = "oauth",
    request_body = OAuthConfigRequest,
    responses(
        (status = 200, body = OAuthAckResponse, description = "OAuth config persisted."),
        (status = 400, body = ApiErrorBody, description = "Malformed OAuthConfig payload."),
    ),
    security(("session_cookie" = []), ("bearer" = [])))]
pub async fn v1_oauth_set_config_proxy(
    State(state): State<AppState>,
    axum::Json(req): axum::Json<OAuthConfigRequest>,
) -> Result<Json<OAuthAckResponse>, crate::ApiError> {
    state.oauth_service.update_config(req.into()).await;
    Ok(Json(OAuthAckResponse {}))
}

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

    if crate::get_active_profile_name(&state).await.is_none() {
        return Err(spiritstream_core::CoreError::NoActiveProfile.into());
    }
    let result = state.oauth_service.start_flow(&provider).await?;

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
            Some(OAuthCallback::ImplicitSuccess {
                access_token,
                state: _,
            }) => {
                log::info!("Implicit OAuth flow completed for {provider_name}");
                let user_info_result: Result<spiritstream_core::services::OAuthUserInfo, String> =
                    match provider_name.as_str() {
                        "twitch" => oauth_service
                            .fetch_twitch_user(&access_token)
                            .await
                            .map(|u| spiritstream_core::services::OAuthUserInfo {
                                provider: "twitch".to_string(),
                                user_id: u.id,
                                username: u.login,
                                display_name: u.display_name,
                            })
                            .map_err(|e| e.to_string()),
                        _ => Err("Implicit flow not supported for this provider".to_string()),
                    };
                match user_info_result {
                    Ok(user_info) => {
                        match crate::update_profile_oauth_account(
                            &state_clone,
                            &provider_name,
                            access_token,
                            None,
                            0,
                            &user_info,
                        )
                        .await
                        {
                            Ok(()) => {
                                use spiritstream_core::services::EventSink;
                                state_clone
                                    .event_bus
                                    .emit("oauth_complete", serde_json::json!(user_info));
                            }
                            Err(err) => log::error!("Failed to save OAuth profile settings: {err}"),
                        }
                    }
                    Err(err) => log::error!("Failed to fetch user info for {provider_name}: {err}"),
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

    if let Err(e) = opener::open(&result.auth_url) {
        log::warn!("Failed to open browser: {}. URL: {}", e, result.auth_url);
    }
    Ok(Json(result.into()))
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

#[utoipa::path(post, path = "/oauth/{provider}/forget", tag = "oauth",
    params(("provider" = String, Path, description = "OAuth provider")),
    responses((status = 200, body = OAuthAckResponse)),
    security(("session_cookie" = []), ("bearer" = [])))]
pub async fn v1_oauth_forget_proxy(
    State(state): State<AppState>,
    AxumPath(provider): AxumPath<String>,
) -> Result<Json<OAuthAckResponse>, crate::ApiError> {
    let profile_settings = crate::get_active_profile_settings(&state)
        .await
        .ok_or(spiritstream_core::CoreError::NoActiveProfile)?;
    let token = match provider.as_str() {
        "twitch" => profile_settings.oauth.twitch.access_token,
        "youtube" => profile_settings.oauth.youtube.access_token,
        _ => {
            return Err(spiritstream_core::CoreError::NotImplemented {
                feature: format!("Unknown provider: {provider}"),
            }
            .into())
        }
    };
    if !token.is_empty() {
        if let Err(e) = state
            .oauth_service
            .revoke_token(&provider, token.as_str())
            .await
        {
            log::warn!("Failed to revoke {provider} token: {e}");
        }
    }
    crate::clear_profile_oauth_account(&state, &provider).await?;
    Ok(Json(OAuthAckResponse {}))
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
