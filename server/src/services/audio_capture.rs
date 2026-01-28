// Audio Capture Service
// Uses cpal for native audio capture (microphones, line-in, system audio loopback)

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, DeviceId, Host, SampleFormat, Stream, StreamConfig};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::broadcast;

/// Information about an audio device
#[derive(Debug, Clone, serde::Serialize)]
pub struct AudioDeviceInfo {
    pub id: String,
    pub name: String,
    pub is_input: bool,
    pub is_default: bool,
    pub sample_rates: Vec<u32>,
    pub channels: Vec<u16>,
}

/// Audio buffer containing PCM samples
#[derive(Debug, Clone)]
pub struct AudioBuffer {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub channels: u16,
    pub timestamp_ms: u64,
}

/// Audio capture configuration
#[derive(Debug, Clone)]
pub struct AudioCaptureConfig {
    pub sample_rate: Option<u32>,
    pub channels: Option<u16>,
}

impl Default for AudioCaptureConfig {
    fn default() -> Self {
        Self {
            sample_rate: None, // Use device default
            channels: None,    // Use device default
        }
    }
}

/// Type of audio source
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum AudioSourceType {
    /// Input device (microphone, line-in)
    Input,
    /// Output loopback (system audio capture) - Windows only via WASAPI
    OutputLoopback,
}

/// Active audio capture stream
struct ActiveStream {
    _stream: Stream,
    device_name: String,
    source_type: AudioSourceType,
    stop_flag: Arc<AtomicBool>,
}

/// Service for managing audio capture
pub struct AudioCaptureService {
    host: Host,
    active_streams: Mutex<HashMap<String, ActiveStream>>,
}

impl AudioCaptureService {
    pub fn new() -> Self {
        let host = cpal::default_host();
        Self {
            host,
            active_streams: Mutex::new(HashMap::new()),
        }
    }

