// H264 Capture Service
// Captures screen frames via scap and encodes to H264 using FFmpeg
// Supports two output modes:
// 1. HTTP/MPEG-TS streaming (for go2rtc with #video=copy passthrough)
// 2. RTSP push to go2rtc (alternative low-latency mode)

use std::collections::HashMap;
use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use bytes::Bytes;
use scap::capturer::Resolution;
use scap::frame::Frame;
use tokio::sync::broadcast;

use super::screen_capture::{ScreenCaptureConfig, ScreenCaptureService};
use crate::models::ScreenCaptureSource;

const DEFAULT_CHANNEL_CAPACITY: usize = 64;
const ORPHAN_TIMEOUT_SECS: u64 = 60;

/// Active H264 capture session
struct H264CaptureSession {
    /// Stop flag for graceful shutdown
    stop_flag: Arc<AtomicBool>,
    /// Output broadcast sender (for HTTP mode)
    output_tx: Option<broadcast::Sender<Bytes>>,
    /// Last time this session was accessed
    last_accessed: Arc<Mutex<Instant>>,
    /// Screen capture thread handle
    _capture_handle: std::thread::JoinHandle<()>,
    /// Width of captured frames
    width: u32,
    /// Height of captured frames
    height: u32,
    /// Display ID for stopping the underlying screen capture
    display_id: u32,
}

/// Configuration for H264 encoding
#[derive(Debug, Clone)]
pub struct H264EncodingConfig {
    /// Video bitrate in kbps (default: 4000)
    pub bitrate_kbps: u32,
    /// Keyframe interval in frames (default: 30 = 1 second at 30fps)
    pub keyframe_interval: u32,
    /// Encoding preset (ultrafast, superfast, veryfast, faster, fast, medium)
    pub preset: String,
    /// Use hardware encoding if available (VideoToolbox on macOS)
    pub use_hw_accel: bool,
}

impl Default for H264EncodingConfig {
    fn default() -> Self {
        Self {
            bitrate_kbps: 4000,
            keyframe_interval: 15, // Reduced from 30 for faster initial frame
            preset: "ultrafast".to_string(),
            use_hw_accel: true,
        }
    }
}

/// Service for capturing screen to H264 via RTSP push to go2rtc
pub struct H264CaptureService {
    sessions: Mutex<HashMap<String, H264CaptureSession>>,
    screen_capture: Arc<ScreenCaptureService>,
    ffmpeg_path: String,
}

