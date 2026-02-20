// Streaming Commands
// Handles stream lifecycle, target management, and FFmpeg system operations

use std::sync::Arc;

use serde_json::{json, Value};

use crate::app_state::{AppState, get_arg};
use crate::commands::{get_encoders, test_ffmpeg, validate_ffmpeg_path, test_rtmp_target};
use crate::models::OutputGroup;
use crate::services::EventSink;

/// Handle streaming-related commands.
///
/// Returns `None` if the command is not recognized by this module,
/// or `Some(result)` if the command was handled.
pub async fn handle(state: &AppState, command: &str, payload: &Value) -> Option<Result<Value, String>> {
    let result = match command {
        "start_stream" => {
            let group: OutputGroup = match get_arg(payload, "group") {
                Ok(v) => v,
                Err(e) => return Some(Err(e)),
            };
            let incoming_url: String = match get_arg(payload, "incomingUrl") {
                Ok(v) => v,
                Err(e) => return Some(Err(e)),
            };
            let event_sink: Arc<dyn EventSink> = Arc::new(state.event_bus.clone());
            let pid = state.ffmpeg_handler.start(&group, &incoming_url, event_sink);
            pid.map(|p| json!(p))
        }
        "start_all_streams" => {
            let groups: Vec<OutputGroup> = match get_arg(payload, "groups") {
                Ok(v) => v,
                Err(e) => return Some(Err(e)),
            };
            let incoming_url: String = match get_arg(payload, "incomingUrl") {
                Ok(v) => v,
                Err(e) => return Some(Err(e)),
            };
            let event_sink: Arc<dyn EventSink> = Arc::new(state.event_bus.clone());
            let pids = state.ffmpeg_handler.start_all(&groups, &incoming_url, event_sink);
            pids.map(|p| json!(p))
        }
        "stop_stream" => {
            let group_id: String = match get_arg(payload, "groupId") {
                Ok(v) => v,
                Err(e) => return Some(Err(e)),
            };
            state.ffmpeg_handler.stop(&group_id).map(|_| Value::Null)
        }
        "stop_all_streams" => {
            state.ffmpeg_handler.stop_all().map(|_| Value::Null)
        }
        "get_active_stream_count" => Ok(json!(state.ffmpeg_handler.active_count())),
        "is_group_streaming" => {
            let group_id: String = match get_arg(payload, "groupId") {
                Ok(v) => v,
                Err(e) => return Some(Err(e)),
            };
            Ok(json!(state.ffmpeg_handler.is_streaming(&group_id)))
        }
        "get_active_group_ids" => Ok(json!(state.ffmpeg_handler.get_active_group_ids())),
        "toggle_stream_target" => {
            let target_id: String = match get_arg(payload, "targetId") {
                Ok(v) => v,
                Err(e) => return Some(Err(e)),
            };
            let enabled: bool = match get_arg(payload, "enabled") {
                Ok(v) => v,
                Err(e) => return Some(Err(e)),
            };
            let group: OutputGroup = match get_arg(payload, "group") {
                Ok(v) => v,
                Err(e) => return Some(Err(e)),
            };
            let incoming_url: String = match get_arg(payload, "incomingUrl") {
                Ok(v) => v,
                Err(e) => return Some(Err(e)),
            };
            if enabled {
                state.ffmpeg_handler.enable_target(&target_id);
            } else {
                state.ffmpeg_handler.disable_target(&target_id);
            }
            let event_sink: Arc<dyn EventSink> = Arc::new(state.event_bus.clone());
            let pid = state
                .ffmpeg_handler
                .restart_group(&group.id, &group, &incoming_url, event_sink);
            pid.map(|p| json!(p))
        }
        "is_target_disabled" => {
            let target_id: String = match get_arg(payload, "targetId") {
                Ok(v) => v,
                Err(e) => return Some(Err(e)),
            };
            Ok(json!(state.ffmpeg_handler.is_target_disabled(&target_id)))
        }
        "get_encoders" => get_encoders().map(|e| json!(e)),
        "test_ffmpeg" => test_ffmpeg().map(|v| json!(v)),
        "validate_ffmpeg_path" => {
            let path: String = match get_arg(payload, "path") {
                Ok(v) => v,
                Err(e) => return Some(Err(e)),
            };
            validate_ffmpeg_path(path).map(|v| json!(v))
        }
        "test_rtmp_target" => {
            let url: String = match get_arg(payload, "url") {
                Ok(v) => v,
                Err(e) => return Some(Err(e)),
            };
            let stream_key: String = match get_arg(payload, "streamKey") {
                Ok(v) => v,
                Err(e) => return Some(Err(e)),
            };
            test_rtmp_target(url, stream_key).map(|v| json!(v))
        }
        _ => return None,
    };

    Some(result)
}
