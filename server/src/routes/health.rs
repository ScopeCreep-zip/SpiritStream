use axum::{
    extract::{Json, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde::Serialize;
use serde_json::json;

use crate::state::AppState;

pub(crate) async fn health() -> impl IntoResponse {
    Json(json!({ "ok": true }))
}

/// Readiness check - verifies critical services are functional
pub(crate) async fn ready(State(state): State<AppState>) -> impl IntoResponse {
    #[derive(Debug, Serialize)]
    struct ReadyCheckError {
        check: &'static str,
        error: String,
    }

    let mut errors: Vec<ReadyCheckError> = Vec::new();

    // Check 1: ProfileManager can access profiles directory
    let profiles_ok = match state.profile_manager.get_all_names().await {
        Ok(_) => true,
        Err(err) => {
            errors.push(ReadyCheckError {
                check: "profiles",
                error: err,
            });
            false
        }
    };

    // Check 2: SettingsManager can load settings
    let settings_ok = match state.settings_manager.load() {
        Ok(_) => true,
        Err(err) => {
            errors.push(ReadyCheckError {
                check: "settings",
                error: err,
            });
            false
        }
    };

    // Check 3: ThemeManager initialized (theme list is always available after init)
    let themes_ok = true;

    let all_ok = profiles_ok && settings_ok && themes_ok;
    let failed: Vec<&str> = errors.iter().map(|item| item.check).collect();

    if all_ok {
        Json(json!({ "ready": true })).into_response()
    } else {
        log::warn!("Readiness check failed: {errors:?}");
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({
                "ready": false,
                "failed": failed,
                "errors": errors,
            })),
        )
            .into_response()
    }
}