impl H264CaptureService {
    /// Create a new H264CaptureService
    pub fn new(screen_capture: Arc<ScreenCaptureService>, ffmpeg_path: String) -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
            screen_capture,
            ffmpeg_path,
        }
    }

    /// Start capturing a screen source and encoding to H264 via RTSP push.
    /// FFmpeg pushes directly to go2rtc RTSP server for low-latency passthrough.
    pub fn start_capture_rtsp(
        &self,
        source: &ScreenCaptureSource,
        rtsp_url: String,
        encoding_config: Option<H264EncodingConfig>,
    ) -> Result<(), String> {
        let source_id = source.id.clone();
        let encoding = encoding_config.unwrap_or_default();

        // Check if already capturing
        {
            let sessions = self.sessions.lock().unwrap();
            if sessions.contains_key(&source_id) {
                log::debug!("H264 capture already running for {}", source_id);
                return Ok(());
            }
        }

        let start_time = Instant::now();
        log::info!("Starting H264 capture for source: {} (RTSP: {})", source_id, rtsp_url);

        // Find the correct scap display ID
        let scap_display_id = self.find_scap_display_id(source)?;

        log::debug!(
            "[{:?}] Resolved display_id '{}' (device_name: {:?}) to scap display ID {}",
            start_time.elapsed(),
            source.display_id,
            source.device_name,
            scap_display_id
        );

        // Configure screen capture
        let capture_config = ScreenCaptureConfig {
            fps: source.fps,
            show_cursor: source.capture_cursor,
            show_highlight: false,
            output_resolution: Resolution::Captured,
        };

        // Start native screen capture
        let frame_rx = self.screen_capture.start_display_capture(scap_display_id, capture_config)?;
        log::debug!("[{:?}] Screen capture started", start_time.elapsed());

        // Get frame dimensions from display list (not blocking on frames)
        let (width, height) = self.get_frame_dimensions(&source_id)?;
        log::debug!("[{:?}] Got frame dimensions: {}x{}", start_time.elapsed(), width, height);

        // Create session state
        let stop_flag = Arc::new(AtomicBool::new(false));
        let last_accessed = Arc::new(Mutex::new(Instant::now()));

        // Clone values for the capture thread
        let stop_flag_clone = stop_flag.clone();
        let last_accessed_clone = last_accessed.clone();
        let ffmpeg_path = self.ffmpeg_path.clone();
        let fps = source.fps;
        let source_id_clone = source_id.clone();
        let capture_audio = source.capture_audio;

        // Spawn the capture + encoding thread
        let capture_handle = std::thread::spawn(move || {
            run_capture_encoding_loop(
                frame_rx,
                stop_flag_clone,
                last_accessed_clone,
                ffmpeg_path,
                width,
                height,
                fps,
                encoding,
                source_id_clone,
                capture_audio,
                rtsp_url,
            );
        });

        // Store the session
        {
            let mut sessions = self.sessions.lock().unwrap();
            sessions.insert(
                source_id.clone(),
                H264CaptureSession {
                    stop_flag,
                    output_tx: None, // RTSP mode doesn't use broadcast
                    last_accessed,
                    _capture_handle: capture_handle,
                    width,
                    height,
                    display_id: scap_display_id,
                },
            );
        }

        log::info!(
            "[{:?}] H264 capture started for source {} ({}x{} @ {}fps, RTSP passthrough)",
            start_time.elapsed(),
            source_id, width, height, fps
        );

        Ok(())
    }

    /// Start capturing a screen source and encoding to H264 MPEG-TS via HTTP.
    /// Returns a broadcast receiver for the MPEG-TS stream.
    /// Use this with go2rtc's #video=copy flag for passthrough.
    pub fn start_capture_http(
        &self,
        source: &ScreenCaptureSource,
        encoding_config: Option<H264EncodingConfig>,
    ) -> Result<broadcast::Receiver<Bytes>, String> {
        let source_id = source.id.clone();
        let encoding = encoding_config.unwrap_or_default();

        // Check if already capturing
        {
            let sessions = self.sessions.lock().unwrap();
            if let Some(session) = sessions.get(&source_id) {
                if let Some(ref tx) = session.output_tx {
                    *session.last_accessed.lock().unwrap() = Instant::now();
                    return Ok(tx.subscribe());
                }
            }
        }

        let start_time = Instant::now();
        log::info!("Starting H264 capture for source: {} (HTTP mode)", source_id);

        // Find the correct scap display ID
        let scap_display_id = self.find_scap_display_id(source)?;

        log::debug!(
            "[{:?}] Resolved display_id '{}' (device_name: {:?}) to scap display ID {}",
            start_time.elapsed(),
            source.display_id,
            source.device_name,
            scap_display_id
        );

        // Configure screen capture
        let capture_config = ScreenCaptureConfig {
            fps: source.fps,
            show_cursor: source.capture_cursor,
            show_highlight: false,
            output_resolution: Resolution::Captured,
        };

        // Start native screen capture
        let frame_rx = self.screen_capture.start_display_capture(scap_display_id, capture_config)?;
        log::debug!("[{:?}] Screen capture started", start_time.elapsed());

        // Create output broadcast channel
        let (output_tx, output_rx) = broadcast::channel::<Bytes>(DEFAULT_CHANNEL_CAPACITY);

        // Get frame dimensions from display list (not blocking on frames)
        let (width, height) = self.get_frame_dimensions(&source_id)?;
        log::debug!("[{:?}] Got frame dimensions: {}x{}", start_time.elapsed(), width, height);

        // Create session state
        let stop_flag = Arc::new(AtomicBool::new(false));
        let last_accessed = Arc::new(Mutex::new(Instant::now()));

        // Clone values for the capture thread
        let stop_flag_clone = stop_flag.clone();
        let output_tx_clone = output_tx.clone();
        let last_accessed_clone = last_accessed.clone();
        let ffmpeg_path = self.ffmpeg_path.clone();
        let fps = source.fps;
        let source_id_clone = source_id.clone();
        let capture_audio = source.capture_audio;

        // Spawn the capture + encoding thread (HTTP mode - empty RTSP URL)
        let capture_handle = std::thread::spawn(move || {
            run_capture_encoding_loop_http(
                frame_rx,
                output_tx_clone,
                stop_flag_clone,
                last_accessed_clone,
                ffmpeg_path,
                width,
                height,
                fps,
                encoding,
                source_id_clone,
                capture_audio,
            );
        });

        // Store the session
        {
            let mut sessions = self.sessions.lock().unwrap();
            sessions.insert(
                source_id.clone(),
                H264CaptureSession {
                    stop_flag,
                    output_tx: Some(output_tx),
                    last_accessed,
                    _capture_handle: capture_handle,
                    width,
                    height,
                    display_id: scap_display_id,
                },
            );
        }

        log::info!(
            "[{:?}] H264 capture started for source {} ({}x{} @ {}fps, HTTP/MPEG-TS)",
            start_time.elapsed(),
            source_id, width, height, fps
        );

        Ok(output_rx)
    }

    /// Get or start a stream for a source (HTTP mode)
    /// If the stream is already running, returns a new subscriber
    pub fn get_or_start_stream(
        &self,
        source: &ScreenCaptureSource,
    ) -> Result<broadcast::Receiver<Bytes>, String> {
        let source_id = &source.id;

        // Check for existing session first
        {
            let sessions = self.sessions.lock().unwrap();
            if let Some(session) = sessions.get(source_id) {
                if let Some(ref tx) = session.output_tx {
                    *session.last_accessed.lock().unwrap() = Instant::now();
                    return Ok(tx.subscribe());
                }
            }
        }

        // Start new capture
        self.start_capture_http(source, None)
    }

    /// Subscribe to an existing HTTP stream by source ID.
    /// Returns a new subscriber if the stream exists, None otherwise.
    /// This is used by the HTTP streaming endpoint to serve data to go2rtc.
    pub fn subscribe_to_stream(&self, source_id: &str) -> Option<broadcast::Receiver<Bytes>> {
        let sessions = self.sessions.lock().unwrap();
        if let Some(session) = sessions.get(source_id) {
            if let Some(ref tx) = session.output_tx {
                *session.last_accessed.lock().unwrap() = Instant::now();
                return Some(tx.subscribe());
            }
        }
        None
    }

    /// Check if a capture session is active
    pub fn is_capturing(&self, source_id: &str) -> bool {
        let sessions = self.sessions.lock().unwrap();
        sessions.contains_key(source_id)
    }

    /// Stop a capture session
    pub fn stop_capture(&self, source_id: &str) -> Result<(), String> {
        let mut sessions = self.sessions.lock().unwrap();

        if let Some(session) = sessions.remove(source_id) {
            log::info!("Stopping H264 capture for source: {}", source_id);
            session.stop_flag.store(true, Ordering::SeqCst);

            // Also stop the underlying screen capture using the stored display_id
            let capture_id = format!("display_{}", session.display_id);
            let _ = self.screen_capture.stop_capture(&capture_id);

            Ok(())
        } else {
            Err(format!("No active capture for source: {}", source_id))
        }
    }

    /// Stop all capture sessions
    pub fn stop_all(&self) {
        let mut sessions = self.sessions.lock().unwrap();

        for (source_id, session) in sessions.drain() {
            log::info!("Stopping H264 capture for source: {}", source_id);
            session.stop_flag.store(true, Ordering::SeqCst);
        }

        // Also stop all screen captures
        self.screen_capture.stop_all();
    }

    /// Get info about active captures
    pub fn active_captures(&self) -> Vec<(String, u32, u32)> {
        let sessions = self.sessions.lock().unwrap();
        sessions
            .iter()
            .map(|(id, session)| (id.clone(), session.width, session.height))
            .collect()
    }

    /// Clean up orphaned sessions that have been inactive too long
    pub fn cleanup_orphans(&self) {
        let mut sessions = self.sessions.lock().unwrap();
        let now = Instant::now();

        let orphans: Vec<String> = sessions
            .iter()
            .filter(|(_, session)| {
                let last = *session.last_accessed.lock().unwrap();
                // Check if timeout exceeded since last access
                now.duration_since(last).as_secs() > ORPHAN_TIMEOUT_SECS
            })
            .map(|(id, _)| id.clone())
            .collect();

        for source_id in orphans {
            if let Some(session) = sessions.remove(&source_id) {
                log::info!("Cleaning up orphaned H264 capture: {}", source_id);
                session.stop_flag.store(true, Ordering::SeqCst);
            }
        }
    }

    /// Find the scap display ID that corresponds to the source's display_id
    /// The source's display_id is typically an AVFoundation index which doesn't match scap's IDs
    fn find_scap_display_id(&self, source: &ScreenCaptureSource) -> Result<u32, String> {
        // Get list of scap displays
        let displays = ScreenCaptureService::list_displays();

        if displays.is_empty() {
            return Err("No displays available for capture".to_string());
        }

        log::debug!(
            "Finding scap display for source display_id='{}', device_name={:?}. Available scap displays: {:?}",
            source.display_id,
            source.device_name,
            displays.iter().map(|d| (d.id, &d.name)).collect::<Vec<_>>()
        );

        // Strategy 1: Try to match by device_name if available
        // The device_name might be something like "Capture screen 0" which could match scap's display title
        if let Some(ref device_name) = source.device_name {
            for display in &displays {
                // Check if the scap display name contains relevant parts of the device name
                // or if the device name contains the display index
                if display.name.contains(device_name) || device_name.contains(&display.name) {
                    log::debug!("Matched display by device_name: scap ID {} ('{}')", display.id, display.name);
                    return Ok(display.id);
                }

                // Check if device_name contains "screen X" and matches display index pattern
                if let Some(screen_num) = extract_screen_number(device_name) {
                    // Try matching by screen number position
                    if screen_num < displays.len() {
                        let matched_display = &displays[screen_num];
                        log::debug!(
                            "Matched display by screen number {}: scap ID {} ('{}')",
                            screen_num, matched_display.id, matched_display.name
                        );
                        return Ok(matched_display.id);
                    }
                }
            }
        }

        // Strategy 2: Try to use display_id as an index into the display list
        if let Ok(index) = source.display_id.parse::<usize>() {
            if index < displays.len() {
                let display = &displays[index];
                log::debug!(
                    "Matched display by index {}: scap ID {} ('{}')",
                    index, display.id, display.name
                );
                return Ok(display.id);
            }
        }

        // Strategy 3: Fall back to first (usually primary) display
        let primary = &displays[0];
        log::warn!(
            "Could not match display_id '{}', falling back to first display: scap ID {} ('{}')",
            source.display_id, primary.id, primary.name
        );
        Ok(primary.id)
    }

    /// Get frame dimensions from the display
    fn get_frame_dimensions(&self, source_id: &str) -> Result<(u32, u32), String> {
        // Parse display ID from source_id and get dimensions from display info
        // For now, use common defaults - the actual frame dimensions come from scap
        // We'll update this when we receive the first frame

        // Try to get from screen capture service by listing displays
        let displays = ScreenCaptureService::list_displays();

        // Find matching display
        for display in &displays {
            if display.id.to_string() == *source_id || display.name.contains(source_id) {
                // Use a reasonable default based on typical display sizes
                // The actual frame dimensions will be determined from the first frame
                return Ok((1920, 1080));
            }
        }

        // Default fallback
        Ok((1920, 1080))
    }
}

