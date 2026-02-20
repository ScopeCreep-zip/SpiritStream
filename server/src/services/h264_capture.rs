// H264 Capture Service
// Captures screen frames via scap and encodes to H264 using FFmpeg
// Supports two output modes:
// 1. HTTP/MPEG-TS streaming (for go2rtc with #video=copy passthrough)
// 2. RTSP push to go2rtc (alternative low-latency mode)

use std::collections::HashMap;
use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use parking_lot::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use bytes::Bytes;
use scap::capturer::Resolution;
use scap::frame::Frame;
use tokio::sync::broadcast;

use super::screen_capture::{ScreenCaptureConfig, ScreenCaptureService};
use super::frame_processing::{get_frame_dimensions, extract_frame_data};
use crate::models::ScreenCaptureSource;
use crate::services::ActivityAssertion;

const DEFAULT_CHANNEL_CAPACITY: usize = 64;
const ORPHAN_TIMEOUT_SECS: u64 = 30;
/// Default max hardware encoder sessions.
/// Apple Silicon (M1/M2): 3 concurrent VideoToolbox sessions.
/// Intel Macs: 1 attempt max — HW encoding is unreliable, graceful fallback to libx264.
const DEFAULT_HW_ENCODER_MAX: u8 = if cfg!(target_arch = "aarch64") { 3 } else { 1 };

use std::sync::atomic::AtomicU8;

/// Tracks hardware video encoder session budget.
/// M1/M2 Macs support ~3-4 concurrent VideoToolbox H264 sessions.
/// Exceeding this causes silent degradation or failure.
pub struct HwEncoderBudget {
    max_sessions: AtomicU8,
    active_sessions: Arc<AtomicU8>,
}

impl HwEncoderBudget {
    pub fn new(max: u8) -> Self {
        Self {
            max_sessions: AtomicU8::new(max),
            active_sessions: Arc::new(AtomicU8::new(0)),
        }
    }

    /// Try to acquire a hardware encoder session.
    /// Returns a RAII guard that releases on drop, or None if budget exhausted.
    pub fn try_acquire(&self) -> Option<HwSessionGuard> {
        loop {
            let current = self.active_sessions.load(Ordering::SeqCst);
            let max = self.max_sessions.load(Ordering::Relaxed);
            if current >= max {
                return None;
            }
            if self.active_sessions.compare_exchange(
                current, current + 1, Ordering::SeqCst, Ordering::SeqCst
            ).is_ok() {
                return Some(HwSessionGuard { active_sessions: Arc::clone(&self.active_sessions) });
            }
        }
    }

    /// Get count of active hardware sessions
    pub fn active_count(&self) -> u8 {
        self.active_sessions.load(Ordering::Relaxed)
    }
}

/// RAII guard that releases a hardware encoder session on drop.
/// Owns an Arc to the counter so it can be moved into threads.
pub struct HwSessionGuard {
    active_sessions: Arc<AtomicU8>,
}

impl Drop for HwSessionGuard {
    fn drop(&mut self) {
        self.active_sessions.fetch_sub(1, Ordering::SeqCst);
        log::debug!("Released HW encoder session (active: {})", self.active_sessions.load(Ordering::Relaxed));
    }
}

/// Get current epoch millis for atomic timestamp
fn now_epoch_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// Active H264 capture session
struct H264CaptureSession {
    /// Stop flag for graceful shutdown
    stop_flag: Arc<AtomicBool>,
    /// Output broadcast sender (for HTTP mode)
    output_tx: Option<broadcast::Sender<Bytes>>,
    /// Last access time as epoch millis (lock-free)
    last_accessed: Arc<AtomicU64>,
    /// Screen capture thread handle
    _capture_handle: std::thread::JoinHandle<()>,
    /// Width of captured frames
    width: u32,
    /// Height of captured frames
    height: u32,
    /// Display ID for stopping the underlying screen capture
    display_id: u32,
    /// RAII guard for hardware encoder budget (releases slot on session drop)
    _hw_guard: Option<HwSessionGuard>,
}

/// Configuration for H264 encoding
#[derive(Debug, Clone)]
pub struct H264EncodingConfig {
    /// Video bitrate in kbps (default: 4000)
    pub bitrate_kbps: u32,
    /// Keyframe interval in frames (default: 5 = ~160ms at 30fps for faster preview)
    pub keyframe_interval: u32,
    /// Encoding preset (ultrafast, superfast, veryfast, faster, fast, medium)
    pub preset: String,
    /// Use hardware encoding if available (VideoToolbox on macOS)
    pub use_hw_accel: bool,
}

