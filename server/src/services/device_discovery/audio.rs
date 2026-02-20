// Audio device enumeration
// Platform-specific audio input discovery

use crate::models::AudioInputDevice;
use std::process::Command;

pub(super) fn list_audio_inputs_sync(ffmpeg_path: &str) -> Result<Vec<AudioInputDevice>, String> {
    #[cfg(target_os = "macos")]
    {
        list_audio_inputs_macos(ffmpeg_path)
    }
    #[cfg(target_os = "windows")]
    {
        list_audio_inputs_windows(ffmpeg_path)
    }
    #[cfg(target_os = "linux")]
    {
        list_audio_inputs_linux(ffmpeg_path)
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        let _ = ffmpeg_path;
        Err("Audio device discovery not supported on this platform".to_string())
    }
}

// ============================================================
// macOS implementations
// ============================================================

#[cfg(target_os = "macos")]
fn list_audio_inputs_macos(ffmpeg_path: &str) -> Result<Vec<AudioInputDevice>, String> {
    let output = Command::new(ffmpeg_path)
        .args(["-f", "avfoundation", "-list_devices", "true", "-i", ""])
        .output()
        .map_err(|e| format!("Failed to run FFmpeg: {}", e))?;

    let stderr = String::from_utf8_lossy(&output.stderr);
    parse_avfoundation_audio(&stderr)
}

#[cfg(target_os = "macos")]
pub(super) fn parse_avfoundation_audio(output: &str) -> Result<Vec<AudioInputDevice>, String> {
    let mut devices = Vec::new();
    let mut in_audio_section = false;

    for line in output.lines() {
        if line.contains("AVFoundation audio devices:") {
            in_audio_section = true;
            continue;
        }
        if in_audio_section {
            if let Some(bracket_pos) = line.find("] [") {
                let rest = &line[bracket_pos + 3..];
                if let Some(end_bracket) = rest.find(']') {
                    if let Ok(idx) = rest[..end_bracket].parse::<usize>() {
                        let name = rest[end_bracket + 2..].trim().to_string();
                        if !name.is_empty() {
                            devices.push(AudioInputDevice {
                                device_id: idx.to_string(),
                                name,
                                channels: 2,
                                sample_rate: 48000,
                                is_default: devices.is_empty(),
                            });
                        }
                    }
                }
            }
        }
    }

    Ok(devices)
}

// ============================================================
// Windows implementations
// ============================================================

#[cfg(target_os = "windows")]
fn list_audio_inputs_windows(ffmpeg_path: &str) -> Result<Vec<AudioInputDevice>, String> {
    let output = Command::new(ffmpeg_path)
        .args(["-f", "dshow", "-list_devices", "true", "-i", "dummy"])
        .output()
        .map_err(|e| format!("Failed to run FFmpeg: {}", e))?;

    let stderr = String::from_utf8_lossy(&output.stderr);
    super::windows::parse_dshow_devices(&stderr, "audio")
        .map(|names| {
            names.into_iter().enumerate().map(|(i, (id, name))| AudioInputDevice {
                device_id: id,
                name,
                channels: 2,
                sample_rate: 48000,
                is_default: i == 0,
            }).collect()
        })
}

// ============================================================
// Linux implementations
// ============================================================

#[cfg(target_os = "linux")]
fn list_audio_inputs_linux(_ffmpeg_path: &str) -> Result<Vec<AudioInputDevice>, String> {
    // Use pactl to list PulseAudio sources
    let output = Command::new("pactl")
        .args(["list", "sources", "short"])
        .output()
        .map_err(|_| "pactl not found. Install pulseaudio-utils package.".to_string())?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut devices = Vec::new();

    for line in stdout.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 2 {
            let device_id = parts[1].to_string();
            // Skip monitor sources (they're output monitors, not inputs)
            if !device_id.contains(".monitor") {
                devices.push(AudioInputDevice {
                    device_id: device_id.clone(),
                    name: device_id,
                    channels: 2,
                    sample_rate: 48000,
                    is_default: devices.is_empty(),
                });
            }
        }
    }

    Ok(devices)
}
