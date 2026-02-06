// Audio Level Extractor Service
// Extracts audio levels from various source types using FFmpeg
// Supports: MediaFile, ScreenCapture, RTMP, CaptureCard, etc.

use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tokio::sync::mpsc;

// Windows: Hide console windows for spawned processes
#[cfg(windows)]
use std::os::windows::process::CommandExt;
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

/// Audio level data extracted from a source
/// Follows OBS's audio metering model:
/// - Peak = instantaneous maximum absolute sample value
/// - RMS = root mean square (average power)
/// Both are tracked independently per channel for stereo sources
#[derive(Debug, Clone)]
pub struct ExtractedAudioLevel {
    pub source_id: String,
    /// Overall RMS level (0.0 - 1.0)
    pub rms: f32,
    /// Overall peak level (0.0 - 1.0)
    pub peak: f32,
    /// Left channel RMS for stereo
    pub left_rms: Option<f32>,
    /// Right channel RMS for stereo
    pub right_rms: Option<f32>,
    /// Left channel peak for stereo (instantaneous max)
    pub left_peak: Option<f32>,
    /// Right channel peak for stereo (instantaneous max)
    pub right_peak: Option<f32>,
}

/// Configuration for audio extraction
#[derive(Debug, Clone)]
pub struct AudioExtractionConfig {
    /// Source type (for logging)
    pub source_type: String,
    /// Input URL or path (e.g., file path, rtmp:// URL, avfoundation device)
    pub input: String,
    /// Optional input format (e.g., "avfoundation" for screen capture)
    pub input_format: Option<String>,
    /// Optional input options (e.g., capture specific display)
    pub input_options: Vec<(String, String)>,
    /// Whether to only capture audio (no video)
    pub audio_only: bool,
}

/// Active audio extraction process
struct ActiveExtraction {
    /// FFmpeg child process
    child: Child,
    /// Stop flag for graceful shutdown
    stop_flag: Arc<AtomicBool>,
    /// Source type for logging
    #[allow(dead_code)]
    source_type: String,
    /// Last update time (for potential health checks)
    #[allow(dead_code)]
    last_update: Instant,
}

/// Service for extracting audio levels from various sources using FFmpeg
pub struct AudioLevelExtractor {
    ffmpeg_path: String,
    active_extractions: Mutex<HashMap<String, ActiveExtraction>>,
}

impl AudioLevelExtractor {
    /// Create a new AudioLevelExtractor
    pub fn new(ffmpeg_path: String) -> Self {
        Self {
            ffmpeg_path,
            active_extractions: Mutex::new(HashMap::new()),
        }
    }

    /// Start extracting audio levels from a media file
    pub fn start_media_file_extraction(
        &self,
        source_id: &str,
        file_path: &str,
        level_tx: mpsc::UnboundedSender<ExtractedAudioLevel>,
    ) -> Result<(), String> {
        // Check if file is a supported media format (not HTML, text, etc.)
        let path = std::path::Path::new(file_path);
        let extension = path.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .unwrap_or_default();

        // Media file extensions that can have audio
        const MEDIA_EXTENSIONS: &[&str] = &[
            // Video
            "mp4", "mkv", "avi", "mov", "wmv", "flv", "webm", "m4v", "mpeg", "mpg", "ts", "m2ts",
            // Audio
            "mp3", "wav", "aac", "flac", "ogg", "m4a", "wma", "opus", "aiff",
        ];

        if !MEDIA_EXTENSIONS.contains(&extension.as_str()) {
            return Err(format!(
                "File '{}' is not a supported media format (found: .{}). Audio metering only works with video/audio files.",
                path.file_name().unwrap_or_default().to_string_lossy(),
                if extension.is_empty() { "none" } else { &extension }
            ));
        }

        let config = AudioExtractionConfig {
            source_type: "MediaFile".to_string(),
            input: file_path.to_string(),
            input_format: None,
            input_options: vec![],
            audio_only: true, // Only need audio for level extraction
        };
        self.start_extraction(source_id, config, level_tx)
    }

    /// Start extracting audio levels from an RTMP stream
    pub fn start_rtmp_extraction(
        &self,
        source_id: &str,
        rtmp_url: &str,
        level_tx: mpsc::UnboundedSender<ExtractedAudioLevel>,
    ) -> Result<(), String> {
        let config = AudioExtractionConfig {
            source_type: "Rtmp".to_string(),
            input: rtmp_url.to_string(),
            input_format: Some("flv".to_string()),
            input_options: vec![
                ("rw_timeout".to_string(), "5000000".to_string()), // 5s timeout
            ],
            audio_only: true,
        };
        self.start_extraction(source_id, config, level_tx)
    }

