use axum::{
    extract::Json,
    response::IntoResponse,
};
use serde::Deserialize;
use serde_json::json;

use crate::services::PermissionsService;

/// GET /api/permissions/status - Get permission status
/// Uses async-safe permission checks that run scap calls in spawn_blocking on macOS
pub(crate) async fn permissions_status_handler() -> impl IntoResponse {
    let status = PermissionsService::get_status_async().await;
    Json(json!({ "ok": true, "data": status }))
}

#[derive(Debug, Deserialize)]
pub(crate) struct RequestPermissionsRequest {
    types: Vec<String>,
}

/// POST /api/permissions/request - Request permissions
/// On macOS, screen recording permission opens System Preferences
/// On Windows/Linux, permissions are handled via picker dialogs when capture starts
pub(crate) async fn request_permissions_handler(
    Json(req): Json<RequestPermissionsRequest>,
) -> impl IntoResponse {
    let mut results = std::collections::HashMap::new();

    for perm_type in &req.types {
        let granted = match perm_type.as_str() {
            "camera" => PermissionsService::request_camera_permission(),
            "microphone" => PermissionsService::request_microphone_permission(),
            "screen" | "screenRecording" => {
                PermissionsService::request_screen_recording_permission_async().await
            }
            _ => false,
        };
        results.insert(perm_type.clone(), granted);
    }

    let status = PermissionsService::get_status_async().await;
    Json(json!({ "ok": true, "data": { "results": results, "status": status } }))
}
