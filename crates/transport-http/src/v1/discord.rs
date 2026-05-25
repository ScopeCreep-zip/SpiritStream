//! Discord webhook handlers — `/api/v1/discord/webhook/*`.
//!
//! Test, send, and cooldown-reset shims over `DiscordWebhookService`.

use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::AppState;

// --------------------------------------------------------------------------
// Discord.

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DiscordWebhookTestRequest {
    pub url: String,
}

#[utoipa::path(post, path = "/discord/webhook/test", tag = "discord",
    request_body = DiscordWebhookTestRequest,
    responses((status = 200, body = serde_json::Value)),
    security(("session_cookie" = []), ("bearer" = [])))]
pub async fn v1_discord_test_webhook_proxy(
    State(state): State<AppState>,
    axum::Json(req): axum::Json<DiscordWebhookTestRequest>,
) -> Result<Json<serde_json::Value>, crate::ApiError> {
    let result = state.discord_service.test_webhook(&req.url).await;
    Ok(Json(serde_json::json!(result)))
}

#[utoipa::path(post, path = "/discord/webhook/send", tag = "discord",
    responses((status = 200, body = serde_json::Value)),
    security(("session_cookie" = []), ("bearer" = [])))]
pub async fn v1_discord_send_notification_proxy(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, crate::ApiError> {
    let profile_settings = state
        .active_profile_settings
        .lock()
        .await
        .clone()
        .ok_or(spiritstream_core::CoreError::NoActiveProfile)?;
    let discord_settings = profile_settings.discord;
    if !discord_settings.webhook_enabled {
        return Ok(Json(serde_json::json!({
            "success": false,
            "message": "Discord webhook is not enabled",
            "skippedCooldown": false
        })));
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
    Ok(Json(serde_json::json!(result)))
}

#[utoipa::path(delete, path = "/discord/webhook/cooldown", tag = "discord",
    responses((status = 200)),
    security(("session_cookie" = []), ("bearer" = [])))]
pub async fn v1_discord_reset_cooldown_proxy(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, crate::ApiError> {
    state.discord_service.reset_cooldown().await;
    Ok(Json(serde_json::Value::Null))
}


