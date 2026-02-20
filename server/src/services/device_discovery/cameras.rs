// Camera device enumeration
// Platform-specific camera discovery (macOS: AVFoundation, Windows: DirectShow, Linux: V4L2)

use crate::models::{AudioInputDevice, CameraDevice, Resolution};
use std::process::Command;

/// Auto-pair cameras with their linked audio devices by name matching
/// Uses keyword matching to find related audio devices (e.g., FaceTime Camera -> Built-in Microphone)
pub(super) fn pair_cameras_with_audio(
    cameras: &mut Vec<CameraDevice>,
    audio_devices: &[AudioInputDevice],
) {
    for camera in cameras.iter_mut() {
        let camera_name_lower = camera.name.to_lowercase();

        // Extract keywords from camera name (skip common generic terms)
        let keywords: Vec<&str> = camera_name_lower
            .split(|c: char| c.is_whitespace() || c == '-' || c == '(' || c == ')')
            .filter(|s| s.len() > 3 && *s != "camera" && *s != "webcam" && *s != "video")
            .collect();

        for audio in audio_devices {
            let audio_name_lower = audio.name.to_lowercase();

            // Strategy 1: Match by shared keywords (e.g., "FaceTime", "Logitech", "C920")
            let keyword_match = keywords.iter().any(|kw| audio_name_lower.contains(kw));

            // Strategy 2: Special case for macOS - FaceTime camera -> Built-in Microphone
            let builtin_match = camera_name_lower.contains("facetime")
                && (audio_name_lower.contains("built-in") || audio_name_lower.contains("macbook"));

            // Strategy 3: USB cameras often have matching names (e.g., "Logitech C920" camera and audio)
            let exact_prefix_match = !camera_name_lower.contains("facetime")
                && audio_name_lower.starts_with(&camera_name_lower[..camera_name_lower.len().min(10)]);

            if keyword_match || builtin_match || exact_prefix_match {
                camera.linked_audio_device_id = Some(audio.device_id.clone());
                camera.linked_audio_device_name = Some(audio.name.clone());
                break;
            }
        }
    }
}

pub(super) fn list_cameras_sync(ffmpeg_path: &str) -> Result<Vec<CameraDevice>, String> {
    #[cfg(target_os = "macos")]
    {
        list_cameras_macos(ffmpeg_path)
    }
    #[cfg(target_os = "windows")]
    {
        list_cameras_windows(ffmpeg_path)
    }
    #[cfg(target_os = "linux")]
    {
        list_cameras_linux(ffmpeg_path)
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        let _ = ffmpeg_path;
        Err("Camera discovery not supported on this platform".to_string())
    }
}

// ============================================================
// macOS implementations using AVFoundation via FFmpeg
// ============================================================

#[cfg(target_os = "macos")]
fn list_cameras_macos(ffmpeg_path: &str) -> Result<Vec<CameraDevice>, String> {
    // Use FFmpeg to list AVFoundation devices
    let output = Command::new(ffmpeg_path)
        .args(["-f", "avfoundation", "-list_devices", "true", "-i", ""])
        .output()
        .map_err(|e| format!("Failed to run FFmpeg: {}", e))?;

    // FFmpeg writes device list to stderr
    let stderr = String::from_utf8_lossy(&output.stderr);
    let mut cameras = parse_avfoundation_cameras(&stderr)?;

    // Get audio devices for auto-pairing
    let audio_devices = super::audio::parse_avfoundation_audio(&stderr)?;

    // Auto-pair cameras with their microphones
    pair_cameras_with_audio(&mut cameras, &audio_devices);

    Ok(cameras)
}

