// ScreenCaptureKit Audio Capture Service (macOS only)
// Captures system audio using Apple's ScreenCaptureKit framework (macOS 13.0+)
// This enables audio metering for screen capture, window capture, and game capture sources

#![cfg(target_os = "macos")]

use screencapturekit::prelude::*;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;

use crate::services::audio_capture::AudioBuffer;

/// Active ScreenCaptureKit audio capture session
struct ActiveCapture {
    stream: SCStream,
    stop_flag: Arc<AtomicBool>,
    capture_type: String,
}

/// Service for capturing system audio via ScreenCaptureKit
pub struct SckAudioCaptureService {
    active_captures: Mutex<HashMap<String, ActiveCapture>>,
}

impl SckAudioCaptureService {
    pub fn new() -> Self {
        Self {
            active_captures: Mutex::new(HashMap::new()),
        }
    }

    /// Check if ScreenCaptureKit is available (macOS 13.0+)
    pub fn is_available() -> bool {
        true
    }

    /// Start capturing system audio for display (screen capture)
    /// `display_id` is the CGDirectDisplayID as string (new profiles) or AVFoundation index (old profiles)
    /// `device_name` is the AVFoundation device name (e.g., "Capture screen 0") for old profile migration
    pub fn start_display_audio_capture(
        &self,
        source_id: &str,
        display_id: &str,
        device_name: Option<&str>,
        audio_tx: broadcast::Sender<AudioBuffer>,
    ) -> Result<(), String> {
        self.start_capture_internal(
            source_id,
            CaptureTarget::Display {
                display_id: display_id.to_string(),
                device_name: device_name.map(|s| s.to_string()),
            },
            audio_tx,
            "ScreenCapture",
        )
    }

    /// Start capturing audio for a specific window
    pub fn start_window_audio_capture(
        &self,
        source_id: &str,
        window_id: u32,
        audio_tx: broadcast::Sender<AudioBuffer>,
    ) -> Result<(), String> {
        self.start_capture_internal(
            source_id,
            CaptureTarget::Window(window_id),
            audio_tx,
            "WindowCapture",
        )
    }

    /// Start capturing system audio (for game capture - captures all system audio)
    pub fn start_system_audio_capture(
        &self,
        source_id: &str,
        audio_tx: broadcast::Sender<AudioBuffer>,
    ) -> Result<(), String> {
        self.start_capture_internal(
            source_id,
            CaptureTarget::SystemAudio,
            audio_tx,
            "GameCapture",
        )
    }

    fn start_capture_internal(
        &self,
        source_id: &str,
        target: CaptureTarget,
        audio_tx: broadcast::Sender<AudioBuffer>,
        capture_type: &str,
    ) -> Result<(), String> {
        // Check if already capturing - return success if so (idempotent)
        {
            let captures = self.active_captures.lock().unwrap();
            if captures.contains_key(source_id) {
                log::debug!(
                    "[SCK] Already capturing {} audio for source '{}', skipping",
                    capture_type,
                    source_id
                );
                return Ok(());
            }
        }

        log::info!(
            "[SCK] Starting {} audio capture for source '{}'",
            capture_type,
            source_id
        );

        // Get shareable content (displays and windows)
        let content = SCShareableContent::get()
            .map_err(|e| format!("Failed to get shareable content: {:?}", e))?;

        // Create content filter based on target
        let filter = match target {
            CaptureTarget::Display { ref display_id, ref device_name } => {
                let displays = content.displays();
                let display = resolve_sck_display(
                    &displays,
                    display_id,
                    device_name.as_deref(),
                )?;
                SCContentFilter::create()
                    .with_display(display)
                    .with_excluding_windows(&[])
                    .build()
            }
            CaptureTarget::Window(window_id) => {
                let windows = content.windows();
                let window = windows
                    .iter()
                    .find(|w| w.window_id() == window_id)
                    .ok_or_else(|| format!("Window {} not found", window_id))?;
                SCContentFilter::create()
                    .with_window(window)
                    .build()
            }
            CaptureTarget::SystemAudio => {
                // For system audio, capture the main display but only care about audio
                let displays = content.displays();
                let display = displays
                    .first()
                    .ok_or_else(|| "No displays found".to_string())?;
                SCContentFilter::create()
                    .with_display(display)
                    .with_excluding_windows(&[])
                    .build()
            }
        };

        // Configure stream for audio capture
        // Use minimal video settings since we only need audio
        let config = SCStreamConfiguration::new()
            .with_width(2)  // Minimal video (required by API)
            .with_height(2)
            .with_captures_audio(true)
            .with_sample_rate(48000)
            .with_channel_count(2);

        // Create the stream
        let mut stream = SCStream::new(&filter, &config);

        // Create output handler
        let stop_flag = Arc::new(AtomicBool::new(false));
        let handler = AudioOutputHandler {
            source_id: source_id.to_string(),
            audio_tx,
            stop_flag: stop_flag.clone(),
            emit_count: AtomicU64::new(0),
        };

        // Add audio output handler
        stream.add_output_handler(handler, SCStreamOutputType::Audio);

        // Start the capture
        stream.start_capture()
            .map_err(|e| format!("Failed to start ScreenCaptureKit capture: {:?}", e))?;

        // Store active capture
        {
            let mut captures = self.active_captures.lock().unwrap();
            captures.insert(
                source_id.to_string(),
                ActiveCapture {
                    stream,
                    stop_flag,
                    capture_type: capture_type.to_string(),
                },
            );
        }

        log::info!(
            "[SCK] {} audio capture started for source '{}'",
            capture_type,
            source_id
        );

        Ok(())
    }