impl Default for H264EncodingConfig {
    fn default() -> Self {
        // Intel Macs: default to software encoding at lower bitrate.
        // VideoToolbox on Intel silently falls back to CPU-based encoding
        // at hardware-quality settings (4000kbps), wasting 15-25% CPU per session.
        if cfg!(target_arch = "x86_64") {
            Self {
                bitrate_kbps: 2000,
                keyframe_interval: 5,
                preset: "ultrafast".to_string(),
                use_hw_accel: false,
            }
        } else {
            Self {
                bitrate_kbps: 4000,
                keyframe_interval: 5, // ~160ms at 30fps for faster preview switching
                preset: "ultrafast".to_string(),
                use_hw_accel: true,
            }
        }
    }
}

/// Service for capturing screen to H264 via RTSP push to go2rtc
pub struct H264CaptureService {
    sessions: Mutex<HashMap<String, H264CaptureSession>>,
    screen_capture: Arc<ScreenCaptureService>,
    ffmpeg_path: String,
    /// Handle to the background cleanup task for clean shutdown
    cleanup_handle: std::sync::Mutex<Option<tokio::task::JoinHandle<()>>>,
    /// Prevents macOS App Nap from throttling H264 capture threads
    activity_assertion: Mutex<Option<ActivityAssertion>>,
    /// Hardware encoder session budget (VideoToolbox slots)
    hw_budget: HwEncoderBudget,
}

