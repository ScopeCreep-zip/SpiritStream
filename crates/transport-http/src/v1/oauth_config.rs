//! OAuth client-credential configuration surface.
//!
//! `GET /oauth/config` returns per-provider setup summaries (configured
//! flag, whether a secret is needed, the active client-id override, the
//! provider's registration portal) — everything the in-app "Set up
//! sign-in" form renders, so the frontend holds zero provider knowledge.
//! `PUT /oauth/config/{provider}` stores one provider's credentials
//! (the form's save button); `PUT /oauth/config` is the full-replace
//! admin/CLI mirror. Both persist through core's SecretStore so setup
//! done in the UI survives restarts.

use axum::extract::{Path as AxumPath, State};
use axum::Json;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use spiritstream_core::services::{OAuthConfig, OAuthProviderSummary};

use crate::AppState;

use super::oauth::OAuthAckResponse;

/// One provider's setup state — array element of `GET /oauth/config`.
#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct OAuthProviderSummaryWire {
    /// `twitch` | `youtube` | `kick` | `facebook` | `trovo`.
    pub provider: String,
    /// Real credentials present (user-entered, env, or release-embedded).
    pub configured: bool,
    /// Whether the credentials form must collect a client secret.
    pub needs_secret: bool,
    /// The stored client-id override, when the user entered one.
    /// Secrets never appear on this surface.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub override_client_id: Option<String>,
    /// The provider's developer-portal page for registering an app.
    pub registration_url: String,
}

impl From<OAuthProviderSummary> for OAuthProviderSummaryWire {
    fn from(s: OAuthProviderSummary) -> Self {
        Self {
            provider: s.provider,
            configured: s.configured,
            needs_secret: s.needs_secret,
            override_client_id: s.override_client_id,
            registration_url: s.registration_url,
        }
    }
}

/// Body of `PUT /oauth/config/{provider}` — the in-app setup form.
/// Empty / absent fields clear the stored override for that field.
#[derive(Serialize, Deserialize, ToSchema, Default)]
#[serde(rename_all = "camelCase")]
pub struct OAuthProviderCredentialsRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_secret: Option<String>,
}

/// Mirror of [`OAuthConfig`] — accepted by the full-replace
/// `PUT /oauth/config` and surfaced in OpenAPI instead of the previous
/// `serde_json::Value`.
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trovo_client_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trovo_client_secret: Option<String>,
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
            trovo_client_id: r.trovo_client_id,
            trovo_client_secret: r.trovo_client_secret,
        }
    }
}

#[utoipa::path(get, path = "/oauth/config", tag = "oauth",
    responses((status = 200, body = [OAuthProviderSummaryWire])),
    security(("session_cookie" = []), ("bearer" = [])))]
pub async fn v1_oauth_get_config_proxy(
    State(state): State<AppState>,
) -> Result<Json<Vec<OAuthProviderSummaryWire>>, crate::ApiError> {
    // Truthful per-provider state from the live config (placeholder
    // detection in core). The UI renders unconfigured providers with
    // the in-app setup form — these flags hardcoding `true` was half
    // of the dead-link bug.
    let summaries = state.oauth_service.provider_summaries().await;
    Ok(Json(summaries.into_iter().map(Into::into).collect()))
}

#[utoipa::path(put, path = "/oauth/config/{provider}", tag = "oauth",
    params(("provider" = String, Path, description = "OAuth provider")),
    request_body = OAuthProviderCredentialsRequest,
    responses(
        (status = 200, body = [OAuthProviderSummaryWire], description = "Credentials stored; updated summaries returned."),
        (status = 501, body = ApiErrorBody, description = "Unknown provider."),
    ),
    security(("session_cookie" = []), ("bearer" = [])))]
pub async fn v1_oauth_set_provider_credentials_proxy(
    State(state): State<AppState>,
    AxumPath(provider): AxumPath<String>,
    axum::Json(req): axum::Json<OAuthProviderCredentialsRequest>,
) -> Result<Json<Vec<OAuthProviderSummaryWire>>, crate::ApiError> {
    state
        .oauth_service
        .set_provider_credentials(&provider, req.client_id, req.client_secret)
        .await?;
    let summaries = state.oauth_service.provider_summaries().await;
    Ok(Json(summaries.into_iter().map(Into::into).collect()))
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
    state.oauth_service.update_config(req.into()).await?;
    Ok(Json(OAuthAckResponse {}))
}
