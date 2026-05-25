//! Theme handlers — `/api/v1/themes/*`.
//!
//! List, install, refresh, and per-theme token introspection.
//! Thin shims over `ThemeManager`.

use axum::{
    extract::{Path as AxumPath, State},
    Json,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::AppState;

// --------------------------------------------------------------------------
// Themes.

#[utoipa::path(get, path = "/themes", tag = "themes",
    responses((status = 200, body = serde_json::Value)),
    security(("session_cookie" = []), ("bearer" = [])))]
pub async fn v1_themes_list_proxy(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, crate::ApiError> {
    let themes = state.theme_manager.list_themes();
    Ok(Json(serde_json::json!(themes)))
}

#[utoipa::path(post, path = "/themes/refresh", tag = "themes",
    responses((status = 200, body = serde_json::Value)),
    security(("session_cookie" = []), ("bearer" = [])))]
pub async fn v1_themes_refresh_proxy(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, crate::ApiError> {
    state.theme_manager.sync_project_themes();
    let themes = state.theme_manager.list_themes();
    Ok(Json(serde_json::json!(themes)))
}

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ThemeInstallRequest {
    pub theme_path: String,
}

#[utoipa::path(post, path = "/themes", tag = "themes",
    request_body = ThemeInstallRequest,
    responses(
        (status = 200, body = serde_json::Value, description = "Theme installed."),
        (status = 400, body = ApiErrorBody, description = "Theme file invalid (bad JSON, missing required tokens, etc.)."),
        (status = 403, body = ApiErrorBody, description = "Theme path outside allowed root."),
        (status = 500, body = ApiErrorBody, description = "Internal error reading or copying the file."),
    ),
    security(("session_cookie" = []), ("bearer" = [])))]
pub async fn v1_themes_install_proxy(
    State(state): State<AppState>,
    axum::Json(req): axum::Json<ThemeInstallRequest>,
) -> Result<Json<serde_json::Value>, crate::ApiError> {
    let path = std::path::PathBuf::from(&req.theme_path);
    spiritstream_core::services::validate_extension(&path, &["json", "jsonc"])?;
    if req.theme_path.contains("..") {
        return Err(spiritstream_core::CoreError::PathOutsideAllowedRoot {
            path: req.theme_path,
        }
        .into());
    }
    let summary = state.theme_manager.install_theme(&path)?;
    Ok(Json(serde_json::json!(summary)))
}

#[utoipa::path(get, path = "/themes/{theme_id}/tokens", tag = "themes",
    params(("theme_id" = String, Path, description = "Theme ID")),
    responses(
        (status = 200, body = serde_json::Value, description = "Theme token map."),
        (status = 400, body = ApiErrorBody, description = "Theme not found or invalid."),
    ),
    security(("session_cookie" = []), ("bearer" = [])))]
pub async fn v1_theme_tokens_proxy(
    State(state): State<AppState>,
    AxumPath(theme_id): AxumPath<String>,
) -> Result<Json<serde_json::Value>, crate::ApiError> {
    let tokens = state.theme_manager.get_theme_tokens(&theme_id)?;
    Ok(Json(serde_json::json!(tokens)))
}