    /// List available input devices (microphones, line-in)
    pub fn list_input_devices(&self) -> Vec<AudioDeviceInfo> {
        let default_input_id = self.host.default_input_device()
            .and_then(|d| d.id().ok());

        self.host
            .input_devices()
            .map(|devices| {
                devices
                    .filter_map(|device| self.device_to_info(&device, true, &default_input_id))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// List available output devices (for loopback capture on Windows)
    pub fn list_output_devices(&self) -> Vec<AudioDeviceInfo> {
        let default_output_id = self.host.default_output_device()
            .and_then(|d| d.id().ok());

        self.host
            .output_devices()
            .map(|devices| {
                devices
                    .filter_map(|device| self.device_to_info(&device, false, &default_output_id))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get default input device info
    pub fn default_input_device(&self) -> Option<AudioDeviceInfo> {
        let default_id = self.host.default_input_device()
            .and_then(|d| d.id().ok());
        self.host
            .default_input_device()
            .and_then(|device| self.device_to_info(&device, true, &default_id))
    }

    /// Get default output device info
    pub fn default_output_device(&self) -> Option<AudioDeviceInfo> {
        let default_id = self.host.default_output_device()
            .and_then(|d| d.id().ok());
        self.host
            .default_output_device()
            .and_then(|device| self.device_to_info(&device, false, &default_id))
    }

    /// Check if output loopback is supported on this platform
    pub fn is_loopback_supported() -> bool {
        #[cfg(target_os = "windows")]
        {
            true // WASAPI supports loopback
        }
        #[cfg(not(target_os = "windows"))]
        {
            false // Other platforms need different approaches
        }
    }

    /// Start capturing from an input device
    pub fn start_input_capture(
        &self,
        device_id: &str,
        config: AudioCaptureConfig,
    ) -> Result<broadcast::Receiver<AudioBuffer>, String> {
        let device = self.find_input_device(device_id)?;
        self.start_capture_internal(device_id, device, config, AudioSourceType::Input)
    }

    /// Start capturing system audio (output loopback) - Windows only
    #[cfg(target_os = "windows")]
    pub fn start_loopback_capture(
        &self,
        device_id: &str,
        config: AudioCaptureConfig,
    ) -> Result<broadcast::Receiver<AudioBuffer>, String> {
        let device = self.find_output_device(device_id)?;
        self.start_capture_internal(device_id, device, config, AudioSourceType::OutputLoopback)
    }

    #[cfg(not(target_os = "windows"))]
    pub fn start_loopback_capture(
        &self,
        _device_id: &str,
        _config: AudioCaptureConfig,
    ) -> Result<broadcast::Receiver<AudioBuffer>, String> {
        Err("Output loopback is only supported on Windows via WASAPI. On macOS, use screen capture with audio enabled.".to_string())
    }

    /// Stop capturing from a device
    pub fn stop_capture(&self, device_id: &str) -> Result<(), String> {
        let mut streams = self.active_streams.lock().unwrap();

        if let Some(active) = streams.remove(device_id) {
            active.stop_flag.store(true, Ordering::Relaxed);
            log::info!("Stopped audio capture for device: {} ({})", device_id, active.device_name);
            Ok(())
        } else {
            Err(format!("No active capture for device: {}", device_id))
        }
    }

    /// Stop all active captures
    pub fn stop_all(&self) {
        let mut streams = self.active_streams.lock().unwrap();
        for (id, active) in streams.drain() {
            active.stop_flag.store(true, Ordering::Relaxed);
            log::info!("Stopped audio capture for device: {}", id);
        }
    }

    /// Check if a device is currently being captured
    pub fn is_capturing(&self, device_id: &str) -> bool {
        let streams = self.active_streams.lock().unwrap();
        streams.contains_key(device_id)
    }

    /// Get count of active captures
    pub fn active_capture_count(&self) -> usize {
        let streams = self.active_streams.lock().unwrap();
        streams.len()
    }

    /// Get list of active capture IDs
    pub fn active_capture_ids(&self) -> Vec<String> {
        let streams = self.active_streams.lock().unwrap();
        streams.keys().cloned().collect()
    }

    /// Get info about all active captures (device_id, device_name, source_type)
    pub fn active_captures_info(&self) -> Vec<(String, String, AudioSourceType)> {
        let streams = self.active_streams.lock().unwrap();
        streams
            .iter()
            .map(|(id, stream)| (id.clone(), stream.device_name.clone(), stream.source_type))
            .collect()
    }

    // Internal helper methods

    fn device_to_info(&self, device: &Device, is_input: bool, default_id: &Option<DeviceId>) -> Option<AudioDeviceInfo> {
        let device_id = device.id().ok()?;
        let description = device.description().ok()?;
        let name = description.name().to_string();
        let id = device_id.1.clone(); // DeviceId is (HostId, String), get the String part

        let mut sample_rates = Vec::new();
        let mut channels = Vec::new();

        // Handle input and output configs separately since they have different types
        if is_input {
            if let Ok(configs) = device.supported_input_configs() {
                for config in configs {
                    // Collect common sample rates within supported range
                    for rate in [44100, 48000, 96000] {
                        if config.min_sample_rate() <= rate && config.max_sample_rate() >= rate {
                            if !sample_rates.contains(&rate) {
                                sample_rates.push(rate);
                            }
                        }
                    }

                    let ch = config.channels();
                    if !channels.contains(&ch) {
                        channels.push(ch);
                    }
                }
            }
        } else {
            if let Ok(configs) = device.supported_output_configs() {
                for config in configs {
                    // Collect common sample rates within supported range
                    for rate in [44100, 48000, 96000] {
                        if config.min_sample_rate() <= rate && config.max_sample_rate() >= rate {
                            if !sample_rates.contains(&rate) {
                                sample_rates.push(rate);
                            }
                        }
                    }

                    let ch = config.channels();
                    if !channels.contains(&ch) {
                        channels.push(ch);
                    }
                }
            }
        }

        sample_rates.sort();
        channels.sort();

        let is_default = default_id.as_ref().map(|d| *d == device_id).unwrap_or(false);

        Some(AudioDeviceInfo {
            id,
            name,
            is_input,
            is_default,
            sample_rates,
            channels,
        })
    }

    fn find_input_device(&self, device_id: &str) -> Result<Device, String> {
        self.host
            .input_devices()
            .map_err(|e| format!("Failed to enumerate input devices: {}", e))?
            .find(|d| d.id().map(|id| id.1 == device_id).unwrap_or(false))
            .ok_or_else(|| format!("Input device not found: {}", device_id))
    }

    #[cfg(target_os = "windows")]
    fn find_output_device(&self, device_id: &str) -> Result<Device, String> {
        self.host
            .output_devices()
            .map_err(|e| format!("Failed to enumerate output devices: {}", e))?
            .find(|d| d.id().map(|id| id.1 == device_id).unwrap_or(false))
            .ok_or_else(|| format!("Output device not found: {}", device_id))
    }

    fn start_capture_internal(
        &self,
        device_id: &str,
        device: Device,
        _config: AudioCaptureConfig,
        source_type: AudioSourceType,
    ) -> Result<broadcast::Receiver<AudioBuffer>, String> {
        // Check if already capturing
        {
            let streams = self.active_streams.lock().unwrap();
            if streams.contains_key(device_id) {
                return Err(format!("Already capturing from device: {}", device_id));
            }
        }

        // Get default config for the device
        let supported_config = device.default_input_config()
            .map_err(|e| format!("Failed to get device config: {}", e))?;

        let sample_format = supported_config.sample_format();
        let stream_config: StreamConfig = supported_config.into();
        let sample_rate = stream_config.sample_rate;
        let channels = stream_config.channels;

        // Create broadcast channel
        let (tx, rx) = broadcast::channel::<AudioBuffer>(64);
        let stop_flag = Arc::new(AtomicBool::new(false));
        let stop_flag_clone = stop_flag.clone();

        let start_time = std::time::Instant::now();

        // Build the stream based on sample format
        let stream = match sample_format {
            SampleFormat::F32 => {
                let tx = tx.clone();
                device.build_input_stream(
                    &stream_config,
                    move |data: &[f32], _: &cpal::InputCallbackInfo| {
                        if stop_flag_clone.load(Ordering::Relaxed) {
                            return;
                        }
                        let buffer = AudioBuffer {
                            samples: data.to_vec(),
                            sample_rate,
                            channels,
                            timestamp_ms: start_time.elapsed().as_millis() as u64,
                        };
                        let _ = tx.send(buffer);
                    },
                    |err| log::error!("Audio stream error: {}", err),
                    None,
                )
            }
            SampleFormat::I16 => {
                let tx = tx.clone();
                device.build_input_stream(
                    &stream_config,
                    move |data: &[i16], _: &cpal::InputCallbackInfo| {
                        if stop_flag_clone.load(Ordering::Relaxed) {
                            return;
                        }
                        // Convert i16 to f32
                        let samples: Vec<f32> = data.iter()
                            .map(|&s| s as f32 / i16::MAX as f32)
                            .collect();
                        let buffer = AudioBuffer {
                            samples,
                            sample_rate,
                            channels,
                            timestamp_ms: start_time.elapsed().as_millis() as u64,
                        };
                        let _ = tx.send(buffer);
                    },
                    |err| log::error!("Audio stream error: {}", err),
                    None,
                )
            }
            SampleFormat::U16 => {
                let tx = tx.clone();
                device.build_input_stream(
                    &stream_config,
                    move |data: &[u16], _: &cpal::InputCallbackInfo| {
                        if stop_flag_clone.load(Ordering::Relaxed) {
                            return;
                        }
                        // Convert u16 to f32
                        let samples: Vec<f32> = data.iter()
                            .map(|&s| (s as f32 / u16::MAX as f32) * 2.0 - 1.0)
                            .collect();
                        let buffer = AudioBuffer {
                            samples,
                            sample_rate,
                            channels,
                            timestamp_ms: start_time.elapsed().as_millis() as u64,
                        };
                        let _ = tx.send(buffer);
                    },
                    |err| log::error!("Audio stream error: {}", err),
                    None,
                )
            }
            _ => return Err(format!("Unsupported sample format: {:?}", sample_format)),
        }
        .map_err(|e| format!("Failed to build audio stream: {}", e))?;

        stream.play().map_err(|e| format!("Failed to start audio stream: {}", e))?;

        // Store active stream
        let device_name = device.description()
            .map(|d| d.name().to_string())
            .unwrap_or_else(|_| device_id.to_string());
        {
            let mut streams = self.active_streams.lock().unwrap();
            streams.insert(
                device_id.to_string(),
                ActiveStream {
                    _stream: stream,
                    device_name: device_name.clone(),
                    source_type,
                    stop_flag,
                },
            );
        }

        log::info!(
            "Started {:?} audio capture: {} ({}Hz, {} channels)",
            source_type,
            device_name,
            sample_rate,
            channels
        );

        Ok(rx)
    }
}

impl Default for AudioCaptureService {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for AudioCaptureService {
    fn drop(&mut self) {
        self.stop_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_input_devices() {
        let service = AudioCaptureService::new();

        println!("Input devices:");
        for device in service.list_input_devices() {
            println!(
                "  - {} (default: {}, rates: {:?}, channels: {:?})",
                device.name, device.is_default, device.sample_rates, device.channels
            );
        }
    }

    #[test]
    fn test_list_output_devices() {
        let service = AudioCaptureService::new();

        println!("Output devices:");
        for device in service.list_output_devices() {
            println!(
                "  - {} (default: {}, rates: {:?}, channels: {:?})",
                device.name, device.is_default, device.sample_rates, device.channels
            );
        }
    }

    #[test]
    fn test_default_devices() {
        let service = AudioCaptureService::new();

        if let Some(input) = service.default_input_device() {
            println!("Default input: {}", input.name);
        } else {
            println!("No default input device");
        }

        if let Some(output) = service.default_output_device() {
            println!("Default output: {}", output.name);
        } else {
            println!("No default output device");
        }
    }
}