    /// Start extracting audio levels from screen capture (macOS)
    /// Note: macOS screen audio capture requires ScreenCaptureKit (macOS 12.3+) integration
    /// or a loopback audio driver like BlackHole/Soundflower. FFmpeg avfoundation cannot
    /// capture system audio directly.
    #[cfg(target_os = "macos")]
    pub fn start_screen_capture_extraction(
        &self,
        _source_id: &str,
        _display_index: u32,
        _level_tx: mpsc::UnboundedSender<ExtractedAudioLevel>,
    ) -> Result<(), String> {
        // macOS screen audio capture is not available via FFmpeg avfoundation.
        // It requires either:
        // 1. ScreenCaptureKit integration (macOS 12.3+) - not yet implemented
        // 2. A virtual audio device like BlackHole or Soundflower
        // For now, return an error explaining this limitation.
        Err("Screen audio capture on macOS requires ScreenCaptureKit integration (coming soon) or a loopback audio driver like BlackHole".to_string())
    }

    /// Start extracting audio levels from screen capture (Windows)
    /// Uses WASAPI loopback to capture desktop audio
    #[cfg(target_os = "windows")]
    pub fn start_screen_capture_extraction(
        &self,
        source_id: &str,
        _display_index: u32,
        level_tx: mpsc::UnboundedSender<ExtractedAudioLevel>,
    ) -> Result<(), String> {
        // Windows: Use dshow with WASAPI loopback
        // Note: This captures all desktop audio, not just from the specific display
        let config = AudioExtractionConfig {
            source_type: "ScreenCapture".to_string(),
            input: "audio=virtual-audio-capturer".to_string(),
            input_format: Some("dshow".to_string()),
            input_options: vec![],
            audio_only: true,
        };
        self.start_extraction(source_id, config, level_tx)
    }

    /// Start extracting audio levels from screen capture (Linux)
    #[cfg(target_os = "linux")]
    pub fn start_screen_capture_extraction(
        &self,
        source_id: &str,
        display_index: u32,
        level_tx: mpsc::UnboundedSender<ExtractedAudioLevel>,
    ) -> Result<(), String> {
        // Linux: Use PulseAudio monitor source
        let config = AudioExtractionConfig {
            source_type: "ScreenCapture".to_string(),
            input: "default".to_string(),
            input_format: Some("pulse".to_string()),
            input_options: vec![],
            audio_only: true,
        };
        self.start_extraction(source_id, config, level_tx)
    }

    /// Start extracting audio levels from a capture card
    pub fn start_capture_card_extraction(
        &self,
        source_id: &str,
        device_id: &str,
        level_tx: mpsc::UnboundedSender<ExtractedAudioLevel>,
    ) -> Result<(), String> {
        #[cfg(target_os = "macos")]
        let (input, format) = (device_id.to_string(), Some("avfoundation".to_string()));

        #[cfg(target_os = "windows")]
        let (input, format) = (format!("audio={}", device_id), Some("dshow".to_string()));

        #[cfg(target_os = "linux")]
        let (input, format) = (device_id.to_string(), Some("alsa".to_string()));

        let config = AudioExtractionConfig {
            source_type: "CaptureCard".to_string(),
            input,
            input_format: format,
            input_options: vec![],
            audio_only: true,
        };
        self.start_extraction(source_id, config, level_tx)
    }

    /// Start extracting audio levels from window capture
    #[cfg(target_os = "macos")]
    pub fn start_window_capture_extraction(
        &self,
        source_id: &str,
        window_id: &str,
        level_tx: mpsc::UnboundedSender<ExtractedAudioLevel>,
    ) -> Result<(), String> {
        // macOS: Window audio capture via ScreenCaptureKit
        let config = AudioExtractionConfig {
            source_type: "WindowCapture".to_string(),
            input: format!("{}:none", window_id),
            input_format: Some("avfoundation".to_string()),
            input_options: vec![],
            audio_only: true,
        };
        self.start_extraction(source_id, config, level_tx)
    }

