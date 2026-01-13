// Stream Commands
// Handles FFmpeg streaming operations

use tauri::{AppHandle, State};
use crate::models::OutputGroup;
use crate::services::FFmpegHandler;

/// Start streaming for an output group with real-time stats
#[tauri::command]
pub fn start_stream(
    app: AppHandle,
    group: OutputGroup,
    incoming_url: String,
    ffmpeg_handler: State<'_, FFmpegHandler>
) -> Result<u32, String> {
    // Validate inputs
    if incoming_url.is_empty() {
        return Err("Incoming URL is required".to_string());
    }
    if group.stream_targets.is_empty() {
        return Err("At least one stream target is required".to_string());
    }
    if group.video.codec.is_empty() {
        return Err("Video encoder is required".to_string());
    }
    if group.audio.codec.is_empty() {
        return Err("Audio codec is required".to_string());
    }

    ffmpeg_handler.start(&group, &incoming_url, &app)
}

/// Start streaming for multiple output groups in one batch
#[tauri::command]
pub fn start_all_streams(
    app: AppHandle,
    groups: Vec<OutputGroup>,
    incoming_url: String,
    ffmpeg_handler: State<'_, FFmpegHandler>,
) -> Result<Vec<u32>, String> {
    if incoming_url.is_empty() {
        return Err("Incoming URL is required".to_string());
    }

    if groups.is_empty() {
        return Err("At least one output group is required".to_string());
    }

    for group in &groups {
        if group.video.codec.is_empty() {
            return Err("Video encoder is required".to_string());
        }
        if group.audio.codec.is_empty() {
            return Err("Audio codec is required".to_string());
        }
    }

    ffmpeg_handler.start_all(&groups, &incoming_url, &app)
}

/// Stop streaming for an output group
#[tauri::command]
pub fn stop_stream(
    group_id: String,
    ffmpeg_handler: State<'_, FFmpegHandler>
) -> Result<(), String> {
    ffmpeg_handler.stop(&group_id)
}

/// Stop all active streams
#[tauri::command]
pub fn stop_all_streams(
    ffmpeg_handler: State<'_, FFmpegHandler>
) -> Result<(), String> {
    ffmpeg_handler.stop_all()
}

/// Get the number of active streams
#[tauri::command]
pub fn get_active_stream_count(
    ffmpeg_handler: State<'_, FFmpegHandler>
) -> Result<usize, String> {
    Ok(ffmpeg_handler.active_count())
}

/// Check if a specific output group is currently streaming
#[tauri::command]
pub fn is_group_streaming(
    group_id: String,
    ffmpeg_handler: State<'_, FFmpegHandler>
) -> Result<bool, String> {
    Ok(ffmpeg_handler.is_streaming(&group_id))
}

/// Get list of active stream group IDs
#[tauri::command]
pub fn get_active_group_ids(
    ffmpeg_handler: State<'_, FFmpegHandler>
) -> Result<Vec<String>, String> {
    Ok(ffmpeg_handler.get_active_group_ids())
}

/// Toggle a specific stream target on/off
/// This will restart the parent output group with the updated target list
#[tauri::command]
pub fn toggle_stream_target(
    app: AppHandle,
    target_id: String,
    enabled: bool,
    group: OutputGroup,
    incoming_url: String,
    ffmpeg_handler: State<'_, FFmpegHandler>
) -> Result<u32, String> {
    // Update the disabled targets set
    if enabled {
        ffmpeg_handler.enable_target(&target_id);
    } else {
        ffmpeg_handler.disable_target(&target_id);
    }

    // Restart the group with the updated target list
    ffmpeg_handler.restart_group(&group.id, &group, &incoming_url, &app)
}

/// Check if a specific stream target is currently disabled
#[tauri::command]
pub fn is_target_disabled(
    target_id: String,
    ffmpeg_handler: State<'_, FFmpegHandler>
) -> Result<bool, String> {
    Ok(ffmpeg_handler.is_target_disabled(&target_id))
}