impl Drop for H264CaptureService {
    fn drop(&mut self) {
        self.stop_all();
    }
}

/// Main capture and encoding loop running in a separate thread (RTSP output mode)
fn run_capture_encoding_loop(
    mut frame_rx: broadcast::Receiver<Arc<Frame>>,
    stop_flag: Arc<AtomicBool>,
    last_accessed: Arc<Mutex<Instant>>,
    ffmpeg_path: String,
    initial_width: u32,
    initial_height: u32,
    fps: u32,
    encoding: H264EncodingConfig,
    source_id: String,
    capture_audio: bool,
    rtsp_output_url: String,
) {
    let encoding_start = Instant::now();

    // Wait for the first frame to get actual dimensions
    let (width, height) = match wait_for_first_frame(&mut frame_rx, &stop_flag) {
        Some((w, h)) => (w, h),
        None => {
            log::warn!("No frames received for H264 capture: {}", source_id);
            return;
        }
    };

    // Cap resolution at 1280x720 for low latency (matches camera resolution)
    // Higher resolutions (1920x1080, 2560x1440) cause significant encoding latency
    let (target_width, target_height, needs_scale) = if width > 1280 || height > 720 {
        // Scale down maintaining aspect ratio
        let scale_w = 1280.0 / width as f64;
        let scale_h = 720.0 / height as f64;
        let scale = scale_w.min(scale_h);
        // Round to even numbers for YUV420p compatibility
        let new_w = ((width as f64 * scale) as u32 / 2) * 2;
        let new_h = ((height as f64 * scale) as u32 / 2) * 2;
        log::info!(
            "Capping resolution from {}x{} to {}x{} for low latency",
            width, height, new_w, new_h
        );
        (new_w, new_h, true)
    } else {
        (width, height, false)
    };

    log::debug!(
        "[{:?}] First frame received for {}: {}x{} -> {}x{} (initial estimate was {}x{}, RTSP output)",
        encoding_start.elapsed(),
        source_id, width, height, target_width, target_height, initial_width, initial_height
    );

    // Build FFmpeg command for encoding
    // Use low-latency flags to reduce startup time
    let mut ffmpeg_args = vec![
        "-hide_banner".to_string(),
        "-v".to_string(),
        "error".to_string(),
        "-fflags".to_string(),
        "+genpts+nobuffer".to_string(), // Generate PTS, minimize buffering
        "-flags".to_string(),
        "low_delay".to_string(),
        // Input 0: raw video from stdin
        "-f".to_string(),
        "rawvideo".to_string(),
        "-pix_fmt".to_string(),
        "bgra".to_string(),
        "-s".to_string(),
        format!("{}x{}", width, height),
        "-r".to_string(),
        fps.to_string(),
        "-i".to_string(),
        "pipe:0".to_string(),
    ];

    // Add audio input if capture_audio is enabled
    // On macOS: Use avfoundation to capture default audio input
    // On Windows: Use dshow audio capture
    // On Linux: Use ALSA/PulseAudio
    // Note: True desktop/system audio capture on macOS requires BlackHole or similar virtual device
    if capture_audio {
        #[cfg(target_os = "macos")]
        {
            // macOS: capture from default audio input device (typically microphone)
            // :0 is typically the first/default audio input device
            // For system audio, user needs to configure BlackHole/Soundflower
            ffmpeg_args.extend([
                "-f".to_string(),
                "avfoundation".to_string(),
                "-i".to_string(),
                ":0".to_string(), // First audio input device (typically default mic)
            ]);
            log::info!("Audio capture enabled (macOS avfoundation audio device :0)");
        }

        #[cfg(target_os = "windows")]
        {
            // Windows: capture from default audio input
            ffmpeg_args.extend([
                "-f".to_string(),
                "dshow".to_string(),
                "-i".to_string(),
                "audio=default".to_string(),
            ]);
            log::info!("Audio capture enabled (Windows dshow default input)");
        }

        #[cfg(target_os = "linux")]
        {
            // Linux: capture from default PulseAudio source
            ffmpeg_args.extend([
                "-f".to_string(),
                "pulse".to_string(),
                "-i".to_string(),
                "default".to_string(),
            ]);
            log::info!("Audio capture enabled (Linux PulseAudio default)");
        }
    }

    // Add scale filter if resolution needs capping
    if needs_scale {
        ffmpeg_args.extend([
            "-vf".to_string(),
            format!("scale={}:{}:force_original_aspect_ratio=decrease,pad={}:{}:(ow-iw)/2:(oh-ih)/2",
                    target_width, target_height, target_width, target_height),
        ]);
    }

    // Add video encoder settings with low-latency focus
    if encoding.use_hw_accel && cfg!(target_os = "macos") {
        // Use VideoToolbox hardware encoder on macOS with low-latency settings
        ffmpeg_args.extend([
            "-c:v".to_string(),
            "h264_videotoolbox".to_string(),
            "-realtime".to_string(),
            "1".to_string(),
            "-prio_speed".to_string(),
            "1".to_string(), // Prioritize speed over quality for lower latency
            "-allow_sw".to_string(),
            "1".to_string(), // Fallback to software if HW unavailable
            "-profile:v".to_string(),
            "baseline".to_string(), // Faster encoding, simpler profile
            "-level".to_string(),
            "3.1".to_string(),
        ]);
    } else {
        // Use libx264 software encoder with zerolatency tune
        ffmpeg_args.extend([
            "-c:v".to_string(),
            "libx264".to_string(),
            "-preset".to_string(),
            encoding.preset.clone(),
            "-tune".to_string(),
            "zerolatency".to_string(),
            "-profile:v".to_string(),
            "baseline".to_string(),
        ]);
    }

    // Video encoding options
    ffmpeg_args.extend([
        "-g".to_string(),
        encoding.keyframe_interval.to_string(),
        "-b:v".to_string(),
        format!("{}k", encoding.bitrate_kbps),
        "-maxrate".to_string(),
        format!("{}k", encoding.bitrate_kbps * 2),
        "-bufsize".to_string(),
        format!("{}k", encoding.bitrate_kbps),
        "-pix_fmt".to_string(),
        "yuv420p".to_string(),
        // Color space metadata for correct YUV-to-RGB conversion in browsers
        // Without this, browsers may use BT.601 instead of BT.709, causing green/pink tint
        "-colorspace".to_string(),
        "bt709".to_string(),
        "-color_primaries".to_string(),
        "bt709".to_string(),
        "-color_trc".to_string(),
        "bt709".to_string(),
        // Limited range (16-235) as expected by most decoders - prevents green tint
        "-color_range".to_string(),
        "tv".to_string(),
    ]);

    // Add audio encoder or disable audio
    if capture_audio {
        ffmpeg_args.extend([
            "-c:a".to_string(),
            "aac".to_string(),
            "-b:a".to_string(),
            "128k".to_string(),
            "-ar".to_string(),
            "48000".to_string(),
        ]);
    } else {
        ffmpeg_args.push("-an".to_string()); // No audio
    }

    // Output: RTSP push to go2rtc (low latency passthrough)
    ffmpeg_args.extend([
        "-rtsp_transport".to_string(),
        "tcp".to_string(),
        "-f".to_string(),
        "rtsp".to_string(),
        rtsp_output_url.clone(),
    ]);
    log::info!("FFmpeg outputting to RTSP: {}", rtsp_output_url);

    log::debug!("FFmpeg command: {} {:?}", ffmpeg_path, ffmpeg_args);

    // Spawn FFmpeg process (RTSP mode - no stdout needed)
    let mut ffmpeg = match Command::new(&ffmpeg_path)
        .args(&ffmpeg_args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(e) => {
            log::error!("Failed to spawn FFmpeg for H264 capture: {}", e);
            return;
        }
    };

    let ffmpeg_pid = ffmpeg.id();
    log::info!(
        "[{:?}] FFmpeg started for H264 capture (PID: {}, RTSP passthrough)",
        encoding_start.elapsed(),
        ffmpeg_pid
    );

    let mut stdin = ffmpeg.stdin.take().expect("Failed to get FFmpeg stdin");

    // Main loop: read frames and write to FFmpeg stdin
    let frame_size = (width * height * 4) as usize; // BGRA = 4 bytes per pixel

    while !stop_flag.load(Ordering::SeqCst) {
        // Update last accessed
        *last_accessed.lock().unwrap() = Instant::now();

        // Use blocking_recv() since we're in a sync thread
        match frame_rx.blocking_recv() {
            Ok(frame) => {
                // Extract raw frame data
                if let Some(data) = extract_frame_data(&frame, frame_size) {
                    if let Err(e) = stdin.write_all(&data) {
                        log::error!("Failed to write frame to FFmpeg: {}", e);
                        break;
                    }
                }
            }
            Err(broadcast::error::RecvError::Lagged(n)) => {
                log::warn!("H264 capture lagged by {} frames for {}", n, source_id);
                // Continue processing
            }
            Err(broadcast::error::RecvError::Closed) => {
                log::info!("Screen capture channel closed for {}", source_id);
                break;
            }
        }
    }

    // Cleanup
    log::info!("Stopping H264 capture encoding loop for {}", source_id);

    // Close stdin to signal FFmpeg to finish
    drop(stdin);

    // Wait for FFmpeg to exit
    let _ = ffmpeg.wait();

    log::info!("H264 capture stopped for {}", source_id);
}

