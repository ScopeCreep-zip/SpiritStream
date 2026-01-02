// System Commands
// Handles system-level operations like encoder detection

use std::process::Command;
use crate::models::Encoders;

/// Find FFmpeg path
fn find_ffmpeg() -> String {
    // Try to find ffmpeg in PATH first
    if let Ok(output) = Command::new("which").arg("ffmpeg").output() {
        if output.status.success() {
            if let Ok(path) = String::from_utf8(output.stdout) {
                return path.trim().to_string();
            }
        }
    }

    // Fallback to common locations on macOS
    #[cfg(target_os = "macos")]
    {
        if std::path::Path::new("/opt/homebrew/bin/ffmpeg").exists() {
            return "/opt/homebrew/bin/ffmpeg".to_string();
        }
        if std::path::Path::new("/usr/local/bin/ffmpeg").exists() {
            return "/usr/local/bin/ffmpeg".to_string();
        }
    }

    "ffmpeg".to_string()
}

/// Get available video and audio encoders by querying FFmpeg
#[tauri::command]
pub async fn get_encoders() -> Result<Encoders, String> {
    let ffmpeg_path = find_ffmpeg();

    // Query FFmpeg for available encoders
    let output = Command::new(&ffmpeg_path)
        .args(["-encoders", "-hide_banner"])
        .output()
        .map_err(|e| format!("Failed to run FFmpeg: {}", e))?;

    if !output.status.success() {
        // Return default encoders if FFmpeg query fails
        return Ok(Encoders {
            video: vec!["libx264".to_string()],
            audio: vec!["aac".to_string()],
        });
    }

    let encoder_list = String::from_utf8_lossy(&output.stdout);

    // Common video encoders to look for
    let video_encoder_names = [
        ("libx264", "x264 (Software)"),
        ("h264_nvenc", "NVENC (NVIDIA)"),
        ("h264_videotoolbox", "VideoToolbox (Apple)"),
        ("h264_qsv", "QuickSync (Intel)"),
        ("h264_amf", "AMF (AMD)"),
    ];

    // Common audio encoders to look for
    let audio_encoder_names = [
        ("aac", "AAC"),
        ("libmp3lame", "MP3"),
        ("libopus", "Opus"),
    ];

    let mut video = Vec::new();
    let mut audio = Vec::new();

    for (name, _label) in video_encoder_names {
        if encoder_list.contains(name) {
            video.push(name.to_string());
        }
    }

    for (name, _label) in audio_encoder_names {
        if encoder_list.contains(name) {
            audio.push(name.to_string());
        }
    }

    // Ensure at least one encoder is available
    if video.is_empty() {
        video.push("libx264".to_string());
    }
    if audio.is_empty() {
        audio.push("aac".to_string());
    }

    Ok(Encoders { video, audio })
}

/// Test FFmpeg installation and return version string
#[tauri::command]
pub async fn test_ffmpeg() -> Result<String, String> {
    let ffmpeg_path = find_ffmpeg();

    let output = Command::new(&ffmpeg_path)
        .args(["-version"])
        .output()
        .map_err(|e| format!("Failed to run FFmpeg: {}", e))?;

    if !output.status.success() {
        return Err("FFmpeg returned an error".to_string());
    }

    let version_output = String::from_utf8_lossy(&output.stdout);

    // Extract the first line which contains the version
    let version_line = version_output
        .lines()
        .next()
        .unwrap_or("Unknown version")
        .to_string();

    Ok(version_line)
}
