// System Commands
// Handles system-level operations like encoder detection

use crate::models::Encoders;

/// Get available video and audio encoders
#[tauri::command]
pub async fn get_encoders() -> Result<Encoders, String> {
    // TODO: Implement encoder detection
    Ok(Encoders {
        video: vec![
            "libx264".to_string(),
            "h264_videotoolbox".to_string(),
        ],
        audio: vec![
            "aac".to_string(),
            "libmp3lame".to_string(),
        ],
    })
}

/// Test FFmpeg installation
#[tauri::command]
pub async fn test_ffmpeg() -> Result<String, String> {
    // TODO: Implement FFmpeg test
    Ok("FFmpeg not tested yet".to_string())
}
