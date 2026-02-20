// Window enumeration and Windows DirectShow parsing
// Per-platform window listing and shared DirectShow device parsing

use crate::models::CaptureCardDevice;

/// Parse DirectShow device listing from FFmpeg stderr output
/// Used by both camera and audio enumeration on Windows
#[cfg(target_os = "windows")]
pub(super) fn parse_dshow_devices(output: &str, device_type: &str) -> Result<Vec<(String, String)>, String> {
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

/// Filter cameras list to find capture cards by known keywords
pub(super) fn filter_capture_cards(
    cameras: Vec<crate::models::CameraDevice>,
) -> Vec<CaptureCardDevice> {
    let capture_card_keywords = ["elgato", "capture", "cam link", "game capture", "avermedia"];

    cameras
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
        .collect()
}
