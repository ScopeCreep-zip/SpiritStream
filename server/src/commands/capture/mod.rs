// Capture & Preview Command Handlers
// Handles source preview lifecycle, permissions, and audio monitoring/capture

mod audio;
mod levels;
mod permissions;
mod preview;

use serde_json::Value;

use crate::app_state::AppState;

/// Handle capture/preview commands.
/// Returns `None` for unrecognized commands, `Some(result)` for handled ones.
pub async fn handle(state: &AppState, command: &str, payload: &Value) -> Option<Result<Value, String>> {
    let result = match command {
        // ====================================================================
        // Preview Commands
        // ====================================================================
        "stop_source_preview" | "stop_all_previews" | "set_idle_mode" => {
            return Some(preview::handle(state, command, payload));
        }

        // ====================================================================
        // Permission Commands
        // ====================================================================
        "check_permissions" | "get_platform" | "request_permission" | "get_permission_guidance" => {
            return Some(permissions::handle(state, command, payload));
        }

        // ====================================================================
        // Audio Level Monitoring Commands
        // ====================================================================
        "get_audio_monitor_status" | "get_audio_monitor_health" => {
            return Some(levels::handle(state, command, payload).await);
        }

        // ====================================================================
        // Audio Capture Commands
        // ====================================================================
        "set_audio_monitor_sources" => {
            match audio::handle_set_audio_monitor_sources(state, payload).await {
                Ok(v) => Ok(v),
                Err(e) => Err(e),
            }
        }

        _ => return None,
    };

    Some(result)
}
