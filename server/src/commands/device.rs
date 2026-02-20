// Device Commands
// Handles device discovery and enumeration operations

use crate::app_state::AppState;
use crate::models::AudioInputDevice;
use crate::services::{DeviceDiscovery, ScreenCaptureService};
use serde_json::{json, Value};

/// Handle device-related commands.
///
/// Returns `None` for unrecognized commands, `Some(result)` for handled ones.
pub async fn handle(state: &AppState, command: &str, _payload: &Value) -> Option<Result<Value, String>> {
    match command {
        "list_cameras" => {
            let discovery = DeviceDiscovery::with_cache(state.ffmpeg_handler.get_ffmpeg_path(), state.device_cache.clone());
            let cameras = discovery.list_cameras_async().await;
            Some(cameras.map(|c| json!(c)))
        }
        "list_displays" => {
            let discovery = DeviceDiscovery::with_cache(state.ffmpeg_handler.get_ffmpeg_path(), state.device_cache.clone());
            let displays = discovery.list_displays_async().await;
            Some(displays.map(|d| json!(d)))
        }
        "list_audio_devices" => {
            // Use cpal for audio device enumeration (not FFmpeg) to ensure device IDs
            // match what cpal uses when starting capture
            let cpal_devices = state.audio_capture.list_input_devices();
            let devices: Vec<AudioInputDevice> = cpal_devices.into_iter().map(|d| {
                AudioInputDevice {
                    // Use hardware UID as the stable identifier
                    // Format: "HostId:DeviceUID" e.g. "CoreAudio:BuiltInMicrophoneDevice"
                    device_id: d.id.clone(),
                    name: d.name,
                    channels: d.channels.first().copied().unwrap_or(2) as u8,
                    sample_rate: d.sample_rates.first().copied().unwrap_or(48000),
                    is_default: d.is_default,
                }
            }).collect();
            log::info!("[list_audio_devices] Found {} devices via cpal:",
                devices.len()
            );
            for dev in &devices {
                log::info!("  - '{}' (id: {})", dev.name, dev.device_id);
            }
            Some(Ok(json!(devices)))
        }
        "list_capture_cards" => {
            let discovery = DeviceDiscovery::with_cache(state.ffmpeg_handler.get_ffmpeg_path(), state.device_cache.clone());
            let cards = discovery.list_capture_cards_async().await;
            Some(cards.map(|c| json!(c)))
        }
        "list_windows" => {
            // Use ScreenCaptureService for window enumeration (ScreenCaptureKit on macOS)
            let windows = ScreenCaptureService::list_windows_async().await;
            Some(Ok(json!(windows)))
        }
        "refresh_devices" => {
            // Return all device types at once using async parallel enumeration
            let discovery = DeviceDiscovery::with_cache(state.ffmpeg_handler.get_ffmpeg_path(), state.device_cache.clone());

            // Run device discovery and window enumeration in parallel
            let (all_devices, windows) = tokio::join!(
                discovery.refresh_devices_async(),
                ScreenCaptureService::list_windows_async()
            );

            let all_devices = match all_devices {
                Ok(d) => d,
                Err(e) => return Some(Err(e)),
            };

            // Use cpal for audio device enumeration (not FFmpeg) to ensure device IDs
            // match what cpal uses when starting capture
            let cpal_devices = state.audio_capture.list_input_devices();
            let audio_devices: Vec<AudioInputDevice> = cpal_devices.into_iter().map(|d| {
                AudioInputDevice {
                    // Use hardware UID as the stable identifier
                    device_id: d.id.clone(),
                    name: d.name,
                    channels: d.channels.first().copied().unwrap_or(2) as u8,
                    sample_rate: d.sample_rates.first().copied().unwrap_or(48000),
                    is_default: d.is_default,
                }
            }).collect();

            Some(Ok(json!({
                "cameras": all_devices.cameras,
                "displays": all_devices.displays,
                "windows": windows,
                "audioDevices": audio_devices,
                "captureCards": all_devices.capture_cards
            })))
        }

        _ => None,
    }
}