/// Main capture and encoding loop for HTTP/MPEG-TS output mode
fn run_capture_encoding_loop_http(
    mut frame_rx: broadcast::Receiver<Arc<Frame>>,
    output_tx: broadcast::Sender<Bytes>,
    stop_flag: Arc<AtomicBool>,
    last_accessed: Arc<Mutex<Instant>>,
    ffmpeg_path: String,
    initial_width: u32,
    initial_height: u32,
    fps: u32,
    encoding: H264EncodingConfig,
    source_id: String,
    capture_audio: bool,
) {
    let encoding_start = Instant::now();

    // Wait for the first frame to get actual dimensions
    let (width, height) = match wait_for_first_frame(&mut frame_rx, &stop_flag) {
        Some((w, h)) => (w, h),
        None => {
            log::warn!("No frames received for H264 capture: {}", source_id);
            return;
        }
    };

    // Cap resolution at 1280x720 for low latency
    let (target_width, target_height, needs_scale) = if width > 1280 || height > 720 {
        let scale_w = 1280.0 / width as f64;
        let scale_h = 720.0 / height as f64;
        let scale = scale_w.min(scale_h);
        let new_w = ((width as f64 * scale) as u32 / 2) * 2;
        let new_h = ((height as f64 * scale) as u32 / 2) * 2;
        log::info!("Capping resolution from {}x{} to {}x{} for low latency", width, height, new_w, new_h);
        (new_w, new_h, true)
    } else {
        (width, height, false)
    };

    log::debug!(
        "[{:?}] First frame received for {}: {}x{} -> {}x{} (initial estimate was {}x{}, HTTP/MPEG-TS mode)",
        encoding_start.elapsed(), source_id, width, height, target_width, target_height, initial_width, initial_height
    );

    // Build FFmpeg command
    let mut ffmpeg_args = vec![
        "-hide_banner".to_string(),
        "-v".to_string(), "error".to_string(),
        "-fflags".to_string(), "+genpts+nobuffer".to_string(),
        "-flags".to_string(), "low_delay".to_string(),
        "-f".to_string(), "rawvideo".to_string(),
        "-pix_fmt".to_string(), "bgra".to_string(),
        "-s".to_string(), format!("{}x{}", width, height),
        "-r".to_string(), fps.to_string(),
        "-i".to_string(), "pipe:0".to_string(),
    ];

    // Audio input
    if capture_audio {
        #[cfg(target_os = "macos")]
        ffmpeg_args.extend(["-f".to_string(), "avfoundation".to_string(), "-i".to_string(), ":0".to_string()]);
        #[cfg(target_os = "windows")]
        ffmpeg_args.extend(["-f".to_string(), "dshow".to_string(), "-i".to_string(), "audio=default".to_string()]);
        #[cfg(target_os = "linux")]
        ffmpeg_args.extend(["-f".to_string(), "pulse".to_string(), "-i".to_string(), "default".to_string()]);
    }

    // Scale filter if needed
    if needs_scale {
        ffmpeg_args.extend(["-vf".to_string(),
            format!("scale={}:{}:force_original_aspect_ratio=decrease,pad={}:{}:(ow-iw)/2:(oh-ih)/2",
                    target_width, target_height, target_width, target_height)]);
    }

    // Video encoder
    if encoding.use_hw_accel && cfg!(target_os = "macos") {
        ffmpeg_args.extend([
            "-c:v".to_string(), "h264_videotoolbox".to_string(),
            "-realtime".to_string(), "1".to_string(),
            "-prio_speed".to_string(), "1".to_string(),
            "-allow_sw".to_string(), "1".to_string(),
            "-profile:v".to_string(), "baseline".to_string(),
            "-level".to_string(), "3.1".to_string(),
        ]);
    } else {
        ffmpeg_args.extend([
            "-c:v".to_string(), "libx264".to_string(),
            "-preset".to_string(), encoding.preset.clone(),
            "-tune".to_string(), "zerolatency".to_string(),
            "-profile:v".to_string(), "baseline".to_string(),
        ]);
    }

    // Encoding options with color space metadata
    ffmpeg_args.extend([
        "-g".to_string(), encoding.keyframe_interval.to_string(),
        "-b:v".to_string(), format!("{}k", encoding.bitrate_kbps),
        "-maxrate".to_string(), format!("{}k", encoding.bitrate_kbps * 2),
        "-bufsize".to_string(), format!("{}k", encoding.bitrate_kbps),
        "-pix_fmt".to_string(), "yuv420p".to_string(),
        "-colorspace".to_string(), "bt709".to_string(),
        "-color_primaries".to_string(), "bt709".to_string(),
        "-color_trc".to_string(), "bt709".to_string(),
        "-color_range".to_string(), "tv".to_string(),
    ]);

    // Audio or no audio
    if capture_audio {
        ffmpeg_args.extend(["-c:a".to_string(), "aac".to_string(), "-b:a".to_string(), "128k".to_string(), "-ar".to_string(), "48000".to_string()]);
    } else {
        ffmpeg_args.push("-an".to_string());
    }

    // MPEG-TS output to stdout
    ffmpeg_args.extend([
        "-flush_packets".to_string(), "1".to_string(),
        "-f".to_string(), "mpegts".to_string(),
        "-muxdelay".to_string(), "0".to_string(),
        "pipe:1".to_string(),
    ]);

    log::debug!("FFmpeg command: {} {:?}", ffmpeg_path, ffmpeg_args);

    // Spawn FFmpeg
    let mut ffmpeg = match Command::new(&ffmpeg_path)
        .args(&ffmpeg_args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(e) => {
            log::error!("Failed to spawn FFmpeg: {}", e);
            return;
        }
    };

    let ffmpeg_pid = ffmpeg.id();
    log::info!("[{:?}] FFmpeg started (PID: {}, HTTP/MPEG-TS mode)", encoding_start.elapsed(), ffmpeg_pid);

    let mut stdin = ffmpeg.stdin.take().expect("stdin");
    let stdout = ffmpeg.stdout.take().expect("stdout");

    // Spawn output reader thread
    let stop_flag_clone = stop_flag.clone();
    let source_id_clone = source_id.clone();
    let output_thread = std::thread::spawn(move || {
        read_mpegts_output(stdout, output_tx, stop_flag_clone, source_id_clone);
    });

    // Main loop: read frames and write to FFmpeg
    let frame_size = (width * height * 4) as usize;

    while !stop_flag.load(Ordering::SeqCst) {
        *last_accessed.lock().unwrap() = Instant::now();

        match frame_rx.blocking_recv() {
            Ok(frame) => {
                if let Some(data) = extract_frame_data(&frame, frame_size) {
                    if let Err(e) = stdin.write_all(&data) {
                        log::error!("Failed to write frame: {}", e);
                        break;
                    }
                }
            }
            Err(broadcast::error::RecvError::Lagged(n)) => {
                log::warn!("Lagged by {} frames for {}", n, source_id);
            }
            Err(broadcast::error::RecvError::Closed) => {
                log::info!("Channel closed for {}", source_id);
                break;
            }
        }
    }

    log::info!("Stopping HTTP encoding loop for {}", source_id);
    drop(stdin);
    let _ = ffmpeg.wait();
    let _ = output_thread.join();
    log::info!("HTTP capture stopped for {}", source_id);
}