    #[cfg(target_os = "windows")]
    pub fn start_window_capture_extraction(
        &self,
        source_id: &str,
        _window_id: &str,
        level_tx: mpsc::UnboundedSender<ExtractedAudioLevel>,
    ) -> Result<(), String> {
        // Windows: Use WASAPI for app-specific audio (requires special handling)
        let config = AudioExtractionConfig {
            source_type: "WindowCapture".to_string(),
            input: "default".to_string(),
            input_format: Some("dshow".to_string()),
            input_options: vec![],
            audio_only: true,
        };
        self.start_extraction(source_id, config, level_tx)
    }

    #[cfg(target_os = "linux")]
    pub fn start_window_capture_extraction(
        &self,
        source_id: &str,
        _window_id: &str,
        level_tx: mpsc::UnboundedSender<ExtractedAudioLevel>,
    ) -> Result<(), String> {
        // Linux: Use PulseAudio for app-specific audio
        let config = AudioExtractionConfig {
            source_type: "WindowCapture".to_string(),
            input: "default".to_string(),
            input_format: Some("pulse".to_string()),
            input_options: vec![],
            audio_only: true,
        };
        self.start_extraction(source_id, config, level_tx)
    }

    /// Start extracting audio levels from game capture
    pub fn start_game_capture_extraction(
        &self,
        source_id: &str,
        level_tx: mpsc::UnboundedSender<ExtractedAudioLevel>,
    ) -> Result<(), String> {
        // Game capture audio typically comes from system audio loopback
        #[cfg(target_os = "windows")]
        let (input, format) = ("default".to_string(), Some("dshow".to_string()));

        #[cfg(target_os = "macos")]
        let (input, format) = ("none:0".to_string(), Some("avfoundation".to_string()));

        #[cfg(target_os = "linux")]
        let (input, format) = ("default".to_string(), Some("pulse".to_string()));

        let config = AudioExtractionConfig {
            source_type: "GameCapture".to_string(),
            input,
            input_format: format,
            input_options: vec![],
            audio_only: true,
        };
        self.start_extraction(source_id, config, level_tx)
    }

    /// Start extracting audio levels from a media playlist (current item)
    pub fn start_media_playlist_extraction(
        &self,
        source_id: &str,
        current_file_path: &str,
        level_tx: mpsc::UnboundedSender<ExtractedAudioLevel>,
    ) -> Result<(), String> {
        // Extract audio from the current playlist item
        let config = AudioExtractionConfig {
            source_type: "MediaPlaylist".to_string(),
            input: current_file_path.to_string(),
            input_format: None,
            input_options: vec![],
            audio_only: true,
        };
        self.start_extraction(source_id, config, level_tx)
    }

    /// Start extracting audio levels from an NDI source
    pub fn start_ndi_extraction(
        &self,
        source_id: &str,
        ndi_source_name: &str,
        ip_address: Option<&str>,
        level_tx: mpsc::UnboundedSender<ExtractedAudioLevel>,
    ) -> Result<(), String> {
        // NDI source URL format: ndi://source_name or ndi://ip:port
        let input = if let Some(ip) = ip_address {
            format!("{}@{}", ndi_source_name, ip)
        } else {
            ndi_source_name.to_string()
        };

        let config = AudioExtractionConfig {
            source_type: "Ndi".to_string(),
            input,
            input_format: Some("libndi_newtek".to_string()), // Requires FFmpeg with NDI support
            input_options: vec![],
            audio_only: true,
        };
        self.start_extraction(source_id, config, level_tx)
    }

