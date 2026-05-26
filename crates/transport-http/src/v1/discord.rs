//! Discord webhook handlers — `/api/v1/discord/webhook/*`.
//!
//! Test, send, and cooldown-reset shims over `DiscordWebhookService`.

use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use spiritstream_core::services::WebhookResult;

use crate::AppState;

// --------------------------------------------------------------------------
// Wire-mirror types. utoipa is transport-only, so mirror the core
// `WebhookResult` here.

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct WebhookResultResponse {
    pub success: bool,
    pub message: String,
    pub skipped_cooldown: bool,
}

impl From<WebhookResult> for WebhookResultResponse {
    fn from(r: WebhookResult) -> Self {
        Self {
            success: r.success,
            message: r.message,
            skipped_cooldown: r.skipped_cooldown,
        }
    }
}

/// Empty 200 ack body for `DELETE /discord/webhook/cooldown`. Serialises
/// as `{}`.
#[derive(Serialize, Deserialize, ToSchema)]
pub struct DiscordAckResponse {}

// --------------------------------------------------------------------------
// Discord.

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DiscordWebhookTestRequest {
    pub url: String,
}

#[utoipa::path(post, path = "/discord/webhook/test", tag = "discord",
    request_body = DiscordWebhookTestRequest,
    responses((status = 200, body = WebhookResultResponse)),
    security(("session_cookie" = []), ("bearer" = [])))]
pub async fn v1_discord_test_webhook_proxy(
    State(state): State<AppState>,
    axum::Json(req): axum::Json<DiscordWebhookTestRequest>,
) -> Result<Json<WebhookResultResponse>, crate::ApiError> {
    let result = state.discord_service.test_webhook(&req.url).await;
    Ok(Json(result.into()))
}

#[utoipa::path(post, path = "/discord/webhook/send", tag = "discord",
    responses((status = 200, body = WebhookResultResponse)),
    security(("session_cookie" = []), ("bearer" = [])))]
pub async fn v1_discord_send_notification_proxy(
    State(state): State<AppState>,
) -> Result<Json<WebhookResultResponse>, crate::ApiError> {
    let profile_settings = state
        .active_profile_settings
        .lock()
        .await
        .clone()
        .ok_or(spiritstream_core::CoreError::NoActiveProfile)?;
    let discord_settings = profile_settings.discord;
    if !discord_settings.webhook_enabled {
        return Ok(Json(WebhookResultResponse {
            success: false,
            message: "Discord webhook is not enabled".to_string(),
            skipped_cooldown: false,
        }));
    }
    let image_path = if discord_settings.image_path.is_empty() {
        None
    } else {
        Some(discord_settings.image_path.as_str())
    };
    let result = state
        .discord_service
        .send_go_live_notification(
            &discord_settings.webhook_url,
            &discord_settings.go_live_message,
            image_path,
            discord_settings.cooldown_enabled,
            discord_settings.cooldown_seconds,
        )
        .await;
    Ok(Json(result.into()))
}

#[utoipa::path(delete, path = "/discord/webhook/cooldown", tag = "discord",
    responses((status = 200, body = DiscordAckResponse)),
    security(("session_cookie" = []), ("bearer" = [])))]
pub async fn v1_discord_reset_cooldown_proxy(
    State(state): State<AppState>,
) -> Result<Json<DiscordAckResponse>, crate::ApiError> {
    state.discord_service.reset_cooldown().await;
    Ok(Json(DiscordAckResponse {}))
}
