// Stream Commands
// Handles FFmpeg streaming operations

use tauri::{AppHandle, State};
use crate::models::OutputGroup;
use crate::services::FFmpegHandler;

/// Start streaming for an output group with real-time stats
#[tauri::command]
pub async fn start_stream(
    app: AppHandle,
    group: OutputGroup,
    incoming_url: String,
    ffmpeg_handler: State<'_, FFmpegHandler>
) -> Result<u32, String> {
    ffmpeg_handler.start(&group, &incoming_url, &app)
}

/// Stop streaming for an output group
#[tauri::command]
pub async fn stop_stream(
    group_id: String,
    ffmpeg_handler: State<'_, FFmpegHandler>
) -> Result<(), String> {
    ffmpeg_handler.stop(&group_id)
}

/// Stop all active streams
#[tauri::command]
pub async fn stop_all_streams(
    ffmpeg_handler: State<'_, FFmpegHandler>
) -> Result<(), String> {
    ffmpeg_handler.stop_all()
}

/// Get the number of active streams
#[tauri::command]
pub fn get_active_stream_count(
    ffmpeg_handler: State<'_, FFmpegHandler>
) -> usize {
    ffmpeg_handler.active_count()
}

/// Check if a specific output group is currently streaming
#[tauri::command]
pub fn is_group_streaming(
    group_id: String,
    ffmpeg_handler: State<'_, FFmpegHandler>
) -> bool {
    ffmpeg_handler.is_streaming(&group_id)
}

/// Get list of active stream group IDs
#[tauri::command]
pub fn get_active_group_ids(
    ffmpeg_handler: State<'_, FFmpegHandler>
) -> Vec<String> {
    ffmpeg_handler.get_active_group_ids()
}
