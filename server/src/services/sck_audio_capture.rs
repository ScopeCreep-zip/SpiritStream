// ScreenCaptureKit Audio Capture Service (macOS only)
// Captures system audio using Apple's ScreenCaptureKit framework (macOS 13.0+)
// This enables audio metering for screen capture, window capture, and game capture sources

#![cfg(target_os = "macos")]

use screencapturekit::prelude::*;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

use crate::services::audio_level_extractor::ExtractedAudioLevel;

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
    pub fn start_display_audio_capture(
        &self,
        source_id: &str,
        display_index: u32,
        level_tx: mpsc::UnboundedSender<ExtractedAudioLevel>,
    ) -> Result<(), String> {
        self.start_capture_internal(
            source_id,
            CaptureTarget::Display(display_index),
            level_tx,
            "ScreenCapture",
        )
    }

    /// Start capturing audio for a specific window
    pub fn start_window_audio_capture(
        &self,
        source_id: &str,
        window_id: u32,
        level_tx: mpsc::UnboundedSender<ExtractedAudioLevel>,
    ) -> Result<(), String> {
        self.start_capture_internal(
            source_id,
            CaptureTarget::Window(window_id),
            level_tx,
            "WindowCapture",
        )
    }

    /// Start capturing system audio (for game capture - captures all system audio)
    pub fn start_system_audio_capture(
        &self,
        source_id: &str,
        level_tx: mpsc::UnboundedSender<ExtractedAudioLevel>,
    ) -> Result<(), String> {
        self.start_capture_internal(
            source_id,
            CaptureTarget::SystemAudio,
            level_tx,
            "GameCapture",
        )
    }

    fn start_capture_internal(
        &self,
        source_id: &str,
        target: CaptureTarget,
        level_tx: mpsc::UnboundedSender<ExtractedAudioLevel>,
        capture_type: &str,
    ) -> Result<(), String> {
        // Check if already capturing
        {
            let captures = self.active_captures.lock().unwrap();
            if captures.contains_key(source_id) {
                return Err(format!("Already capturing audio for source: {}", source_id));
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
            CaptureTarget::Display(index) => {
                let displays = content.displays();
                // Try the index first (0-based), then try index-1 (if it looks like 1-based display number)
                let display = displays
                    .get(index as usize)
                    .or_else(|| {
                        // If index > 0 and direct index failed, try as 1-based index
                        if index > 0 {
                            displays.get((index - 1) as usize)
                        } else {
                            None
                        }
                    })
                    .ok_or_else(|| format!("Display {} not found (available: {}, tried indices {} and {})",
                        index, displays.len(), index, index.saturating_sub(1)))?;
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
            level_tx,
            stop_flag: stop_flag.clone(),
            last_emit: Mutex::new(Instant::now()),
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
    Display(u32),
    Window(u32),
    SystemAudio,
}

/// Output handler that processes audio samples and calculates levels
struct AudioOutputHandler {
    source_id: String,
    level_tx: mpsc::UnboundedSender<ExtractedAudioLevel>,
    stop_flag: Arc<AtomicBool>,
    last_emit: Mutex<Instant>,
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
            // Rate limit to ~30Hz (every 33ms)
            let should_emit = {
                let mut last_emit = self.last_emit.lock().unwrap();
                if last_emit.elapsed() >= Duration::from_millis(33) {
                    *last_emit = Instant::now();
                    true
                } else {
                    false
                }
            };

            if should_emit {
                let level = calculate_audio_levels(&self.source_id, &audio_data);

                // Log first few emissions
                let count = self.emit_count.fetch_add(1, Ordering::Relaxed) + 1;
                if count <= 5 || count % 300 == 0 {
                    log::debug!(
                        "[SCK] Source '{}' emit #{}: RMS L={:.4} R={:.4}, Peak L={:.4} R={:.4}",
                        self.source_id,
                        count,
                        level.left_rms.unwrap_or(0.0),
                        level.right_rms.unwrap_or(0.0),
                        level.left_peak.unwrap_or(0.0),
                        level.right_peak.unwrap_or(0.0),
                    );
                }

                let _ = self.level_tx.send(level);
            }
        }
    }
}

/// Audio data extracted from CMSampleBuffer
struct AudioData {
    samples: Vec<f32>,
    channels: u16,
}

/// Extract audio samples from a CMSampleBuffer
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

    // Get the first buffer (usually contains interleaved stereo data)
    let audio_buffer = audio_buffer_list.buffer(0)?;

    // Get raw data from the buffer
    let data = audio_buffer.data();

    if data.is_empty() {
        return None;
    }

    // CoreAudio typically uses 32-bit float audio
    // Convert bytes to f32 samples
    let samples: Vec<f32> = data
        .chunks_exact(4)
        .filter_map(|chunk| {
            if chunk.len() == 4 {
                Some(f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
            } else {
                None
            }
        })
        .collect();

    if samples.is_empty() {
        return None;
    }

    // Default to stereo (2 channels) as configured in the stream
    // AudioBufferRef doesn't expose channel count directly, so we use the configured value
    let channels = 2u16;

    Some(AudioData { samples, channels })
}

/// Calculate audio levels (RMS and peak) from audio samples
fn calculate_audio_levels(source_id: &str, audio: &AudioData) -> ExtractedAudioLevel {
    let channels = audio.channels as usize;

    if channels == 0 || audio.samples.is_empty() {
        return ExtractedAudioLevel {
            source_id: source_id.to_string(),
            rms: 0.0,
            peak: 0.0,
            left_rms: Some(0.0),
            right_rms: Some(0.0),
            left_peak: Some(0.0),
            right_peak: Some(0.0),
        };
    }

    // Deinterleave channels and calculate per-channel levels
    let mut left_sum_sq = 0.0f64;
    let mut left_peak = 0.0f32;
    let mut right_sum_sq = 0.0f64;
    let mut right_peak = 0.0f32;
    let mut left_count = 0usize;
    let mut right_count = 0usize;

    for (i, &sample) in audio.samples.iter().enumerate() {
        let abs_sample = sample.abs();

        if channels >= 2 {
            // Stereo: even indices are left, odd are right
            if i % 2 == 0 {
                left_sum_sq += (sample as f64).powi(2);
                left_peak = left_peak.max(abs_sample);
                left_count += 1;
            } else {
                right_sum_sq += (sample as f64).powi(2);
                right_peak = right_peak.max(abs_sample);
                right_count += 1;
            }
        } else {
            // Mono: use same data for both channels
            left_sum_sq += (sample as f64).powi(2);
            left_peak = left_peak.max(abs_sample);
            left_count += 1;
            right_sum_sq = left_sum_sq;
            right_peak = left_peak;
            right_count = left_count;
        }
    }

    // Calculate RMS for each channel
    let left_rms = if left_count > 0 {
        ((left_sum_sq / left_count as f64).sqrt() as f32).min(1.0)
    } else {
        0.0
    };

    let right_rms = if right_count > 0 {
        ((right_sum_sq / right_count as f64).sqrt() as f32).min(1.0)
    } else {
        0.0
    };

    // Clamp peaks
    let left_peak = left_peak.min(1.0);
    let right_peak = right_peak.min(1.0);

    // Overall levels (average of channels)
    let rms = (left_rms + right_rms) / 2.0;
    let peak = left_peak.max(right_peak);

    ExtractedAudioLevel {
        source_id: source_id.to_string(),
        rms,
        peak,
        left_rms: Some(left_rms),
        right_rms: Some(right_rms),
        left_peak: Some(left_peak),
        right_peak: Some(right_peak),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_audio_levels_silence() {
        let audio = AudioData {
            samples: vec![0.0; 1024],
            channels: 2,
        };
        let levels = calculate_audio_levels("test", &audio);
        assert!(levels.rms < 0.001);
        assert!(levels.peak < 0.001);
    }

    #[test]
    fn test_calculate_audio_levels_stereo() {
        // Create test signal: left channel louder than right
        let mut samples = Vec::new();
        for _ in 0..512 {
            samples.push(0.5);  // Left channel
            samples.push(0.25); // Right channel
        }
        let audio = AudioData {
            samples,
            channels: 2,
        };
        let levels = calculate_audio_levels("test", &audio);

        assert!(levels.left_rms.unwrap() > levels.right_rms.unwrap());
        assert!((levels.left_peak.unwrap() - 0.5).abs() < 0.01);
        assert!((levels.right_peak.unwrap() - 0.25).abs() < 0.01);
    }
}
