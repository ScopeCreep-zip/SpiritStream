// Device Discovery Service
// Platform-specific device enumeration for cameras, displays, audio devices, and capture cards

use crate::models::{CameraDevice, DisplayInfo, AudioInputDevice, CaptureCardDevice, Resolution};
use std::process::Command;

/// Device discovery service for enumerating available input devices
pub struct DeviceDiscovery {
    ffmpeg_path: String,
}

impl DeviceDiscovery {
    /// Create a new DeviceDiscovery instance with the specified FFmpeg path
    pub fn new(ffmpeg_path: String) -> Self {
        Self { ffmpeg_path }
    }

    /// List available camera devices
    /// Uses FFmpeg device listing (avfoundation on macOS, dshow on Windows, v4l2 on Linux)
    pub fn list_cameras(&self) -> Result<Vec<CameraDevice>, String> {
        #[cfg(target_os = "macos")]
        {
            self.list_cameras_macos()
        }
        #[cfg(target_os = "windows")]
        {
            self.list_cameras_windows()
        }
        #[cfg(target_os = "linux")]
        {
            self.list_cameras_linux()
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
        {
            Err("Camera discovery not supported on this platform".to_string())
        }
    }

    /// List available displays for screen capture
    pub fn list_displays(&self) -> Result<Vec<DisplayInfo>, String> {
        #[cfg(target_os = "macos")]
        {
            self.list_displays_macos()
        }
        #[cfg(target_os = "windows")]
        {
            self.list_displays_windows()
        }
        #[cfg(target_os = "linux")]
        {
            self.list_displays_linux()
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
        {
            Err("Display discovery not supported on this platform".to_string())
        }
    }

    /// List available audio input devices
    pub fn list_audio_inputs(&self) -> Result<Vec<AudioInputDevice>, String> {
        #[cfg(target_os = "macos")]
        {
            self.list_audio_inputs_macos()
        }
        #[cfg(target_os = "windows")]
        {
            self.list_audio_inputs_windows()
        }
        #[cfg(target_os = "linux")]
        {
            self.list_audio_inputs_linux()
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
        {
            Err("Audio device discovery not supported on this platform".to_string())
        }
    }

    /// List available capture cards (HDMI capture devices)
    pub fn list_capture_cards(&self) -> Result<Vec<CaptureCardDevice>, String> {
        #[cfg(target_os = "macos")]
        {
            self.list_capture_cards_macos()
        }
        #[cfg(target_os = "windows")]
        {
            self.list_capture_cards_windows()
        }
        #[cfg(target_os = "linux")]
        {
            self.list_capture_cards_linux()
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
        {
            Err("Capture card discovery not supported on this platform".to_string())
        }
    }

    // ============================================================
    // macOS implementations using AVFoundation via FFmpeg
    // ============================================================

    #[cfg(target_os = "macos")]
    fn list_cameras_macos(&self) -> Result<Vec<CameraDevice>, String> {
        // Use FFmpeg to list AVFoundation devices
        let output = Command::new(&self.ffmpeg_path)
            .args(["-f", "avfoundation", "-list_devices", "true", "-i", ""])
            .output()
            .map_err(|e| format!("Failed to run FFmpeg: {}", e))?;

        // FFmpeg writes device list to stderr
        let stderr = String::from_utf8_lossy(&output.stderr);
        Self::parse_avfoundation_cameras(&stderr)
    }

    #[cfg(target_os = "macos")]
    fn parse_avfoundation_cameras(output: &str) -> Result<Vec<CameraDevice>, String> {
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
                                });
                            }
                        }
                    }
                }
            }
        }

        Ok(cameras)
    }

    #[cfg(target_os = "macos")]
    fn list_displays_macos(&self) -> Result<Vec<DisplayInfo>, String> {
        // Use FFmpeg to list screen capture devices
        let output = Command::new(&self.ffmpeg_path)
            .args(["-f", "avfoundation", "-list_devices", "true", "-i", ""])
            .output()
            .map_err(|e| format!("Failed to run FFmpeg: {}", e))?;

        let stderr = String::from_utf8_lossy(&output.stderr);
        Self::parse_avfoundation_displays(&stderr)
    }

    #[cfg(target_os = "macos")]
    fn parse_avfoundation_displays(output: &str) -> Result<Vec<DisplayInfo>, String> {
        let mut displays = Vec::new();
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
                if let Some(bracket_pos) = line.find("] [") {
                    let rest = &line[bracket_pos + 3..];
                    if let Some(end_bracket) = rest.find(']') {
                        if let Ok(idx) = rest[..end_bracket].parse::<usize>() {
                            let name = rest[end_bracket + 2..].trim().to_string();
                            // Only include screen capture devices
                            if name.contains("Capture screen") {
                                displays.push(DisplayInfo {
                                    display_id: idx.to_string(),
                                    name: format!("Display {}", displays.len() + 1),
                                    width: 1920,  // Default, actual resolution needs screen query
                                    height: 1080,
                                    is_primary: displays.is_empty(),
                                });
                            }
                        }
                    }
                }
            }
        }

        // If no displays found via FFmpeg, add a default one
        if displays.is_empty() {
            displays.push(DisplayInfo {
                display_id: "0".to_string(),
                name: "Main Display".to_string(),
                width: 1920,
                height: 1080,
                is_primary: true,
            });
        }

        Ok(displays)
    }

    #[cfg(target_os = "macos")]
    fn list_audio_inputs_macos(&self) -> Result<Vec<AudioInputDevice>, String> {
        let output = Command::new(&self.ffmpeg_path)
            .args(["-f", "avfoundation", "-list_devices", "true", "-i", ""])
            .output()
            .map_err(|e| format!("Failed to run FFmpeg: {}", e))?;

        let stderr = String::from_utf8_lossy(&output.stderr);
        Self::parse_avfoundation_audio(&stderr)
    }

    #[cfg(target_os = "macos")]
    fn parse_avfoundation_audio(output: &str) -> Result<Vec<AudioInputDevice>, String> {
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

    #[cfg(target_os = "macos")]
    fn list_capture_cards_macos(&self) -> Result<Vec<CaptureCardDevice>, String> {
        // On macOS, capture cards appear as video devices in AVFoundation
        // We look for known capture card names
        let cameras = self.list_cameras_macos()?;
        let capture_card_keywords = ["elgato", "capture", "cam link", "game capture", "avermedia"];

        let capture_cards = cameras
            .into_iter()
            .filter(|c| {
                let name_lower = c.name.to_lowercase();
                capture_card_keywords.iter().any(|k| name_lower.contains(k))
            })
            .map(|c| CaptureCardDevice {
                device_id: c.device_id,
                name: c.name,
                inputs: vec!["hdmi".to_string()],
            })
            .collect();

        Ok(capture_cards)
    }

    // ============================================================
    // Windows implementations using DirectShow
    // ============================================================

    #[cfg(target_os = "windows")]
    fn list_cameras_windows(&self) -> Result<Vec<CameraDevice>, String> {
        let output = Command::new(&self.ffmpeg_path)
            .args(["-f", "dshow", "-list_devices", "true", "-i", "dummy"])
            .output()
            .map_err(|e| format!("Failed to run FFmpeg: {}", e))?;

        let stderr = String::from_utf8_lossy(&output.stderr);
        Self::parse_dshow_devices(&stderr, "video")
            .map(|names| {
                names.into_iter().map(|(id, name)| CameraDevice {
                    device_id: id,
                    name,
                    resolutions: vec![
                        Resolution { width: 1920, height: 1080, fps: vec![30, 60] },
                        Resolution { width: 1280, height: 720, fps: vec![30, 60] },
                    ],
                }).collect()
            })
    }

    #[cfg(target_os = "windows")]
    fn list_displays_windows(&self) -> Result<Vec<DisplayInfo>, String> {
        // Windows uses gdigrab for screen capture
        // List available monitors
        Ok(vec![DisplayInfo {
            display_id: "desktop".to_string(),
            name: "Primary Display".to_string(),
            width: 1920,
            height: 1080,
            is_primary: true,
        }])
    }

    #[cfg(target_os = "windows")]
    fn list_audio_inputs_windows(&self) -> Result<Vec<AudioInputDevice>, String> {
        let output = Command::new(&self.ffmpeg_path)
            .args(["-f", "dshow", "-list_devices", "true", "-i", "dummy"])
            .output()
            .map_err(|e| format!("Failed to run FFmpeg: {}", e))?;

        let stderr = String::from_utf8_lossy(&output.stderr);
        Self::parse_dshow_devices(&stderr, "audio")
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

    #[cfg(target_os = "windows")]
    fn list_capture_cards_windows(&self) -> Result<Vec<CaptureCardDevice>, String> {
        let cameras = self.list_cameras_windows()?;
        let capture_card_keywords = ["elgato", "capture", "cam link", "game capture", "avermedia"];

        let capture_cards = cameras
            .into_iter()
            .filter(|c| {
                let name_lower = c.name.to_lowercase();
                capture_card_keywords.iter().any(|k| name_lower.contains(k))
            })
            .map(|c| CaptureCardDevice {
                device_id: c.device_id,
                name: c.name,
                inputs: vec!["hdmi".to_string()],
            })
            .collect();

        Ok(capture_cards)
    }

    #[cfg(target_os = "windows")]
    fn parse_dshow_devices(output: &str, device_type: &str) -> Result<Vec<(String, String)>, String> {
        let mut devices = Vec::new();
        let mut in_section = false;
        let section_marker = format!("DirectShow {} devices", device_type);

        for line in output.lines() {
            if line.contains(&section_marker) {
                in_section = true;
                continue;
            }
            if in_section && line.contains("DirectShow") && !line.contains(&section_marker) {
                break;
            }
            if in_section && line.contains("]  \"") {
                // Parse lines like '[dshow @ ...] "Device Name"'
                if let Some(start) = line.find('"') {
                    if let Some(end) = line[start+1..].find('"') {
                        let name = line[start+1..start+1+end].to_string();
                        devices.push((name.clone(), name));
                    }
                }
            }
        }

        Ok(devices)
    }

    // ============================================================
    // Linux implementations using V4L2
    // ============================================================

    #[cfg(target_os = "linux")]
    fn list_cameras_linux(&self) -> Result<Vec<CameraDevice>, String> {
        let output = Command::new("v4l2-ctl")
            .args(["--list-devices"])
            .output()
            .map_err(|_| "v4l2-ctl not found. Install v4l-utils package.".to_string())?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        Self::parse_v4l2_cameras(&stdout)
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
                    });
                }
                current_name = None;
            }
        }

        Ok(cameras)
    }

    #[cfg(target_os = "linux")]
    fn list_displays_linux(&self) -> Result<Vec<DisplayInfo>, String> {
        // Use xrandr to list displays
        let output = Command::new("xrandr")
            .output()
            .map_err(|_| "xrandr not found".to_string())?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut displays = Vec::new();

        for line in stdout.lines() {
            if line.contains(" connected") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if let Some(name) = parts.first() {
                    let is_primary = line.contains("primary");

                    // Try to parse resolution
                    let (width, height) = parts.iter()
                        .find(|p| p.contains('x') && p.chars().next().map_or(false, |c| c.is_digit(10)))
                        .and_then(|res| {
                            let dims: Vec<&str> = res.split(|c| c == 'x' || c == '+').collect();
                            if dims.len() >= 2 {
                                Some((
                                    dims[0].parse().unwrap_or(1920),
                                    dims[1].parse().unwrap_or(1080),
                                ))
                            } else {
                                None
                            }
                        })
                        .unwrap_or((1920, 1080));

                    displays.push(DisplayInfo {
                        display_id: name.to_string(),
                        name: name.to_string(),
                        width,
                        height,
                        is_primary,
                    });
                }
            }
        }

        if displays.is_empty() {
            displays.push(DisplayInfo {
                display_id: ":0".to_string(),
                name: "Display :0".to_string(),
                width: 1920,
                height: 1080,
                is_primary: true,
            });
        }

        Ok(displays)
    }

    #[cfg(target_os = "linux")]
    fn list_audio_inputs_linux(&self) -> Result<Vec<AudioInputDevice>, String> {
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

    #[cfg(target_os = "linux")]
    fn list_capture_cards_linux(&self) -> Result<Vec<CaptureCardDevice>, String> {
        let cameras = self.list_cameras_linux()?;
        let capture_card_keywords = ["elgato", "capture", "cam link", "game capture", "avermedia"];

        let capture_cards = cameras
            .into_iter()
            .filter(|c| {
                let name_lower = c.name.to_lowercase();
                capture_card_keywords.iter().any(|k| name_lower.contains(k))
            })
            .map(|c| CaptureCardDevice {
                device_id: c.device_id,
                name: c.name,
                inputs: vec!["hdmi".to_string()],
            })
            .collect();

        Ok(capture_cards)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_discovery() -> DeviceDiscovery {
        // Use "ffmpeg" for tests - assumes FFmpeg is in PATH for test environment
        DeviceDiscovery::new("ffmpeg".to_string())
    }

    #[test]
    fn test_list_cameras() {
        // This test will work on any platform
        let discovery = test_discovery();
        let result = discovery.list_cameras();
        // Should not error, but may return empty list if no cameras
        assert!(result.is_ok());
    }

    #[test]
    fn test_list_displays() {
        let discovery = test_discovery();
        let result = discovery.list_displays();
        assert!(result.is_ok());
        // Should always have at least one display
        assert!(!result.unwrap().is_empty());
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn test_parse_avfoundation_cameras() {
        let sample_output = r#"
[AVFoundation indev @ 0x7f9a8b800000] AVFoundation video devices:
[AVFoundation indev @ 0x7f9a8b800000] [0] FaceTime HD Camera
[AVFoundation indev @ 0x7f9a8b800000] [1] Capture screen 0
[AVFoundation indev @ 0x7f9a8b800000] AVFoundation audio devices:
[AVFoundation indev @ 0x7f9a8b800000] [0] Built-in Microphone
"#;
        let cameras = DeviceDiscovery::parse_avfoundation_cameras(sample_output).unwrap();
        assert_eq!(cameras.len(), 1);
        assert_eq!(cameras[0].name, "FaceTime HD Camera");
        assert_eq!(cameras[0].device_id, "0");
    }
}
