// Permission checking commands (screen capture, camera, microphone)
// Stubs for HTTP mode - actual permission handling is in Tauri desktop layer.
// In HTTP/browser mode, we return granted since browser handles its own permissions.

use serde_json::{json, Value};

use crate::app_state::{AppState, get_opt_arg};

/// Handle permission-related commands.
pub(super) fn handle(_state: &AppState, command: &str, payload: &Value) -> Result<Value, String> {
    match command {
        "check_permissions" => {
            Ok(json!({
                "camera": "granted",
                "microphone": "granted",
                "screenRecording": "granted"
            }))
        }
        "get_platform" => {
            let platform = if cfg!(target_os = "macos") {
                "macos"
            } else if cfg!(target_os = "windows") {
                "windows"
            } else if cfg!(target_os = "linux") {
                "linux"
            } else {
                "unknown"
            };
            Ok(json!(platform))
        }
        "request_permission" => {
            // In HTTP mode, we can't trigger OS permission dialogs
            // Return true to indicate the request was "successful" (browser handles its own)
            Ok(json!(true))
        }
        "get_permission_guidance" => {
            let perm_type: String = get_opt_arg(&payload, "permType")?.unwrap_or_default();
            let guidance = match perm_type.as_str() {
                "camera" => "Please allow camera access in your browser when prompted.",
                "microphone" => "Please allow microphone access in your browser when prompted.",
                "screen_recording" => "Please allow screen sharing in your browser when prompted.",
                _ => "Please check your browser permissions settings.",
            };
            Ok(json!(guidance))
        }
        _ => Err(format!("Unknown permission command: {}", command)),
    }
}
