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
#[serde(rename_all = "camelCase")]
pub struct AudioDeviceInfo {
    /// Stable hardware UID (CoreAudio UID on macOS, device path on Linux, etc.)
    /// This is the preferred identifier for device matching
    pub id: String,
    /// Human-readable device name
    pub name: String,
    /// Whether this is an input device
    pub is_input: bool,
    /// Whether this is the system default device
    pub is_default: bool,
    /// Supported sample rates
    pub sample_rates: Vec<u32>,
    /// Supported channel counts
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
    /// Maps source_id -> device_id for proper cleanup
    /// This solves the mismatch where active_streams uses device names as keys
    /// but cleanup logic needs to compare against source UUIDs
    source_to_device: Mutex<HashMap<String, String>>,
    /// Cached input devices for faster lookup (refreshed on demand)
    cached_input_devices: Mutex<Option<Vec<(Device, String, String)>>>, // (device, name, uid)
    /// Last cache refresh time
    cache_time: Mutex<Option<std::time::Instant>>,
}

/// Cache duration for device list (5 seconds)
const DEVICE_CACHE_DURATION: std::time::Duration = std::time::Duration::from_secs(5);

impl AudioCaptureService {
    pub fn new() -> Self {
        let host = cpal::default_host();
        Self {
            host,
            active_streams: Mutex::new(HashMap::new()),
            source_to_device: Mutex::new(HashMap::new()),
            cached_input_devices: Mutex::new(None),
            cache_time: Mutex::new(None),
        }
    }

    /// Pre-warm the device cache for faster first capture
    /// Call this during server startup
    pub fn warm_cache(&self) {
        log::info!("[AudioCapture] Warming device cache...");
        let start = std::time::Instant::now();
        let _ = self.get_cached_input_devices();
        log::info!("[AudioCapture] Device cache warmed in {:?}", start.elapsed());
    }

    /// Get cached input devices, refreshing if cache is stale
    fn get_cached_input_devices(&self) -> Vec<(Device, String, String)> {
        let mut cache = self.cached_input_devices.lock().unwrap();
        let mut cache_time = self.cache_time.lock().unwrap();

        // Check if cache is still valid
        let cache_valid = cache_time
            .map(|t| t.elapsed() < DEVICE_CACHE_DURATION)
            .unwrap_or(false);

        if cache_valid && cache.is_some() {
            return cache.clone().unwrap();
        }

        // Refresh cache
        log::debug!("[AudioCapture] Refreshing device cache...");
        let devices: Vec<_> = self.host
            .input_devices()
            .map(|iter| iter.collect())
            .unwrap_or_default();

        let cached: Vec<(Device, String, String)> = devices
            .into_iter()
            .filter_map(|device| {
                let name = device.description().ok()?.name().to_string();
                let uid = device.id().ok()?.to_string();
                Some((device, name, uid))
            })
            .collect();

        log::debug!("[AudioCapture] Cached {} input devices", cached.len());
        *cache = Some(cached.clone());
        *cache_time = Some(std::time::Instant::now());
        cached
    }