/// Read MPEG-TS output from FFmpeg and broadcast chunks
fn read_mpegts_output(
    mut stdout: std::process::ChildStdout,
    output_tx: broadcast::Sender<Bytes>,
    stop_flag: Arc<AtomicBool>,
    source_id: String,
) {
    const CHUNK_SIZE: usize = 188 * 7; // TS packet multiples
    let mut buffer = vec![0u8; CHUNK_SIZE];

    log::debug!("Starting MPEG-TS reader for {}", source_id);

    loop {
        if stop_flag.load(Ordering::SeqCst) {
            break;
        }

        match stdout.read(&mut buffer) {
            Ok(0) => {
                log::debug!("FFmpeg stdout EOF for {}", source_id);
                break;
            }
            Ok(n) => {
                let chunk = Bytes::copy_from_slice(&buffer[..n]);
                let _ = output_tx.send(chunk);
            }
            Err(e) => {
                if e.kind() != std::io::ErrorKind::Interrupted {
                    log::error!("Error reading FFmpeg output: {}", e);
                    break;
                }
            }
        }
    }

    log::debug!("MPEG-TS reader stopped for {}", source_id);
}

/// Wait for the first frame to determine actual dimensions
fn wait_for_first_frame(
    frame_rx: &mut broadcast::Receiver<Arc<Frame>>,
    stop_flag: &Arc<AtomicBool>,
) -> Option<(u32, u32)> {
    let start = Instant::now();
    let timeout = Duration::from_secs(5); // Reduced from 10s

    loop {
        if stop_flag.load(Ordering::SeqCst) {
            return None;
        }

        if start.elapsed() > timeout {
            log::warn!("Timeout waiting for first frame");
            return None;
        }

        // Use blocking_recv with a short timeout instead of polling
        // This is more efficient than try_recv + sleep
        match frame_rx.blocking_recv() {
            Ok(frame) => {
                let elapsed = start.elapsed();
                log::debug!("First frame received in {:?}", elapsed);
                let (width, height) = get_frame_dimensions(&frame);
                return Some((width, height));
            }
            Err(broadcast::error::RecvError::Lagged(n)) => {
                log::debug!("Lagged by {} frames while waiting for first frame", n);
                // Continue to get the next frame
            }
            Err(broadcast::error::RecvError::Closed) => {
                return None;
            }
        }
    }
}

