// Stream Commands
// Handles FFmpeg streaming operations

use crate::models::OutputGroup;

/// Start streaming for an output group
#[tauri::command]
pub async fn start_stream(group: OutputGroup, incoming_url: String) -> Result<u32, String> {
    // TODO: Implement with FFmpegHandler service
    Err("Not implemented".to_string())
}

/// Stop streaming for an output group
#[tauri::command]
pub async fn stop_stream(group_id: String) -> Result<(), String> {
    // TODO: Implement with FFmpegHandler service
    Err("Not implemented".to_string())
}

/// Stop all active streams
#[tauri::command]
pub async fn stop_all_streams() -> Result<(), String> {
    // TODO: Implement with FFmpegHandler service
    Err("Not implemented".to_string())
}
