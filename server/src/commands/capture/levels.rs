// Audio level monitoring commands (monitor status, health)

use serde_json::{json, Value};

use crate::app_state::AppState;

/// Handle audio level monitoring commands.
pub(super) async fn handle(state: &AppState, command: &str, _payload: &Value) -> Result<Value, String> {
    match command {
        "get_audio_monitor_status" => {
            Ok(json!({
                "running": state.audio_level_service.is_running()
            }))
        }
        "get_audio_monitor_health" => {
            let health = state.audio_level_service.get_health_status();
            Ok(json!({
                "sources": health
            }))
        }
        _ => Err(format!("Unknown levels command: {}", command)),
    }
}