    /// Generic extraction start using FFmpeg's ebur128 filter for accurate level metering
    fn start_extraction(
        &self,
        source_id: &str,
        config: AudioExtractionConfig,
        level_tx: mpsc::UnboundedSender<ExtractedAudioLevel>,
    ) -> Result<(), String> {
        // Check if already extracting
        {
            let extractions = self.active_extractions.lock().unwrap();
            if extractions.contains_key(source_id) {
                return Err(format!("Already extracting audio for source: {}", source_id));
            }
        }

        log::info!(
            "Starting audio level extraction for {} source '{}' from: {}",
            config.source_type, source_id, config.input
        );

        // Build FFmpeg command for audio level extraction
        // Uses ebur128 filter which provides momentary loudness (good for VU meters)
        let mut cmd = Command::new(&self.ffmpeg_path);

        // Hide window on Windows
        #[cfg(windows)]
        cmd.creation_flags(CREATE_NO_WINDOW);

        // Input options
        if let Some(ref format) = config.input_format {
            cmd.arg("-f").arg(format);
        }
        for (key, value) in &config.input_options {
            cmd.arg(format!("-{}", key)).arg(value);
        }

        // Input
        cmd.arg("-i").arg(&config.input);

        // Only audio (no video processing for speed)
        cmd.arg("-vn");

        // Audio filter: astats for real-time level metering
        // astats outputs RMS and peak levels at regular intervals
        // Note: ametadata's key= option only accepts ONE key, so we print all metadata
        // and filter for the keys we need in the parser
        cmd.args([
            "-af", "astats=metadata=1:reset=1,ametadata=print",
            "-f", "null",
            "-"
        ]);

        // Capture stderr for level data
        cmd.stdout(Stdio::null());
        cmd.stderr(Stdio::piped());

        let mut child = cmd.spawn()
            .map_err(|e| format!("Failed to spawn FFmpeg for audio extraction: {}", e))?;

        let stderr = child.stderr.take()
            .ok_or("Failed to capture FFmpeg stderr")?;

        let stop_flag = Arc::new(AtomicBool::new(false));
        let stop_flag_clone = stop_flag.clone();
        let source_id_clone = source_id.to_string();
        let source_type = config.source_type.clone();

        // Spawn thread to parse FFmpeg output with audio capture priority
        super::thread_config::CaptureThreadKind::AudioCapture
            .spawn(&format!("ss-audio-ext-{}", source_id), move || {
                parse_ffmpeg_audio_levels(
                    stderr,
                    stop_flag_clone,
                    source_id_clone,
                    level_tx,
                );
            });

        // Store active extraction
        {
            let mut extractions = self.active_extractions.lock().unwrap();
            extractions.insert(
                source_id.to_string(),
                ActiveExtraction {
                    child,
                    stop_flag,
                    source_type,
                    last_update: Instant::now(),
                },
            );
        }

        log::info!("Audio level extraction started for source: {}", source_id);
        Ok(())
    }

    /// Stop extracting audio levels for a source
    pub fn stop_extraction(&self, source_id: &str) -> Result<(), String> {
        let mut extractions = self.active_extractions.lock().unwrap();

        if let Some(mut extraction) = extractions.remove(source_id) {
            extraction.stop_flag.store(true, Ordering::Relaxed);
            let _ = extraction.child.kill();
            log::info!("Stopped audio extraction for source: {}", source_id);
            Ok(())
        } else {
            Err(format!("No active extraction for source: {}", source_id))
        }
    }

    /// Stop all active extractions
    pub fn stop_all(&self) {
        let mut extractions = self.active_extractions.lock().unwrap();
        for (id, mut extraction) in extractions.drain() {
            extraction.stop_flag.store(true, Ordering::Relaxed);
            let _ = extraction.child.kill();
            log::info!("Stopped audio extraction for source: {}", id);
        }
    }

    /// Check if extraction is active for a source
    pub fn is_extracting(&self, source_id: &str) -> bool {
        let extractions = self.active_extractions.lock().unwrap();
        extractions.contains_key(source_id)
    }

    /// Get list of all active extraction source IDs
    pub fn active_extraction_ids(&self) -> Vec<String> {
        let extractions = self.active_extractions.lock().unwrap();
        extractions.keys().cloned().collect()
    }

    /// Get count of active extractions
    pub fn active_count(&self) -> usize {
        let extractions = self.active_extractions.lock().unwrap();
        extractions.len()
    }
}

impl Drop for AudioLevelExtractor {
    fn drop(&mut self) {
        self.stop_all();
    }
}

