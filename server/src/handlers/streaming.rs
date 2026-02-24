use serde_json::{json, Value};
use std::sync::Arc;

#[cfg(feature = "chat")]
use crate::chat_lifecycle::{auto_connect_chat_platforms, auto_disconnect_chat_platforms};
use crate::state::AppState;
use crate::util::get_arg;
use spiritstream_server::models::OutputGroup;
use spiritstream_server::services::EventSink;

pub(crate) async fn handle(state: &AppState, command: &str, payload: &Value) -> Result<Value, String> {
    match command {
        "start_stream" => {
            let group: OutputGroup = get_arg(payload, "group")?;
            let incoming_url: String = get_arg(payload, "incomingUrl")?;
            #[cfg(feature = "chat")]
            let was_streaming = state.ffmpeg_handler.active_count() > 0;
            let event_sink: Arc<dyn EventSink> = Arc::new(state.event_bus.clone());
            let pid = state.ffmpeg_handler.start(&group, &incoming_url, event_sink)?;
            // Reset reconnection state on successful manual start
            state.ffmpeg_handler.reset_reconnection_state(&group.id);
            // Auto-connect chat platforms on first stream start
            #[cfg(feature = "chat")]
            if !was_streaming {
                state.chat_manager.start_log_session();
                tokio::spawn(auto_connect_chat_platforms(state.clone()));
            }
            Ok(json!(pid))
        }
        "start_all_streams" => {
            let groups: Vec<OutputGroup> = get_arg(payload, "groups")?;
            let incoming_url: String = get_arg(payload, "incomingUrl")?;
            #[cfg(feature = "chat")]
            let was_streaming = state.ffmpeg_handler.active_count() > 0;
            let event_sink: Arc<dyn EventSink> = Arc::new(state.event_bus.clone());
            let pids = state.ffmpeg_handler.start_all(&groups, &incoming_url, event_sink)?;
            // Auto-connect chat platforms when streams start
            #[cfg(feature = "chat")]
            {
                if !was_streaming {
                    state.chat_manager.start_log_session();
                }
                tokio::spawn(auto_connect_chat_platforms(state.clone()));
            }
            Ok(json!(pids))
        }
        "stop_stream" => {
            let group_id: String = get_arg(payload, "groupId")?;
            state.ffmpeg_handler.stop(&group_id)?;
            // Auto-disconnect chat when no more streams are running
            #[cfg(feature = "chat")]
            if state.ffmpeg_handler.active_count() == 0 {
                state.chat_manager.end_log_session();
                let chat_mgr = state.chat_manager.clone();
                let bus = state.event_bus.clone();
                tokio::spawn(auto_disconnect_chat_platforms(chat_mgr, bus));
            }
            Ok(Value::Null)
        }
        "stop_all_streams" => {
            state.ffmpeg_handler.stop_all()?;
            // Auto-disconnect all chat platforms
            #[cfg(feature = "chat")]
            {
                state.chat_manager.end_log_session();
                let chat_mgr = state.chat_manager.clone();
                let bus = state.event_bus.clone();
                tokio::spawn(auto_disconnect_chat_platforms(chat_mgr, bus));
            }
            Ok(Value::Null)
        }
        "retry_stream" => {
            let group_id: String = get_arg(payload, "groupId")?;
            let event_sink: Arc<dyn EventSink> = Arc::new(state.event_bus.clone());
            let ffmpeg_handler = state.ffmpeg_handler.clone();
            // Use spawn_blocking to avoid blocking the async runtime during backoff sleep
            let (pid, next_delay) = tokio::task::spawn_blocking(move || {
                ffmpeg_handler.retry_group(&group_id, event_sink)
            })
            .await
            .map_err(|e| format!("Task join error: {e}"))??;
            Ok(json!({
                "pid": pid,
                "nextDelaySecs": next_delay.map(|d| d.as_secs())
            }))
        }
        "get_active_stream_count" => Ok(json!(state.ffmpeg_handler.active_count())),
        "is_group_streaming" => {
            let group_id: String = get_arg(payload, "groupId")?;
            Ok(json!(state.ffmpeg_handler.is_streaming(&group_id)))
        }
        "get_active_group_ids" => Ok(json!(state.ffmpeg_handler.get_active_group_ids())),
        "toggle_stream_target" => {
            let target_id: String = get_arg(payload, "targetId")?;
            let enabled: bool = get_arg(payload, "enabled")?;
            let group: OutputGroup = get_arg(payload, "group")?;
            let incoming_url: String = get_arg(payload, "incomingUrl")?;
            if enabled {
                state.ffmpeg_handler.enable_target(&target_id);
            } else {
                state.ffmpeg_handler.disable_target(&target_id);
            }
            let event_sink: Arc<dyn EventSink> = Arc::new(state.event_bus.clone());
            let pid = state
                .ffmpeg_handler
                .restart_group(&group.id, &group, &incoming_url, event_sink)?;
            Ok(json!(pid))
        }
        "is_target_disabled" => {
            let target_id: String = get_arg(payload, "targetId")?;
            Ok(json!(state.ffmpeg_handler.is_target_disabled(&target_id)))
        }
        _ => Err(format!("Unknown streaming command: {command}")),
    }
}
