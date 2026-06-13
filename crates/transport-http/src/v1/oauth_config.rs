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

/// One field the user pastes/selects in the provider's dev console.
#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct OAuthConsoleFieldWire {
    pub label: String,
    pub value: String,
    pub copyable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// Pre-filled guided-setup walkthrough for registering an app.
#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct OAuthProviderSetupWire {
    pub steps: Vec<String>,
    pub console_fields: Vec<OAuthConsoleFieldWire>,
}

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
    /// Pre-filled values + steps for the in-app guided setup.
    pub setup: OAuthProviderSetupWire,
}

impl From<spiritstream_core::services::OAuthConsoleField> for OAuthConsoleFieldWire {
    fn from(f: spiritstream_core::services::OAuthConsoleField) -> Self {
        Self {
            label: f.label,
            value: f.value,
            copyable: f.copyable,
            note: f.note,
        }
    }
}

impl From<spiritstream_core::services::OAuthProviderSetup> for OAuthProviderSetupWire {
    fn from(s: spiritstream_core::services::OAuthProviderSetup) -> Self {
        Self {
            steps: s.steps,
            console_fields: s.console_fields.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<OAuthProviderSummary> for OAuthProviderSummaryWire {
    fn from(s: OAuthProviderSummary) -> Self {
        Self {
            provider: s.provider,
            configured: s.configured,
            needs_secret: s.needs_secret,
            override_client_id: s.override_client_id,
            registration_url: s.registration_url,
            setup: s.setup.into(),
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
    let hints = channel_hints(&state).await;
    let summaries = state.oauth_service.provider_summaries(&hints).await;
    Ok(Json(summaries.into_iter().map(Into::into).collect()))
}

/// Pull the channel/username the user already entered for each platform
/// from the active profile, so the guided setup can pre-fill a unique
/// app name (e.g. "SpiritStream - <channel>"). Absent profile / empty
/// fields just yield a generic suggestion — never an error.
async fn channel_hints(state: &AppState) -> std::collections::HashMap<String, String> {
    let mut hints = std::collections::HashMap::new();
    let Some(settings) = crate::get_active_profile_settings(state).await else {
        return hints;
    };
    let chat = &settings.chat;
    let mut insert = |provider: &str, value: &str| {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            hints.insert(provider.to_string(), trimmed.to_string());
        }
    };
    insert("twitch", &chat.twitch_channel);
    insert("youtube", &chat.youtube_channel_id);
    insert("kick", &chat.kick_channel);
    insert("trovo", &chat.trovo_channel_id);
    insert("facebook", &chat.facebook_live_video_id);
    hints
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
    let hints = channel_hints(&state).await;
    let summaries = state.oauth_service.provider_summaries(&hints).await;
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