/// Parse FFmpeg astats output to extract audio levels
fn parse_ffmpeg_audio_levels(
    stderr: std::process::ChildStderr,
    stop_flag: Arc<AtomicBool>,
    source_id: String,
    level_tx: mpsc::UnboundedSender<ExtractedAudioLevel>,
) {
    let reader = BufReader::new(stderr);

    let mut current_rms: Option<f32> = None;
    let mut current_peak: Option<f32> = None;
    let mut current_left_rms: Option<f32> = None;
    let mut current_right_rms: Option<f32> = None;
    let mut current_left_peak: Option<f32> = None;
    let mut current_right_peak: Option<f32> = None;
    let mut last_emit = Instant::now();
    let mut emit_count: u64 = 0;

    for line in reader.lines() {
        if stop_flag.load(Ordering::Relaxed) {
            break;
        }

        let line = match line {
            Ok(l) => l,
            Err(_) => continue,
        };

        // Parse astats metadata output
        // Format: lavfi.astats.Overall.RMS_level=-20.5
        // Channel-specific format: lavfi.astats.1.RMS_level=-20.5 (channel 1 = left)
        //                          lavfi.astats.2.RMS_level=-20.5 (channel 2 = right)
        if line.contains("lavfi.astats") {
            if let Some(value) = extract_astats_value(&line, "Overall.RMS_level") {
                current_rms = Some(db_to_linear(value));
            }
            if let Some(value) = extract_astats_value(&line, "Overall.Peak_level") {
                current_peak = Some(db_to_linear(value));
            }
            // Channel 1 = Left channel for stereo audio
            if let Some(value) = extract_astats_value(&line, "1.RMS_level") {
                current_left_rms = Some(db_to_linear(value));
            }
            if let Some(value) = extract_astats_value(&line, "1.Peak_level") {
                current_left_peak = Some(db_to_linear(value));
            }
            // Channel 2 = Right channel for stereo audio
            if let Some(value) = extract_astats_value(&line, "2.RMS_level") {
                current_right_rms = Some(db_to_linear(value));
            }
            if let Some(value) = extract_astats_value(&line, "2.Peak_level") {
                current_right_peak = Some(db_to_linear(value));
            }

            // Emit level update at ~30Hz (every 33ms)
            if last_emit.elapsed() >= Duration::from_millis(33) {
                if let Some(rms) = current_rms {
                    let peak = current_peak.unwrap_or(rms);

                    // For stereo: use channel-specific data if available
                    // For mono: both channels will be None, fall back to overall
                    let left_rms = current_left_rms.or(Some(rms));
                    let right_rms = current_right_rms.or(current_left_rms).or(Some(rms));
                    let left_peak = current_left_peak.or(Some(peak));
                    let right_peak = current_right_peak.or(current_left_peak).or(Some(peak));

                    let level = ExtractedAudioLevel {
                        source_id: source_id.clone(),
                        rms,
                        peak,
                        left_rms,
                        right_rms,
                        left_peak,
                        right_peak,
                    };

                    // Log first few emissions and periodically to confirm stereo data
                    emit_count += 1;
                    if emit_count <= 5 || emit_count % 300 == 0 {
                        log::debug!(
                            "[AudioExtractor] Source '{}' emit #{}: RMS L={:.4} R={:.4}, Peak L={:.4} R={:.4} (stereo={})",
                            source_id,
                            emit_count,
                            left_rms.unwrap_or(0.0),
                            right_rms.unwrap_or(0.0),
                            left_peak.unwrap_or(0.0),
                            right_peak.unwrap_or(0.0),
                            current_left_rms.is_some() && current_right_rms.is_some()
                        );
                    }

                    if level_tx.send(level).is_err() {
                        // Channel closed, stop extraction
                        break;
                    }

                    last_emit = Instant::now();
                    // Reset for next reading
                    current_rms = None;
                    current_peak = None;
                    current_left_rms = None;
                    current_right_rms = None;
                    current_left_peak = None;
                    current_right_peak = None;
                }
            }
        }
    }

    log::debug!("Audio level parsing ended for source: {} (emitted {} levels)", source_id, emit_count);
}

/// Extract a value from astats metadata line
fn extract_astats_value(line: &str, key: &str) -> Option<f32> {
    let pattern = format!("lavfi.astats.{}", key);
    if line.contains(&pattern) {
        // Line format: "lavfi.astats.Overall.RMS_level=-20.5" or similar
        if let Some(pos) = line.find('=') {
            let value_str = line[pos + 1..].trim();
            // Handle -inf values (silence)
            if value_str == "-inf" || value_str.starts_with("-inf") {
                return Some(-96.0);
            }
            return value_str.parse().ok();
        }
    }
    None
}

/// Convert dB to linear scale (0.0 - 1.0)
fn db_to_linear(db: f32) -> f32 {
    if db <= -96.0 {
        0.0
    } else if db >= 0.0 {
        1.0
    } else {
        10.0_f32.powf(db / 20.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_db_to_linear() {
        assert!((db_to_linear(0.0) - 1.0).abs() < 0.001);
        assert!((db_to_linear(-6.0) - 0.5012).abs() < 0.01);
        assert!((db_to_linear(-20.0) - 0.1).abs() < 0.01);
        assert!((db_to_linear(-96.0) - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_extract_astats_value() {
        let line = "lavfi.astats.Overall.RMS_level=-20.5";
        assert_eq!(extract_astats_value(line, "Overall.RMS_level"), Some(-20.5));

        let line2 = "lavfi.astats.1.RMS_level=-inf";
        assert_eq!(extract_astats_value(line2, "1.RMS_level"), Some(-96.0));
    }
}