impl H264CaptureService {
    /// Create a new H264CaptureService
    pub fn new(screen_capture: Arc<ScreenCaptureService>, ffmpeg_path: String) -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
            screen_capture,
            ffmpeg_path,
            cleanup_handle: std::sync::Mutex::new(None),
            activity_assertion: Mutex::new(None),
            hw_budget: HwEncoderBudget::new(DEFAULT_HW_ENCODER_MAX),
        }
    }

    /// Get the hardware encoder budget (for external monitoring)
    pub fn hw_budget(&self) -> &HwEncoderBudget {
        &self.hw_budget
    }

    /// Acquire App Nap prevention when first capture starts
    fn acquire_activity_assertion(&self) {
        {
            let mut guard = self.activity_assertion.lock();
            if guard.is_none() {
                match ActivityAssertion::begin("SpiritStream H264 capture active") {
                    Ok(assertion) => *guard = Some(assertion),
                    Err(e) => log::warn!("Failed to acquire H264 activity assertion: {}", e),
                }
            }
        }
    }

    /// Release App Nap prevention when all captures stop
    fn release_activity_assertion_if_idle(&self) {
        let is_empty = self.sessions.lock().is_empty();
        if is_empty {
            {
            let mut guard = self.activity_assertion.lock();
                if guard.take().is_some() {
                    log::info!("Released H264 capture activity assertion");
                }
            }
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
        self.acquire_activity_assertion();
        let source_id = source.id.clone();
        let mut encoding = encoding_config.unwrap_or_default();

        // Check if already capturing
        {
            let sessions = self.sessions.lock();
            if sessions.contains_key(&source_id) {
                log::debug!("H264 capture already running for {}", source_id);
                return Ok(());
            }
        }

        // Check HW encoder budget — fall back to software if exhausted
        let hw_guard = if encoding.use_hw_accel {
            match self.hw_budget.try_acquire() {
                Some(guard) => {
                    log::info!("Acquired HW encoder slot (active: {})", self.hw_budget.active_count());
                    Some(guard)
                }
                None => {
                    log::warn!("HW encoder budget exhausted ({} active), falling back to libx264 ultrafast",
                        self.hw_budget.active_count());
                    encoding.use_hw_accel = false;
                    encoding.preset = "ultrafast".to_string();
                    None
                }
            }
        } else {
            None
        };

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
        let last_accessed = Arc::new(AtomicU64::new(now_epoch_millis()));

        // Clone values for the capture thread
        let stop_flag_clone = stop_flag.clone();
        let last_accessed_clone = last_accessed.clone();
        let ffmpeg_path = self.ffmpeg_path.clone();
        let fps = source.fps;
        let source_id_clone = source_id.clone();
        let capture_audio = source.capture_audio;

        // Spawn the capture + encoding thread
        let capture_handle = std::thread::spawn(move || {
            run_encoding_loop(
                frame_rx,
                OutputMode::Rtsp(rtsp_url),
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

        // Store the session (hw_guard lives as long as the session)
        {
            let mut sessions = self.sessions.lock();
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
                    _hw_guard: hw_guard,
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
        self.acquire_activity_assertion();
        let source_id = source.id.clone();
        let mut encoding = encoding_config.unwrap_or_default();

        // Check if already capturing
        {
            let sessions = self.sessions.lock();
            if let Some(session) = sessions.get(&source_id) {
                if let Some(ref tx) = session.output_tx {
                    session.last_accessed.store(now_epoch_millis(), Ordering::Relaxed);
                    return Ok(tx.subscribe());
                }
            }
        }

        // Check HW encoder budget — fall back to software if exhausted
        let hw_guard = if encoding.use_hw_accel {
            match self.hw_budget.try_acquire() {
                Some(guard) => {
                    log::info!("Acquired HW encoder slot for HTTP (active: {})", self.hw_budget.active_count());
                    Some(guard)
                }
                None => {
                    log::warn!("HW encoder budget exhausted ({} active), falling back to libx264 ultrafast",
                        self.hw_budget.active_count());
                    encoding.use_hw_accel = false;
                    encoding.preset = "ultrafast".to_string();
                    None
                }
            }
        } else {
            None
        };

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
        let last_accessed = Arc::new(AtomicU64::new(now_epoch_millis()));

        // Clone values for the capture thread
        let stop_flag_clone = stop_flag.clone();
        let output_tx_clone = output_tx.clone();
        let last_accessed_clone = last_accessed.clone();
        let ffmpeg_path = self.ffmpeg_path.clone();
        let fps = source.fps;
        let source_id_clone = source_id.clone();
        let capture_audio = source.capture_audio;

        // Spawn the capture + encoding thread (HTTP mode)
        let capture_handle = std::thread::spawn(move || {
            run_encoding_loop(
                frame_rx,
                OutputMode::Http(output_tx_clone),
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

        // Store the session (hw_guard lives as long as the session)
        {
            let mut sessions = self.sessions.lock();
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
                    _hw_guard: hw_guard,
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
            let sessions = self.sessions.lock();
            if let Some(session) = sessions.get(source_id) {
                if let Some(ref tx) = session.output_tx {
                    session.last_accessed.store(now_epoch_millis(), Ordering::Relaxed);
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
        let sessions = self.sessions.lock();
        if let Some(session) = sessions.get(source_id) {
            if let Some(ref tx) = session.output_tx {
                session.last_accessed.store(now_epoch_millis(), Ordering::Relaxed);
                return Some(tx.subscribe());
            }
        }
        None
    }

    /// Check if a capture session is active
    pub fn is_capturing(&self, source_id: &str) -> bool {
        let sessions = self.sessions.lock();
        sessions.contains_key(source_id)
    }

    /// Stop a capture session
    pub fn stop_capture(&self, source_id: &str) -> Result<(), String> {
        {
            let mut sessions = self.sessions.lock();

            if let Some(session) = sessions.remove(source_id) {
                log::info!("Stopping H264 capture for source: {}", source_id);
                session.stop_flag.store(true, Ordering::SeqCst);

                // Also stop the underlying screen capture using the stored display_id
                let capture_id = format!("display_{}", session.display_id);
                let _ = self.screen_capture.stop_capture(&capture_id);
            } else {
                return Err(format!("No active capture for source: {}", source_id));
            }
        }
        self.release_activity_assertion_if_idle();
        Ok(())
    }

    /// Stop all capture sessions
    pub fn stop_all(&self) {
        {
            let mut sessions = self.sessions.lock();

            for (source_id, session) in sessions.drain() {
                log::info!("Stopping H264 capture for source: {}", source_id);
                session.stop_flag.store(true, Ordering::SeqCst);
            }
        }

        // Also stop all screen captures
        self.screen_capture.stop_all();

        // Release activity assertion
        {
            let mut guard = self.activity_assertion.lock();
            if guard.take().is_some() {
                log::info!("Released H264 capture activity assertion (stop_all)");
            }
        }
    }

    /// Get info about active captures
    pub fn active_captures(&self) -> Vec<(String, u32, u32)> {
        let sessions = self.sessions.lock();
        sessions
            .iter()
            .map(|(id, session)| (id.clone(), session.width, session.height))
            .collect()
    }

    /// Clean up orphaned sessions that have been inactive too long
    pub fn cleanup_orphans(&self) {
        let mut sessions = self.sessions.lock();
        let now_millis = now_epoch_millis();

        let orphans: Vec<String> = sessions
            .iter()
            .filter(|(_, session)| {
                let last_millis = session.last_accessed.load(Ordering::Relaxed);
                // Check if timeout exceeded since last access
                now_millis.saturating_sub(last_millis) > ORPHAN_TIMEOUT_SECS * 1000
            })
            .map(|(id, _)| id.clone())
            .collect();

        for source_id in orphans {
            if let Some(session) = sessions.remove(&source_id) {
                log::info!("Cleaning up orphaned H264 capture: {}", source_id);
                session.stop_flag.store(true, Ordering::SeqCst);
                // Also stop the underlying screen capture
                let capture_id = format!("display_{}", session.display_id);
                let _ = self.screen_capture.stop_capture(&capture_id);
            }
        }
    }

    /// Start a background task that periodically cleans up orphaned sessions.
    /// Should be called once during service initialization.
    pub fn start_cleanup_task(self: &Arc<Self>) {
        let service = Arc::clone(self);
        let handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(10));
            loop {
                interval.tick().await;
                service.cleanup_orphans();
            }
        });
        if let Ok(mut h) = self.cleanup_handle.lock() {
            *h = Some(handle);
        }
    }

    /// Stop the background cleanup task
    pub fn stop_cleanup_task(&self) {
        if let Ok(mut h) = self.cleanup_handle.lock() {
            if let Some(handle) = h.take() {
                handle.abort();
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
        self.stop_cleanup_task();
        self.stop_all();
    }
}

/// Output mode for H264 encoding — determines how FFmpeg output is handled
enum OutputMode {
    /// RTSP push to go2rtc (stdout null, no output reader)
    Rtsp(String),
    /// HTTP/MPEG-TS output via broadcast channel (stdout piped, output reader thread)
    Http(broadcast::Sender<Bytes>),
}

/// Cap resolution to 1280x720 for low-latency encoding.
/// Returns (target_width, target_height, needs_scale).
fn cap_resolution(width: u32, height: u32) -> (u32, u32, bool) {
    if width > 1280 || height > 720 {
        let scale = (1280.0 / width as f64).min(720.0 / height as f64);
        // Round to even numbers for YUV420p compatibility
        let new_w = ((width as f64 * scale) as u32 / 2) * 2;
        let new_h = ((height as f64 * scale) as u32 / 2) * 2;
        log::info!("Capping resolution from {}x{} to {}x{} for low latency", width, height, new_w, new_h);
        (new_w, new_h, true)
    } else {
        (width, height, false)
    }
}

/// Build common FFmpeg args shared between RTSP and HTTP output modes
fn build_common_ffmpeg_args(
    width: u32,
    height: u32,
    fps: u32,
    encoding: &H264EncodingConfig,
    needs_scale: bool,
    target_width: u32,
    target_height: u32,
    capture_audio: bool,
) -> Vec<String> {
    let mut args = vec![
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

    if capture_audio {
        log::info!("Screen capture audio requested but not yet implemented - requires ScreenCaptureKit audio integration");
    }

    if needs_scale {
        args.extend([
            "-vf".to_string(),
            format!("scale={}:{}:force_original_aspect_ratio=decrease,pad={}:{}:(ow-iw)/2:(oh-ih)/2",
                    target_width, target_height, target_width, target_height),
        ]);
    }

    // Video encoder — HW (VideoToolbox) or SW (libx264)
    if encoding.use_hw_accel && cfg!(target_os = "macos") {
        args.extend([
            "-c:v".to_string(), "h264_videotoolbox".to_string(),
            "-realtime".to_string(), "1".to_string(),
            "-prio_speed".to_string(), "1".to_string(),
            "-allow_sw".to_string(), "0".to_string(),
            "-profile:v".to_string(), "baseline".to_string(),
            "-level".to_string(), "3.1".to_string(),
        ]);
    } else {
        args.extend([
            "-c:v".to_string(), "libx264".to_string(),
            "-preset".to_string(), encoding.preset.clone(),
            "-tune".to_string(), "zerolatency".to_string(),
            "-profile:v".to_string(), "baseline".to_string(),
        ]);
    }

    // Encoding options with BT.709 color space metadata
    args.extend([
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

    // Disable audio — screen capture audio requires ScreenCaptureKit integration
    args.push("-an".to_string());

    args
}

/// Unified capture and encoding loop for both RTSP and HTTP/MPEG-TS output modes.
/// The two modes share 90%+ identical code — only FFmpeg output config and
/// stdout handling differ, controlled by the `OutputMode` enum.
fn run_encoding_loop(
    mut frame_rx: broadcast::Receiver<Arc<Frame>>,
    output: OutputMode,
    stop_flag: Arc<AtomicBool>,
    last_accessed: Arc<AtomicU64>,
    ffmpeg_path: String,
    initial_width: u32,
    initial_height: u32,
    fps: u32,
    encoding: H264EncodingConfig,
    source_id: String,
    capture_audio: bool,
) {
    crate::services::thread_config::set_thread_qos(crate::services::thread_config::QosClass::UserInitiated);
    let encoding_start = Instant::now();

    let mode_label = match &output {
        OutputMode::Rtsp(_) => "RTSP",
        OutputMode::Http(_) => "HTTP/MPEG-TS",
    };

    // Wait for the first frame to get actual dimensions
    let (width, height) = match wait_for_first_frame(&mut frame_rx, &stop_flag) {
        Some((w, h)) => (w, h),
        None => {
            log::warn!("No frames received for H264 capture: {}", source_id);
            return;
        }
    };

    let (target_width, target_height, needs_scale) = cap_resolution(width, height);

    log::debug!(
        "[{:?}] First frame for {}: {}x{} -> {}x{} (estimate {}x{}, {} mode)",
        encoding_start.elapsed(), source_id, width, height,
        target_width, target_height, initial_width, initial_height, mode_label
    );

    // Build common FFmpeg args, then append output-specific args
    let mut ffmpeg_args = build_common_ffmpeg_args(
        width, height, fps, &encoding, needs_scale, target_width, target_height, capture_audio,
    );

    match &output {
        OutputMode::Rtsp(url) => {
            ffmpeg_args.extend([
                "-rtsp_transport".to_string(), "tcp".to_string(),
                "-f".to_string(), "rtsp".to_string(),
                url.clone(),
            ]);
            log::info!("FFmpeg outputting to RTSP: {}", url);
        }
        OutputMode::Http(_) => {
            ffmpeg_args.extend([
                "-flush_packets".to_string(), "1".to_string(),
                "-f".to_string(), "mpegts".to_string(),
                "-muxdelay".to_string(), "0".to_string(),
                "pipe:1".to_string(),
            ]);
        }
    }

    log::debug!("FFmpeg command: {} {:?}", ffmpeg_path, ffmpeg_args);

    // Spawn FFmpeg — stdout piped only for HTTP mode
    let stdout_cfg = match &output {
        OutputMode::Rtsp(_) => Stdio::null(),
        OutputMode::Http(_) => Stdio::piped(),
    };

    let mut ffmpeg = match Command::new(&ffmpeg_path)
        .args(&ffmpeg_args)
        .stdin(Stdio::piped())
        .stdout(stdout_cfg)
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
    log::info!("[{:?}] FFmpeg started (PID: {}, {} mode)", encoding_start.elapsed(), ffmpeg_pid, mode_label);

    let mut stdin = ffmpeg.stdin.take().expect("Failed to get FFmpeg stdin");

    // Spawn output reader thread only for HTTP mode
    let output_thread = match output {
        OutputMode::Http(output_tx) => {
            let stdout = ffmpeg.stdout.take().expect("stdout");
            let stop_clone = stop_flag.clone();
            let sid = source_id.clone();
            Some(std::thread::spawn(move || {
                crate::services::thread_config::set_thread_qos(crate::services::thread_config::QosClass::Utility);
                read_mpegts_output(stdout, output_tx, stop_clone, sid);
            }))
        }
        OutputMode::Rtsp(_) => None,
    };

    // Main loop: read frames and write to FFmpeg stdin
    let frame_size = (width * height * 4) as usize; // BGRA = 4 bytes per pixel

    while !stop_flag.load(Ordering::SeqCst) {
        last_accessed.store(now_epoch_millis(), Ordering::Relaxed);

        match frame_rx.blocking_recv() {
            Ok(frame) => {
                if let Some(data) = extract_frame_data(&frame, frame_size) {
                    if let Err(e) = stdin.write_all(&data) {
                        log::error!("Failed to write frame to FFmpeg: {}", e);
                        break;
                    }
                }
            }
            Err(broadcast::error::RecvError::Lagged(n)) => {
                log::warn!("H264 capture lagged by {} frames for {}", n, source_id);
            }
            Err(broadcast::error::RecvError::Closed) => {
                log::info!("Screen capture channel closed for {}", source_id);
                break;
            }
        }
    }

    // Cleanup
    log::info!("Stopping {} encoding loop for {}", mode_label, source_id);
    drop(stdin);
    let _ = ffmpeg.wait();
    if let Some(thread) = output_thread {
        let _ = thread.join();
    }
    log::info!("H264 capture stopped for {}", source_id);
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
        assert_eq!(config.keyframe_interval, 5); // ~160ms at 30fps for faster preview switching
        assert_eq!(config.preset, "ultrafast");

        // Architecture-dependent defaults
        if cfg!(target_arch = "aarch64") {
            assert_eq!(config.bitrate_kbps, 4000);
            assert!(config.use_hw_accel);
        } else {
            assert_eq!(config.bitrate_kbps, 2000);
            assert!(!config.use_hw_accel);
        }
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
