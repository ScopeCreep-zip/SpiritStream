//! OBS WebSocket handlers — `/api/v1/obs/*`.
//!
//! Connection lifecycle, state snapshot, config get/set, and the
//! OBS stream start/stop pair. Thin shims over `ObsWebSocketHandler`.

use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::AppState;

// --------------------------------------------------------------------------
// OBS.

#[utoipa::path(get, path = "/obs/state", tag = "obs",
    responses((status = 200, body = serde_json::Value)),
    security(("session_cookie" = []), ("bearer" = [])))]
pub async fn v1_obs_state_proxy(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, crate::ApiError> {
    let obs_state = state.obs_handler.get_state().await;
    Ok(Json(serde_json::json!(obs_state)))
}

#[utoipa::path(get, path = "/obs/config", tag = "obs",
    responses((status = 200, body = serde_json::Value)),
    security(("session_cookie" = []), ("bearer" = [])))]
pub async fn v1_obs_get_config_proxy(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, crate::ApiError> {
    let config = state.obs_handler.get_decrypted_config().await?;
    Ok(Json(serde_json::json!(config)))
}

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ObsSetConfigRequest {
    pub host: String,
    pub port: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
    pub use_auth: bool,
    pub direction: String,
    pub auto_connect: bool,
}

#[utoipa::path(put, path = "/obs/config", tag = "obs",
    request_body = ObsSetConfigRequest,
    responses(
        (status = 200, description = "OBS config persisted."),
        (status = 500, body = ApiErrorBody, description = "Internal error encrypting password."),
    ),
    security(("session_cookie" = []), ("bearer" = [])))]
pub async fn v1_obs_set_config_proxy(
    State(state): State<AppState>,
    axum::Json(req): axum::Json<ObsSetConfigRequest>,
) -> Result<Json<serde_json::Value>, crate::ApiError> {
    use spiritstream_core::services::IntegrationDirection;
    let current_config = state.obs_handler.get_config().await;
    let encrypted_password = if let Some(ref pass) = req.password {
        if pass.is_empty() {
            String::new()
        } else {
            state.obs_handler.encrypt_password(pass)?
        }
    } else {
        current_config.password
    };
    let dir = match req.direction.as_str() {
        "obs-to-spiritstream" => IntegrationDirection::ObsToSpiritstream,
        "spiritstream-to-obs" => IntegrationDirection::SpiritstreamToObs,
        "bidirectional" => IntegrationDirection::Bidirectional,
        _ => IntegrationDirection::Disabled,
    };
    let config = spiritstream_core::services::ObsConfig {
        host: req.host,
        port: req.port,
        password: encrypted_password,
        use_auth: req.use_auth,
        direction: dir,
        auto_connect: req.auto_connect,
    };
    state.obs_handler.set_config(config).await;
    Ok(Json(serde_json::Value::Null))
}

#[utoipa::path(get, path = "/obs/connection", tag = "obs",
    responses((status = 200, body = serde_json::Value)),
    security(("session_cookie" = []), ("bearer" = [])))]
pub async fn v1_obs_is_connected_proxy(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, crate::ApiError> {
    Ok(Json(serde_json::json!(
        state.obs_handler.is_connected().await
    )))
}

#[utoipa::path(post, path = "/obs/connection", tag = "obs",
    responses(
        (status = 200, description = "Connected to OBS WebSocket."),
        (status = 502, body = ApiErrorBody, description = "Network failure reaching OBS."),
        (status = 500, body = ApiErrorBody, description = "Internal error during connect."),
    ),
    security(("session_cookie" = []), ("bearer" = [])))]
pub async fn v1_obs_connect_proxy(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, crate::ApiError> {
    state.obs_handler.connect(state.event_bus.clone()).await?;
    Ok(Json(serde_json::Value::Null))
}

#[utoipa::path(delete, path = "/obs/connection", tag = "obs",
    responses((status = 200)),
    security(("session_cookie" = []), ("bearer" = [])))]
pub async fn v1_obs_disconnect_proxy(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, crate::ApiError> {
    state
        .obs_handler
        .disconnect(state.event_bus.clone())
        .await?;
    Ok(Json(serde_json::Value::Null))
}

#[utoipa::path(post, path = "/obs/stream", tag = "obs",
    responses((status = 200)),
    security(("session_cookie" = []), ("bearer" = [])))]
pub async fn v1_obs_start_stream_proxy(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, crate::ApiError> {
    state.obs_handler.start_stream().await?;
    Ok(Json(serde_json::Value::Null))
}

#[utoipa::path(delete, path = "/obs/stream", tag = "obs",
    responses((status = 200)),
    security(("session_cookie" = []), ("bearer" = [])))]
pub async fn v1_obs_stop_stream_proxy(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, crate::ApiError> {
    state.obs_handler.stop_stream().await?;
    Ok(Json(serde_json::Value::Null))
}

