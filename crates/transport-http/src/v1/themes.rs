//! Theme handlers — `/api/v1/themes/*`.
//!
//! List, install, refresh, and per-theme token introspection.
//! Thin shims over `ThemeManager`.

use std::collections::HashMap;

use axum::{
    extract::{Path as AxumPath, State},
    Json,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use spiritstream_core::models::{ThemeMode, ThemeSummary};

use crate::AppState;

// --------------------------------------------------------------------------
// Wire-mirror types. utoipa is transport-only, so mirror every core
// payload we hand to the theme router instead of leaking `ToSchema`
// into the core crate.

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum ThemeModeWire {
    Light,
    Dark,
}

impl From<ThemeMode> for ThemeModeWire {
    fn from(m: ThemeMode) -> Self {
        match m {
            ThemeMode::Light => Self::Light,
            ThemeMode::Dark => Self::Dark,
        }
    }
}

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ThemeSummaryWire {
    pub id: String,
    pub name: String,
    pub mode: ThemeModeWire,
    pub source: String,
    pub built_in: bool,
    pub valid: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl From<ThemeSummary> for ThemeSummaryWire {
    fn from(s: ThemeSummary) -> Self {
        Self {
            id: s.id,
            name: s.name,
            mode: s.mode.into(),
            source: s.source,
            built_in: s.built_in,
            valid: s.valid,
            error: s.error,
        }
    }
}

/// `{tokens: {name → value}}` envelope used by `/themes/{id}/tokens`.
/// Wraps the raw token map so OpenAPI gets a named schema instead of
/// an inline `additionalProperties` object.
#[derive(Serialize, Deserialize, ToSchema)]
pub struct ThemeTokensResponse {
    pub tokens: HashMap<String, String>,
}

// --------------------------------------------------------------------------
// Themes.

#[utoipa::path(get, path = "/themes", tag = "themes",
    responses((status = 200, body = [ThemeSummaryWire])),
    security(("session_cookie" = []), ("bearer" = [])))]
pub async fn v1_themes_list_proxy(
    State(state): State<AppState>,
) -> Result<Json<Vec<ThemeSummaryWire>>, crate::ApiError> {
    let themes = state.theme_manager.list_themes();
    Ok(Json(themes.into_iter().map(Into::into).collect()))
}

#[utoipa::path(post, path = "/themes/refresh", tag = "themes",
    responses((status = 200, body = [ThemeSummaryWire])),
    security(("session_cookie" = []), ("bearer" = [])))]
pub async fn v1_themes_refresh_proxy(
    State(state): State<AppState>,
) -> Result<Json<Vec<ThemeSummaryWire>>, crate::ApiError> {
    state.theme_manager.sync_project_themes();
    let themes = state.theme_manager.list_themes();
    Ok(Json(themes.into_iter().map(Into::into).collect()))
}

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ThemeInstallRequest {
    pub theme_path: String,
}

#[utoipa::path(post, path = "/themes", tag = "themes",
    request_body = ThemeInstallRequest,
    responses(
        (status = 200, body = ThemeSummaryWire, description = "Theme installed."),
        (status = 400, body = ApiErrorBody, description = "Theme file invalid (bad JSON, missing required tokens, etc.)."),
        (status = 403, body = ApiErrorBody, description = "Theme path outside allowed root."),
        (status = 500, body = ApiErrorBody, description = "Internal error reading or copying the file."),
    ),
    security(("session_cookie" = []), ("bearer" = [])))]
pub async fn v1_themes_install_proxy(
    State(state): State<AppState>,
    axum::Json(req): axum::Json<ThemeInstallRequest>,
) -> Result<Json<ThemeSummaryWire>, crate::ApiError> {
    let path = std::path::PathBuf::from(&req.theme_path);
    spiritstream_core::services::validate_extension(&path, &["json", "jsonc"])?;
    if req.theme_path.contains("..") {
        return Err(spiritstream_core::CoreError::PathOutsideAllowedRoot {
            path: req.theme_path,
        }
        .into());
    }
    let summary = state.theme_manager.install_theme(&path)?;
    Ok(Json(summary.into()))
}

#[utoipa::path(get, path = "/themes/{theme_id}/tokens", tag = "themes",
    params(("theme_id" = String, Path, description = "Theme ID")),
    responses(
        (status = 200, body = ThemeTokensResponse, description = "Theme token map."),
        (status = 400, body = ApiErrorBody, description = "Theme not found or invalid."),
    ),
    security(("session_cookie" = []), ("bearer" = [])))]
pub async fn v1_theme_tokens_proxy(
    State(state): State<AppState>,
    AxumPath(theme_id): AxumPath<String>,
) -> Result<Json<ThemeTokensResponse>, crate::ApiError> {
    let tokens = state.theme_manager.get_theme_tokens(&theme_id)?;
    Ok(Json(ThemeTokensResponse { tokens }))
}
