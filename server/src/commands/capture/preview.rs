// Preview lifecycle commands (start/stop preview, idle mode)

use serde_json::Value;

use crate::app_state::{AppState, get_arg};

/// Handle preview-related commands.
pub(super) fn handle(state: &AppState, command: &str, payload: &Value) -> Result<Value, String> {
    match command {
        "stop_source_preview" => {
            let source_id: String = get_arg(&payload, "sourceId")?;
            state.preview_handler.stop_source_preview(&source_id);
            Ok(Value::Null)
        }
        "stop_all_previews" => {
            state.preview_handler.stop_all_previews();
            Ok(Value::Null)
        }
        "set_idle_mode" => {
            let idle: bool = get_arg(&payload, "idle")?;
            state.native_preview.set_idle(idle);
            log::info!("App idle mode set to: {}", idle);
            Ok(Value::Null)
        }
        _ => Err(format!("Unknown preview command: {}", command)),
    }
}
