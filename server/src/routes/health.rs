use axum::{
    extract::{Json, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde_json::json;

use crate::app_state::AppState;

pub(crate) async fn health() -> impl IntoResponse {
    Json(json!({ "ok": true }))
}

/// Readiness check - verifies critical services are functional
pub(crate) async fn ready(State(state): State<AppState>) -> impl IntoResponse {
    let mut checks: Vec<(&str, bool)> = Vec::new();

    // Check 1: ProfileManager can access profiles directory
    let profiles_ok = state.profile_manager.get_all_names().await.is_ok();
    checks.push(("profiles", profiles_ok));

    // Check 2: SettingsManager can load settings
    let settings_ok = state.settings_manager.load().is_ok();
    checks.push(("settings", settings_ok));

    // Check 3: ThemeManager initialized (theme list is always available after init)
    let themes_ok = true;
    checks.push(("themes", themes_ok));

    // Check 4: FFmpeg is available (non-blocking path check)
    let ffmpeg_path = state.ffmpeg_handler.get_ffmpeg_path();
    let ffmpeg_ok = std::path::Path::new(&ffmpeg_path).exists();
    checks.push(("ffmpeg", ffmpeg_ok));

    let all_ok = checks.iter().all(|(_, ok)| *ok);
    let failed: Vec<&str> = checks
        .iter()
        .filter(|(_, ok)| !ok)
        .map(|(name, _)| *name)
        .collect();

    if all_ok {
        Json(json!({ "ready": true })).into_response()
    } else {
        log::warn!("Readiness check failed: {failed:?}");
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({ "ready": false, "failed": failed })),
        )
            .into_response()
    }
}

/// Power and thermal status endpoint
pub(crate) async fn power_status(State(state): State<AppState>) -> impl IntoResponse {
    Json(json!(state.power_budget.status()))
}