    /// Stop capturing audio for a source
    pub fn stop_capture(&self, source_id: &str) -> Result<(), String> {
        let mut captures = self.active_captures.lock().unwrap();

        if let Some(capture) = captures.remove(source_id) {
            capture.stop_flag.store(true, Ordering::Relaxed);
            if let Err(e) = capture.stream.stop_capture() {
                log::warn!("[SCK] Error stopping capture for '{}': {:?}", source_id, e);
            }
            log::info!(
                "[SCK] Stopped {} audio capture for source '{}'",
                capture.capture_type,
                source_id
            );
            Ok(())
        } else {
            Err(format!("No active capture for source: {}", source_id))
        }
    }

    /// Stop all active captures
    pub fn stop_all(&self) {
        let mut captures = self.active_captures.lock().unwrap();
        for (id, capture) in captures.drain() {
            capture.stop_flag.store(true, Ordering::Relaxed);
            let _ = capture.stream.stop_capture();
            log::info!("[SCK] Stopped audio capture for source: {}", id);
        }
    }

    /// Check if a source is being captured
    pub fn is_capturing(&self, source_id: &str) -> bool {
        let captures = self.active_captures.lock().unwrap();
        captures.contains_key(source_id)
    }

    /// Get count of active captures
    pub fn active_count(&self) -> usize {
        let captures = self.active_captures.lock().unwrap();
        captures.len()
    }

    /// Get list of active capture source IDs
    pub fn active_capture_ids(&self) -> Vec<String> {
        let captures = self.active_captures.lock().unwrap();
        captures.keys().cloned().collect()
    }
}

impl Default for SckAudioCaptureService {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for SckAudioCaptureService {
    fn drop(&mut self) {
        self.stop_all();
    }
}

/// Capture target type
enum CaptureTarget {
    Display {
        display_id: String,
        device_name: Option<String>,
    },
    Window(u32),
    SystemAudio,
}

/// Resolve the SCK display matching the given display_id (CGDirectDisplayID as string)
/// with fallback to device_name-based matching for old profiles.
fn resolve_sck_display<'a>(
    displays: &'a [screencapturekit::shareable_content::SCDisplay],
    display_id: &str,
    device_name: Option<&str>,
) -> Result<&'a screencapturekit::shareable_content::SCDisplay, String> {
    if displays.is_empty() {
        return Err("No displays available for audio capture".to_string());
    }

    // Strategy 1: Match by CGDirectDisplayID (new profiles store this as a large number)
    if let Ok(cg_id) = display_id.parse::<u32>() {
        if let Some(d) = displays.iter().find(|d| d.display_id() == cg_id) {
            log::debug!("[SCK] Matched display by CGDirectDisplayID {}", cg_id);
            return Ok(d);
        }
    }

    // Strategy 2: Old profile migration — extract screen number from device_name
    if let Some(name) = device_name {
        if let Some(screen_num) = extract_screen_number(name) {
            if screen_num < displays.len() {
                log::info!(
                    "[SCK] Migrating old profile: device_name '{}' → screen index {}",
                    name,
                    screen_num
                );
                return Ok(&displays[screen_num]);
            }
        }
    }

    // Strategy 3: Old profile with small numeric display_id — treat as array index
    if let Ok(index) = display_id.parse::<usize>() {
        if index < displays.len() {
            log::info!(
                "[SCK] Migrating old profile: display_id '{}' as array index → CGDirectDisplayID {}",
                display_id,
                displays[index].display_id()
            );
            return Ok(&displays[index]);
        }
    }

    Err(format!(
        "Display not found: display_id='{}', device_name={:?}. \
         Available displays: {:?}. Re-select the display in source properties.",
        display_id,
        device_name,
        displays.iter().map(|d| d.display_id()).collect::<Vec<_>>()
    ))
}

/// Extract screen number from a device name like "Capture screen 0" or "Screen 1"
fn extract_screen_number(device_name: &str) -> Option<usize> {
    let lower = device_name.to_lowercase();

    // Look for "screen" followed by a number
    if let Some(pos) = lower.find("screen") {
        let after_screen = &device_name[pos + 6..];
        let trimmed = after_screen.trim_start();
        let num_str: String = trimmed.chars().take_while(|c| c.is_ascii_digit()).collect();
        if !num_str.is_empty() {
            return num_str.parse().ok();
        }
    }

    // Try to find just a trailing number
    let num_str: String = device_name
        .chars()
        .rev()
        .take_while(|c| c.is_ascii_digit())
        .collect();
    if !num_str.is_empty() {
        let reversed: String = num_str.chars().rev().collect();
        return reversed.parse().ok();
    }

    None
}

