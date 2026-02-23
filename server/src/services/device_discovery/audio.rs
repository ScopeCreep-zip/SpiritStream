// Audio device enumeration
// Uses cpal (native audio API) instead of FFmpeg subprocess for reliable device discovery.
// Provides persistent hardware UIDs, accurate channel/sample rate info.

use crate::models::AudioInputDevice;
use crate::services::AudioCaptureService;

pub(super) fn list_audio_inputs_sync(_ffmpeg_path: &str) -> Result<Vec<AudioInputDevice>, String> {
    let service = AudioCaptureService::new();
    let devices = service.list_input_devices();

    Ok(devices
        .into_iter()
        .map(|d| AudioInputDevice {
            device_id: d.id,
            name: d.name,
            channels: d.channels.first().copied().unwrap_or(2) as u8,
            sample_rate: d.sample_rates.last().copied().unwrap_or(48000),
            is_default: d.is_default,
        })
        .collect())
}