/// Get dimensions from a scap Frame
fn get_frame_dimensions(frame: &Frame) -> (u32, u32) {
    match frame {
        Frame::BGRA(bgra) => (bgra.width as u32, bgra.height as u32),
        Frame::RGB(rgb) => (rgb.width as u32, rgb.height as u32),
        Frame::YUVFrame(yuv) => (yuv.width as u32, yuv.height as u32),
        _ => (1920, 1080), // Fallback for other formats
    }
}

/// Extract raw frame data from a scap Frame
fn extract_frame_data(frame: &Frame, expected_size: usize) -> Option<Vec<u8>> {
    let data = match frame {
        Frame::BGRA(bgra) => bgra.data.clone(),
        _ => {
            log::warn!("Unexpected frame format, expected BGRA");
            return None;
        }
    };

    // Verify size
    if data.len() < expected_size {
        log::warn!(
            "Frame data size mismatch: expected {}, got {}",
            expected_size,
            data.len()
        );
    }

    Some(data)
}

/// Extract screen number from a device name like "Capture screen 0" or "Screen 1"
fn extract_screen_number(device_name: &str) -> Option<usize> {
    // Try to find patterns like "screen 0", "Screen 1", "screen0", etc.
    let lower = device_name.to_lowercase();

    // Look for "screen" followed by a number
    if let Some(pos) = lower.find("screen") {
        let after_screen = &device_name[pos + 6..];
        // Skip any whitespace
        let trimmed = after_screen.trim_start();
        // Try to parse the number
        let num_str: String = trimmed.chars().take_while(|c| c.is_ascii_digit()).collect();
        if !num_str.is_empty() {
            return num_str.parse().ok();
        }
    }

    // Try to find just a trailing number
    let num_str: String = device_name.chars().rev().take_while(|c| c.is_ascii_digit()).collect();
    if !num_str.is_empty() {
        let reversed: String = num_str.chars().rev().collect();
        return reversed.parse().ok();
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encoding_config_defaults() {
        let config = H264EncodingConfig::default();
        assert_eq!(config.bitrate_kbps, 4000);
        assert_eq!(config.keyframe_interval, 15); // Reduced from 30 for faster initial frame
        assert_eq!(config.preset, "ultrafast");
        assert!(config.use_hw_accel);
    }

    #[test]
    fn test_extract_screen_number() {
        assert_eq!(extract_screen_number("Capture screen 0"), Some(0));
        assert_eq!(extract_screen_number("Capture screen 1"), Some(1));
        assert_eq!(extract_screen_number("Screen 2"), Some(2));
        assert_eq!(extract_screen_number("screen0"), Some(0));
        assert_eq!(extract_screen_number("Display 3"), Some(3));
        assert_eq!(extract_screen_number("Main Display"), None);
    }
}