    /// Invalidate the device cache (call when devices change)
    pub fn invalidate_cache(&self) {
        let mut cache = self.cached_input_devices.lock().unwrap();
        let mut cache_time = self.cache_time.lock().unwrap();
        *cache = None;
        *cache_time = None;
        log::debug!("[AudioCapture] Device cache invalidated");
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

    /// Start capturing from an input device, tracking by source_id
    ///
    /// This method tracks the mapping from source_id to device_id so that
    /// cleanup logic can properly identify which captures to stop when
    /// comparing against source UUIDs (not device names).
    pub fn start_input_capture_for_source(
        &self,
        source_id: &str,
        device_id_or_name: &str,
        config: AudioCaptureConfig,
    ) -> Result<broadcast::Receiver<AudioBuffer>, String> {
        let device = self.find_input_device(device_id_or_name)?;
        let actual_device_id = device.description()
            .map(|d| d.name().to_string())
            .unwrap_or_else(|_| device_id_or_name.to_string());

        // Track the mapping before starting capture
        self.source_to_device
            .lock()
            .map_err(|_| "Source-to-device mapping lock poisoned".to_string())?
            .insert(source_id.to_string(), actual_device_id.clone());

        log::info!("Tracking source '{}' -> device '{}'", source_id, actual_device_id);

        self.start_capture_internal(&actual_device_id, device, config, AudioSourceType::Input)
    }

    /// Stop capturing by source ID
    ///
    /// Looks up the device_id associated with this source and stops that capture.
    pub fn stop_capture_for_source(&self, source_id: &str) -> Result<(), String> {
        let device_id = {
            let mapping = self.source_to_device
                .lock()
                .map_err(|_| "Source-to-device mapping lock poisoned".to_string())?;
            mapping.get(source_id).cloned()
        };

        if let Some(device_id) = device_id {
            self.stop_capture(&device_id)?;
            self.source_to_device
                .lock()
                .map_err(|_| "Source-to-device mapping lock poisoned".to_string())?
                .remove(source_id);
            log::info!("Stopped capture for source '{}' (device: '{}')", source_id, device_id);
            Ok(())
        } else {
            Err(format!("No capture tracked for source: {}", source_id))
        }
    }

    /// Get list of active source IDs (not device IDs)
    ///
    /// Returns the source UUIDs that have active captures, which can be
    /// compared against the list of sources that should be monitored.
    pub fn active_source_ids(&self) -> Vec<String> {
        self.source_to_device
            .lock()
            .map(|mapping| mapping.keys().cloned().collect())
            .unwrap_or_default()
    }

    /// Check if a source is currently being captured
    pub fn is_capturing_source(&self, source_id: &str) -> bool {
        self.source_to_device
            .lock()
            .map(|mapping| mapping.contains_key(source_id))
            .unwrap_or(false)
    }

    /// Stop capturing from a device
    pub fn stop_capture(&self, device_id: &str) -> Result<(), String> {
        let mut streams = self.active_streams
            .lock()
            .map_err(|_| "Active streams lock poisoned".to_string())?;

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
        if let Ok(mut streams) = self.active_streams.lock() {
            for (id, active) in streams.drain() {
                active.stop_flag.store(true, Ordering::Relaxed);
                log::info!("Stopped audio capture for device: {}", id);
            }
        }
        // Also clear the source-to-device mapping
        if let Ok(mut mapping) = self.source_to_device.lock() {
            mapping.clear();
        }
    }

    /// Check if a device is currently being captured
    pub fn is_capturing(&self, device_id: &str) -> bool {
        self.active_streams
            .lock()
            .map(|streams| streams.contains_key(device_id))
            .unwrap_or(false)
    }

    /// Get count of active captures
    pub fn active_capture_count(&self) -> usize {
        self.active_streams
            .lock()
            .map(|streams| streams.len())
            .unwrap_or(0)
    }

    /// Get list of active capture IDs
    pub fn active_capture_ids(&self) -> Vec<String> {
        self.active_streams
            .lock()
            .map(|streams| streams.keys().cloned().collect())
            .unwrap_or_default()
    }

    /// Get info about all active captures (device_id, device_name, source_type)
    pub fn active_captures_info(&self) -> Vec<(String, String, AudioSourceType)> {
        self.active_streams
            .lock()
            .map(|streams| {
                streams
                    .iter()
                    .map(|(id, stream)| (id.clone(), stream.device_name.clone(), stream.source_type))
                    .collect()
            })
            .unwrap_or_default()
    }

    // Internal helper methods

    fn device_to_info(&self, device: &Device, is_input: bool, default_id: &Option<DeviceId>) -> Option<AudioDeviceInfo> {
        let device_id = device.id().ok()?;
        let description = device.description().ok()?;
        let name = description.name().to_string();
        // Use the hardware UID as the stable identifier
        // Format: "HostId:DeviceUID" e.g. "CoreAudio:BuiltInMicrophoneDevice"
        // This can be parsed back with DeviceId::from_str()
        let id = device_id.to_string();

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

    fn find_input_device(&self, device_id_or_name: &str) -> Result<Device, String> {
        // Use cached devices for faster lookup
        let cached_devices = self.get_cached_input_devices();

        // Log available devices for debugging (only on first lookup or cache refresh)
        log::debug!("=== find_input_device('{}') ===", device_id_or_name);
        log::debug!("Available cpal input devices ({} total, cached):", cached_devices.len());
        let mut device_names: Vec<String> = Vec::new();
        let mut device_uids: Vec<String> = Vec::new();
        for (i, (_, name, uid)) in cached_devices.iter().enumerate() {
            log::debug!("  [{}] '{}' (uid: {})", i, name, uid);
            device_names.push(name.clone());
            device_uids.push(uid.clone());
        }

        // 1. Try to match by hardware UID (most reliable)
        // UID format: "HostId:DeviceUID" e.g. "CoreAudio:BuiltInMicrophoneDevice"
        if device_id_or_name.contains(':') {
            // Looks like a UID, try exact match
            for (device, name, uid) in &cached_devices {
                if uid == device_id_or_name {
                    log::info!("✓ Found device by EXACT UID match: '{}' (name: '{}')", device_id_or_name, name);
                    return Ok(device.clone());
                }
            }
            // Try partial UID match (just the device part after the colon)
            if let Some(uid_part) = device_id_or_name.split(':').nth(1) {
                for (device, name, uid) in &cached_devices {
                    if let Some(device_uid_part) = uid.split(':').nth(1) {
                        if device_uid_part == uid_part {
                            log::info!("✓ Found device by partial UID match: '{}' (name: '{}')", uid_part, name);
                            return Ok(device.clone());
                        }
                    }
                }
            }
        }

        let query_lower = device_id_or_name.to_lowercase();

        // 2. Exact name match (case-insensitive)
        for (device, name, _) in &cached_devices {
            let name_lower = name.to_lowercase();
            if name_lower == query_lower {
                log::info!("✓ Found device by EXACT name match: '{}'", name);
                return Ok(device.clone());
            }
        }

        // 3. Bidirectional substring match (query contains name OR name contains query)
        for (device, name, _) in &cached_devices {
            let name_lower = name.to_lowercase();
            if name_lower.contains(&query_lower) || query_lower.contains(&name_lower) {
                log::info!("✓ Found device by substring match: '{}' <-> '{}'", name, device_id_or_name);
                return Ok(device.clone());
            }
        }

        // 4. "Built-in" heuristic - if query mentions "built-in", find a device with "built-in" or "macbook"
        // This handles camera-linked audio sources like "FaceTime HD Camera (Built-in) (Audio)"
        if query_lower.contains("built-in") || query_lower.contains("facetime") || query_lower.contains("internal") {
            for (device, name, _) in &cached_devices {
                let name_lower = name.to_lowercase();
                // Look for MacBook microphone or any "built-in" device
                if name_lower.contains("macbook") || name_lower.contains("built-in") || name_lower.contains("internal") {
                    // Prefer input devices (microphones) over virtual devices
                    if !name_lower.contains("teams") && !name_lower.contains("virtual") && !name_lower.contains("zoom") {
                        log::info!("✓ Found built-in device by heuristic: '{}' for query '{}'", name, device_id_or_name);
                        return Ok(device.clone());
                    }
                }
            }
        }

        // 5. Index match (numeric input) - LAST RESORT as indices may differ between FFmpeg and cpal
        if let Ok(idx) = device_id_or_name.parse::<usize>() {
            if idx < cached_devices.len() {
                let (device, name, _) = &cached_devices[idx];
                let name_lower = name.to_lowercase();

                // If index points to a virtual device, prefer default input instead
                if name_lower.contains("teams") || name_lower.contains("virtual") || name_lower.contains("zoom") {
                    log::warn!("⚠ Index {} points to virtual device '{}', trying default input instead", idx, name);
                    if let Some(default_device) = self.host.default_input_device() {
                        let default_name = default_device.description().map(|d| d.name().to_string()).unwrap_or_default();
                        log::info!("✓ Using default input device: '{}'", default_name);
                        return Ok(default_device);
                    }
                }

                log::warn!("⚠ Found device by INDEX {} (less reliable): '{}'. Consider using device name instead.", idx, name);
                return Ok(device.clone());
            } else {
                log::warn!("Index {} out of range (only {} devices available)", idx, device_names.len());
            }
        }

        // 6. Final fallback: use default input device if available
        if let Some(default_device) = self.host.default_input_device() {
            let name = default_device.description().map(|d| d.name().to_string()).unwrap_or_default();
            log::warn!("⚠ Using default input device as fallback: '{}'", name);
            return Ok(default_device);
        }

        Err(format!(
            "Input device not found: '{}'. Available devices: [{}]",
            device_id_or_name,
            device_names.join(", ")
        ))
    }

    #[cfg(target_os = "windows")]
    fn find_output_device(&self, device_id_or_name: &str) -> Result<Device, String> {
        let devices = self.host
            .output_devices()
            .map_err(|e| format!("Failed to enumerate output devices: {}", e))?;

        // Try to match by name first (more reliable across FFmpeg/cpal boundary)
        let device_id_or_name_lower = device_id_or_name.to_lowercase();

        for device in devices {
            if let Ok(desc) = device.description() {
                let name = desc.name().to_lowercase();
                if name == device_id_or_name_lower || name.contains(&device_id_or_name_lower) {
                    log::info!("Found output device by name match: '{}' matches '{}'", desc.name(), device_id_or_name);
                    return Ok(device);
                }
            }
        }

        // If numeric, try matching by index
        if let Ok(idx) = device_id_or_name.parse::<usize>() {
            let devices: Vec<_> = self.host.output_devices()
                .map_err(|e| format!("Failed to enumerate output devices: {}", e))?
                .collect();
            if idx < devices.len() {
                if let Some(device) = devices.into_iter().nth(idx) {
                    let name = device.description().map(|d| d.name().to_string()).unwrap_or_default();
                    log::info!("Found output device by index {}: '{}'", idx, name);
                    return Ok(device);
                }
            }
        }

        Err(format!("Output device not found: {}", device_id_or_name))
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
