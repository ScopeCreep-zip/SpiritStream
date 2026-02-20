// Display/monitor enumeration
// Platform-specific display discovery
//
// macOS/Windows: Delegates to ScreenCaptureService (scap) for native display IDs
// (CGDirectDisplayID on macOS, HMONITOR on Windows). AVFoundation names are used
// only for go2rtc device_name enrichment.
//
// Linux: Uses xrandr (scap returns empty on Linux).

use crate::models::DisplayInfo;
use std::process::Command;

pub(super) fn list_displays_sync(ffmpeg_path: &str) -> Result<Vec<DisplayInfo>, String> {
    #[cfg(target_os = "macos")]
    {
        list_displays_macos(ffmpeg_path)
    }
    #[cfg(target_os = "windows")]
    {
        list_displays_windows(ffmpeg_path)
    }
    #[cfg(target_os = "linux")]
    {
        list_displays_linux(ffmpeg_path)
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        let _ = ffmpeg_path;
        Err("Display discovery not supported on this platform".to_string())
    }
}

// ============================================================
// macOS implementation — scap primary, AVFoundation enrichment
// ============================================================

#[cfg(target_os = "macos")]
fn list_displays_macos(ffmpeg_path: &str) -> Result<Vec<DisplayInfo>, String> {
    use crate::services::ScreenCaptureService;

    let scap_displays = ScreenCaptureService::list_displays();
    if scap_displays.is_empty() {
        return Err("No displays found. Screen recording permission may be required.".to_string());
    }

    // Get AVFoundation screen names for go2rtc (e.g., "Capture screen 0")
    let avf_names = parse_avfoundation_screen_names(ffmpeg_path);

    Ok(scap_displays
        .iter()
        .enumerate()
        .map(|(i, scap)| {
            DisplayInfo {
                display_id: scap.display_id.clone(), // CGDirectDisplayID as string
                name: scap.name.clone(),
                device_name: avf_names
                    .get(i)
                    .cloned()
                    .or_else(|| scap.device_name.clone())
                    .or_else(|| Some(format!("Capture screen {}", i))),
                width: scap.width,
                height: scap.height,
                is_primary: scap.is_primary,
            }
        })
        .collect())
}

/// Parse AVFoundation output to extract just screen device names (e.g., "Capture screen 0").
/// Returns Vec<String> of names in order — used solely for go2rtc device_name enrichment.
#[cfg(target_os = "macos")]
fn parse_avfoundation_screen_names(ffmpeg_path: &str) -> Vec<String> {
    let output = match Command::new(ffmpeg_path)
        .args(["-f", "avfoundation", "-list_devices", "true", "-i", ""])
        .output()
    {
        Ok(output) => output,
        Err(e) => {
            log::debug!("FFmpeg AVFoundation enumeration failed: {}", e);
            return Vec::new();
        }
    };

    let stderr = String::from_utf8_lossy(&output.stderr);
    let mut screen_names = Vec::new();
    let mut in_video_section = false;

    for line in stderr.lines() {
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
                    // Check this is a parseable index entry
                    if rest[..end_bracket].parse::<usize>().is_ok() {
                        let name = rest[end_bracket + 2..].trim().to_string();
                        if name.contains("Capture screen") {
                            screen_names.push(name);
                        }
                    }
                }
            }
        }
    }

    screen_names
}

// ============================================================
// Windows implementation — scap primary
// ============================================================

#[cfg(target_os = "windows")]
fn list_displays_windows(_ffmpeg_path: &str) -> Result<Vec<DisplayInfo>, String> {
    use crate::services::ScreenCaptureService;

    let scap_displays = ScreenCaptureService::list_displays();
    if scap_displays.is_empty() {
        // scap unavailable — return single default
        return Ok(vec![DisplayInfo {
            display_id: "0".to_string(),
            name: "Primary Display".to_string(),
            device_name: None,
            width: 1920,
            height: 1080,
            is_primary: true,
        }]);
    }

    Ok(scap_displays
        .iter()
        .map(|scap| DisplayInfo {
            display_id: scap.display_id.clone(),
            name: scap.name.clone(),
            device_name: scap.device_name.clone(),
            width: scap.width,
            height: scap.height,
            is_primary: scap.is_primary,
        })
        .collect())
}

// ============================================================
// Linux implementation — xrandr (scap returns empty on Linux)
// ============================================================

#[cfg(target_os = "linux")]
fn list_displays_linux(_ffmpeg_path: &str) -> Result<Vec<DisplayInfo>, String> {
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
                let (width, height) = parts
                    .iter()
                    .find(|p| {
                        p.contains('x')
                            && p.chars().next().map_or(false, |c| c.is_ascii_digit())
                    })
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
                    device_name: None,
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
            device_name: None,
            width: 1920,
            height: 1080,
            is_primary: true,
        });
    }

    Ok(displays)
}