/// Output handler that converts SCK planar audio to interleaved AudioBuffer
/// and sends via broadcast channel.
struct AudioOutputHandler {
    source_id: String,
    audio_tx: broadcast::Sender<AudioBuffer>,
    stop_flag: Arc<AtomicBool>,
    emit_count: AtomicU64,
}

impl SCStreamOutputTrait for AudioOutputHandler {
    fn did_output_sample_buffer(&self, sample_buffer: CMSampleBuffer, of_type: SCStreamOutputType) {
        // Only process audio samples
        if of_type != SCStreamOutputType::Audio {
            return;
        }

        if self.stop_flag.load(Ordering::Relaxed) {
            return;
        }

        // Extract audio data from CMSampleBuffer using audio_buffer_list
        if let Some(audio_data) = extract_audio_from_sample_buffer(&sample_buffer) {
            // Convert planar (L[], R[]) → interleaved [L,R,L,R,...] AudioBuffer
            let channels = if audio_data.right_samples.is_empty() { 1u16 } else { 2u16 };
            let num_frames = audio_data.left_samples.len();
            let mut interleaved = Vec::with_capacity(num_frames * channels as usize);
            for i in 0..num_frames {
                interleaved.push(audio_data.left_samples[i]);
                if channels == 2 {
                    interleaved.push(
                        audio_data.right_samples.get(i).copied().unwrap_or(0.0),
                    );
                }
            }

            let buffer = AudioBuffer {
                samples: interleaved,
                sample_rate: 48000, // SCK is configured with 48kHz
                channels,
                timestamp_ms: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64,
            };

            let count = self.emit_count.fetch_add(1, Ordering::Relaxed) + 1;
            if count <= 3 || count % 1000 == 0 {
                log::debug!(
                    "[SCK] Source '{}' buffer #{}: {} samples, {} channels",
                    self.source_id, count, buffer.samples.len(), channels
                );
            }

            let _ = self.audio_tx.send(buffer);
        }
    }
}

/// Audio data extracted from CMSampleBuffer
/// ScreenCaptureKit delivers planar (non-interleaved) audio:
/// - Buffer 0 = Left channel samples
/// - Buffer 1 = Right channel samples (if stereo)
struct AudioData {
    left_samples: Vec<f32>,
    right_samples: Vec<f32>,
}

/// Convert raw bytes to f32 samples (CoreAudio uses 32-bit float)
fn bytes_to_f32_samples(data: &[u8]) -> Vec<f32> {
    data.chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect()
}

/// Extract audio samples from a CMSampleBuffer
/// ScreenCaptureKit delivers planar (non-interleaved) audio format:
/// - num_buffers >= 2: Each buffer contains one channel (planar stereo)
/// - num_buffers == 1: Single buffer (mono, duplicated to both channels)
fn extract_audio_from_sample_buffer(buffer: &CMSampleBuffer) -> Option<AudioData> {
    // Get the audio buffer list from the sample buffer
    let audio_buffer_list = match buffer.audio_buffer_list() {
        Some(list) => list,
        None => {
            // This is expected for video frames, only log occasionally
            return None;
        }
    };

    let num_buffers = audio_buffer_list.num_buffers();
    if num_buffers == 0 {
        return None;
    }

    // ScreenCaptureKit uses planar (non-interleaved) audio format
    // Each channel is in a separate buffer
    if num_buffers >= 2 {
        // Planar stereo: buffer 0 = left, buffer 1 = right
        let left_buffer = audio_buffer_list.buffer(0)?;
        let right_buffer = audio_buffer_list.buffer(1)?;

        let left_data = left_buffer.data();
        let right_data = right_buffer.data();

        if left_data.is_empty() && right_data.is_empty() {
            return None;
        }

        let left_samples = bytes_to_f32_samples(left_data);
        let right_samples = bytes_to_f32_samples(right_data);

        if left_samples.is_empty() && right_samples.is_empty() {
            return None;
        }

        Some(AudioData {
            left_samples,
            right_samples,
        })
    } else {
        // Single buffer - mono audio, use for both channels
        let audio_buffer = audio_buffer_list.buffer(0)?;
        let data = audio_buffer.data();

        if data.is_empty() {
            return None;
        }

        let samples = bytes_to_f32_samples(data);

        if samples.is_empty() {
            return None;
        }

        // Mono: same data for both channels
        Some(AudioData {
            left_samples: samples.clone(),
            right_samples: samples,
        })
    }
}

// Audio level calculation is now handled by compute_stereo_levels() in audio_levels.rs
// via the unified AudioBuffer → register_audio_source() pipeline.