#[cfg(target_os = "macos")]
pub(super) fn parse_avfoundation_cameras(output: &str) -> Result<Vec<CameraDevice>, String> {
    let mut cameras = Vec::new();
    let mut in_video_section = false;

    for line in output.lines() {
        if line.contains("AVFoundation video devices:") {
            in_video_section = true;
            continue;
        }
        if line.contains("AVFoundation audio devices:") {
            break;
        }
        if in_video_section {
            // Parse lines like "[AVFoundation indev @ 0x...] [0] FaceTime HD Camera"
            if let Some(bracket_pos) = line.find("] [") {
                let rest = &line[bracket_pos + 3..];
                if let Some(end_bracket) = rest.find(']') {
                    if let Ok(idx) = rest[..end_bracket].parse::<usize>() {
                        let name = rest[end_bracket + 2..].trim().to_string();
                        // Skip screen capture devices (they show as video but are displays)
                        if !name.contains("Capture screen") && !name.is_empty() {
                            cameras.push(CameraDevice {
                                device_id: idx.to_string(),
                                name,
                                resolutions: vec![
                                    Resolution { width: 1920, height: 1080, fps: vec![30, 60] },
                                    Resolution { width: 1280, height: 720, fps: vec![30, 60] },
                                    Resolution { width: 640, height: 480, fps: vec![30] },
                                ],
                                linked_audio_device_id: None,
                                linked_audio_device_name: None,
                            });
                        }
                    }
                }
            }
        }
    }

    Ok(cameras)
}

// ============================================================
// Windows implementations using DirectShow
// ============================================================

#[cfg(target_os = "windows")]
fn list_cameras_windows(ffmpeg_path: &str) -> Result<Vec<CameraDevice>, String> {
    let output = Command::new(ffmpeg_path)
        .args(["-f", "dshow", "-list_devices", "true", "-i", "dummy"])
        .output()
        .map_err(|e| format!("Failed to run FFmpeg: {}", e))?;

    let stderr = String::from_utf8_lossy(&output.stderr);
    let mut cameras: Vec<CameraDevice> = super::windows::parse_dshow_devices(&stderr, "video")?
        .into_iter()
        .map(|(id, name)| CameraDevice {
            device_id: id,
            name,
            resolutions: vec![
                Resolution { width: 1920, height: 1080, fps: vec![30, 60] },
                Resolution { width: 1280, height: 720, fps: vec![30, 60] },
            ],
            linked_audio_device_id: None,
            linked_audio_device_name: None,
        })
        .collect();

    // Get audio devices for auto-pairing
    let audio_devices = super::audio::list_audio_inputs_sync(ffmpeg_path)?;
    pair_cameras_with_audio(&mut cameras, &audio_devices);

    Ok(cameras)
}

// ============================================================
// Linux implementations using V4L2
// ============================================================

#[cfg(target_os = "linux")]
fn list_cameras_linux(ffmpeg_path: &str) -> Result<Vec<CameraDevice>, String> {
    let output = Command::new("v4l2-ctl")
        .args(["--list-devices"])
        .output()
        .map_err(|_| "v4l2-ctl not found. Install v4l-utils package.".to_string())?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut cameras = parse_v4l2_cameras(&stdout)?;

    // Get audio devices for auto-pairing
    let audio_devices = super::audio::list_audio_inputs_sync(ffmpeg_path)?;
    pair_cameras_with_audio(&mut cameras, &audio_devices);

    Ok(cameras)
}

#[cfg(target_os = "linux")]
fn parse_v4l2_cameras(output: &str) -> Result<Vec<CameraDevice>, String> {
    let mut cameras = Vec::new();
    let mut current_name: Option<String> = None;

    for line in output.lines() {
        let trimmed = line.trim();
        if trimmed.ends_with(':') {
            // Device name line
            current_name = Some(trimmed.trim_end_matches(':').to_string());
        } else if trimmed.starts_with("/dev/video") && !trimmed.contains("1") {
            // Only use the first video device for each camera
            if let Some(ref name) = current_name {
                cameras.push(CameraDevice {
                    device_id: trimmed.to_string(),
                    name: name.clone(),
                    resolutions: vec![
                        Resolution { width: 1920, height: 1080, fps: vec![30] },
                        Resolution { width: 1280, height: 720, fps: vec![30] },
                    ],
                    linked_audio_device_id: None,
                    linked_audio_device_name: None,
                });
            }
            current_name = None;
        }
    }

    Ok(cameras)
}
