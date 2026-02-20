// Preview Handler Service
// Manages FFmpeg processes for MJPEG preview streams

use dashmap::DashMap;
use std::io::{BufRead, BufReader, Read};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use parking_lot::Mutex;
use std::time::{Duration, Instant};
use tokio::sync::broadcast;
use tokio::time::timeout;
use bytes::Bytes;

use crate::models::{Scene, Source};
use crate::services::{Compositor, extract_jpeg_frame};

/// Timeout for snapshot capture (prevents indefinite blocking)
const SNAPSHOT_TIMEOUT_SECS: u64 = 10;

/// Placeholder JPEG for empty/failed scenes (1x1 dark gray pixel, minimal size)
/// This is a valid JPEG that can be served when scene preview fails
const PLACEHOLDER_JPEG: &[u8] = &[
    0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46, 0x49, 0x46, 0x00, 0x01,
    0x01, 0x00, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00, 0xFF, 0xDB, 0x00, 0x43,
    0x00, 0x08, 0x06, 0x06, 0x07, 0x06, 0x05, 0x08, 0x07, 0x07, 0x07, 0x09,
    0x09, 0x08, 0x0A, 0x0C, 0x14, 0x0D, 0x0C, 0x0B, 0x0B, 0x0C, 0x19, 0x12,
    0x13, 0x0F, 0x14, 0x1D, 0x1A, 0x1F, 0x1E, 0x1D, 0x1A, 0x1C, 0x1C, 0x20,
    0x24, 0x2E, 0x27, 0x20, 0x22, 0x2C, 0x23, 0x1C, 0x1C, 0x28, 0x37, 0x29,
    0x2C, 0x30, 0x31, 0x34, 0x34, 0x34, 0x1F, 0x27, 0x39, 0x3D, 0x38, 0x32,
    0x3C, 0x2E, 0x33, 0x34, 0x32, 0xFF, 0xC0, 0x00, 0x0B, 0x08, 0x00, 0x01,
    0x00, 0x01, 0x01, 0x01, 0x11, 0x00, 0xFF, 0xC4, 0x00, 0x1F, 0x00, 0x00,
    0x01, 0x05, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08,
    0x09, 0x0A, 0x0B, 0xFF, 0xC4, 0x00, 0xB5, 0x10, 0x00, 0x02, 0x01, 0x03,
    0x03, 0x02, 0x04, 0x03, 0x05, 0x05, 0x04, 0x04, 0x00, 0x00, 0x01, 0x7D,
    0x01, 0x02, 0x03, 0x00, 0x04, 0x11, 0x05, 0x12, 0x21, 0x31, 0x41, 0x06,
    0x13, 0x51, 0x61, 0x07, 0x22, 0x71, 0x14, 0x32, 0x81, 0x91, 0xA1, 0x08,
    0x23, 0x42, 0xB1, 0xC1, 0x15, 0x52, 0xD1, 0xF0, 0x24, 0x33, 0x62, 0x72,
    0x82, 0x09, 0x0A, 0x16, 0x17, 0x18, 0x19, 0x1A, 0x25, 0x26, 0x27, 0x28,
    0x29, 0x2A, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3A, 0x43, 0x44, 0x45,
    0x46, 0x47, 0x48, 0x49, 0x4A, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59,
    0x5A, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68, 0x69, 0x6A, 0x73, 0x74, 0x75,
    0x76, 0x77, 0x78, 0x79, 0x7A, 0x83, 0x84, 0x85, 0x86, 0x87, 0x88, 0x89,
    0x8A, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9A, 0xA2, 0xA3,
    0xA4, 0xA5, 0xA6, 0xA7, 0xA8, 0xA9, 0xAA, 0xB2, 0xB3, 0xB4, 0xB5, 0xB6,
    0xB7, 0xB8, 0xB9, 0xBA, 0xC2, 0xC3, 0xC4, 0xC5, 0xC6, 0xC7, 0xC8, 0xC9,
    0xCA, 0xD2, 0xD3, 0xD4, 0xD5, 0xD6, 0xD7, 0xD8, 0xD9, 0xDA, 0xE1, 0xE2,
    0xE3, 0xE4, 0xE5, 0xE6, 0xE7, 0xE8, 0xE9, 0xEA, 0xF1, 0xF2, 0xF3, 0xF4,
    0xF5, 0xF6, 0xF7, 0xF8, 0xF9, 0xFA, 0xFF, 0xDA, 0x00, 0x08, 0x01, 0x01,
    0x00, 0x00, 0x3F, 0x00, 0xFB, 0xD5, 0xDB, 0x20, 0xA8, 0xA8, 0xA2, 0x80,
    0x0F, 0xFF, 0xD9
];

use crate::services::process_util::{configure_hidden_window, kill_and_wait};

/// MJPEG frame boundary marker
const MJPEG_BOUNDARY: &str = "frame";

/// Default preview settings
const DEFAULT_PREVIEW_WIDTH: u32 = 640;
const DEFAULT_PREVIEW_HEIGHT: u32 = 360;
const DEFAULT_PREVIEW_FPS: u32 = 15;
const DEFAULT_PREVIEW_QUALITY: u32 = 5;

/// Maximum concurrent source previews (LRU eviction)
/// Increased from 5 to 12 to support Studio Mode + Multiview without thrashing
const MAX_SOURCE_PREVIEWS: usize = 12;

/// Cleanup timeout for orphaned previews (seconds)
const ORPHAN_TIMEOUT_SECS: u64 = 30;

/// Maximum time without receiving a frame before reader thread exits (seconds)
const READER_STALL_TIMEOUT_SECS: u64 = 30;

/// Maximum age for a cached frame before considered stale (seconds)
const STALE_FRAME_THRESHOLD_SECS: u64 = 5;

/// Tracks a running preview process with cached latest frame
struct PreviewProcess {
    child: Child,
    last_accessed: Instant,
    /// Cached latest frame for snapshot requests
    latest_frame: Arc<Mutex<Option<Bytes>>>,
    /// When the last frame was received (for staleness detection)
    last_frame_time: Arc<Mutex<Instant>>,
    /// Whether the reader thread is still alive
    is_alive: Arc<AtomicBool>,
    /// Handle to the MJPEG reader thread (joined on stop for clean shutdown)
    reader_handle: Option<std::thread::JoinHandle<()>>,
    /// Handle to the stderr reader thread (joined on stop for clean shutdown)
    stderr_handle: Option<std::thread::JoinHandle<()>>,
}

/// Preview parameters from HTTP query
#[derive(Debug, Clone)]
pub struct PreviewParams {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub quality: u32,
}

impl Default for PreviewParams {
    fn default() -> Self {
        Self {
            width: DEFAULT_PREVIEW_WIDTH,
            height: DEFAULT_PREVIEW_HEIGHT,
            fps: DEFAULT_PREVIEW_FPS,
            quality: DEFAULT_PREVIEW_QUALITY,
        }
    }
}

/// Manages FFmpeg preview processes for scene and source previews
pub struct PreviewHandler {
    ffmpeg_path: String,
    scene_preview: Arc<Mutex<Option<PreviewProcess>>>,
    source_previews: Arc<DashMap<String, PreviewProcess>>,
    /// When true, previews should throttle frame processing (app in background)
    idle_flag: Arc<AtomicBool>,
}

impl PreviewHandler {
    /// Create a new preview handler with the given FFmpeg path
    pub fn new(ffmpeg_path: String) -> Self {
        Self {
            ffmpeg_path,
            scene_preview: Arc::new(Mutex::new(None)),
            source_previews: Arc::new(DashMap::new()),
            idle_flag: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Set the idle flag (shared with NativePreviewService)
    pub fn set_idle_flag(&self, flag: Arc<AtomicBool>) {
        self.idle_flag.store(flag.load(Ordering::Relaxed), Ordering::Relaxed);
    }

    /// Set idle mode
    pub fn set_idle(&self, idle: bool) {
        self.idle_flag.store(idle, Ordering::Relaxed);
    }

    /// Build FFmpeg input args for a source (delegates to shared ffmpeg_source_args module)
    fn build_source_input_args(&self, source: &Source) -> Result<Vec<String>, String> {
        super::ffmpeg_source_args::source_input_args(source, &super::ffmpeg_source_args::PREVIEW_OPTS)
    }

    /// Build FFmpeg args for MJPEG output
    fn build_mjpeg_output_args(&self, params: &PreviewParams) -> Vec<String> {
        vec![
            // Video filter for scaling
            "-vf".to_string(),
            format!("scale={}:{}", params.width, params.height),
            // MJPEG output codec
            "-c:v".to_string(),
            "mjpeg".to_string(),
            // Quality (1-31, lower is better)
            "-q:v".to_string(),
            params.quality.to_string(),
            // Frame rate
            "-r".to_string(),
            params.fps.to_string(),
            // Disable audio
            "-an".to_string(),
            // Output format: motion JPEG with multipart boundary
            "-f".to_string(),
            "mpjpeg".to_string(),
            "-boundary_tag".to_string(),
            MJPEG_BOUNDARY.to_string(),
            // Output to stdout
            "pipe:1".to_string(),
        ]
    }

    /// Start an MJPEG preview stream for a source
    /// Returns a broadcast receiver for the MJPEG frames
    pub fn start_source_preview(
        &self,
        source: &Source,
        params: PreviewParams,
    ) -> Result<broadcast::Receiver<Bytes>, String> {
        let source_id = source.id().to_string();

        // Check if preview is already running
        {
            if let Some(mut preview) = self.source_previews.get_mut(&source_id) {
                preview.last_accessed = Instant::now();
                // Preview already running - we need to create a new broadcast for this request
            }

            // Enforce max source previews (LRU eviction)
            if self.source_previews.len() >= MAX_SOURCE_PREVIEWS && !self.source_previews.contains_key(&source_id) {
                // Find oldest preview - collect key to avoid holding iterator across remove
                let oldest = self.source_previews.iter()
                    .min_by_key(|entry| entry.value().last_accessed)
                    .map(|entry| entry.key().clone());

                if let Some(oldest_id) = oldest {
                    if let Some((_, mut old_preview)) = self.source_previews.remove(&oldest_id) {
                        kill_and_wait(&mut old_preview.child);
                        Self::join_preview_threads(&mut old_preview);
                        log::info!("Evicted old preview for source: {}", oldest_id);
                    }
                }
            }
        }

        // Build FFmpeg command
        let mut input_args = self.build_source_input_args(source)?;
        let output_args = self.build_mjpeg_output_args(&params);

        let mut args = Vec::new();
        args.push("-hide_banner".to_string());
        args.push("-loglevel".to_string());
        args.push("warning".to_string());
        args.append(&mut input_args);
        args.extend(output_args);

        log::info!("Starting preview for source {}: {} {}",
            source_id, self.ffmpeg_path, args.join(" "));

        // Verify FFmpeg exists
        if !std::path::Path::new(&self.ffmpeg_path).exists() {
            return Err(format!("FFmpeg not found at path: {}", self.ffmpeg_path));
        }

        // Spawn FFmpeg process
        let mut cmd = Command::new(&self.ffmpeg_path);
        cmd.args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        configure_hidden_window(&mut cmd);

        let mut child = cmd.spawn()
            .map_err(|e| format!("Failed to start FFmpeg preview: {} (path: {})", e, self.ffmpeg_path))?;

        log::info!("FFmpeg preview process started with PID: {}", child.id());

        let stdout = child.stdout.take()
            .ok_or_else(|| "Failed to capture FFmpeg stdout".to_string())?;

        // Create broadcast channel for frames
        let (tx, rx) = broadcast::channel::<Bytes>(16);

        // Create shared cache for latest frame (used by snapshot endpoint)
        let latest_frame: Arc<Mutex<Option<Bytes>>> = Arc::new(Mutex::new(None));
        let latest_frame_clone = Arc::clone(&latest_frame);

        // Create liveness tracking for the reader thread
        let last_frame_time: Arc<Mutex<Instant>> = Arc::new(Mutex::new(Instant::now()));
        let last_frame_time_clone = Arc::clone(&last_frame_time);
        let is_alive: Arc<AtomicBool> = Arc::new(AtomicBool::new(true));
        let is_alive_clone = Arc::clone(&is_alive);

        // Spawn reader thread
        let source_id_clone = source_id.clone();
        let idle_flag_clone = Arc::clone(&self.idle_flag);
        let reader_handle = std::thread::spawn(move || {
            Self::read_mjpeg_stream(
                stdout, tx, latest_frame_clone, last_frame_time_clone,
                is_alive_clone, source_id_clone, idle_flag_clone
            );
        });

        // Log stderr in background - capture all output for debugging
        let stderr_handle = if let Some(stderr) = child.stderr.take() {
            let source_id_log = source_id.clone();
            Some(std::thread::spawn(move || {
                let reader = BufReader::new(stderr);
                for line in reader.lines().map_while(Result::ok) {
                    // Log all stderr output at debug level, errors at warn
                    if line.contains("error") || line.contains("Error") || line.contains("Invalid") || line.contains("not found") {
                        log::warn!("[Preview:{}] {}", source_id_log, line);
                    } else if !line.trim().is_empty() {
                        log::debug!("[Preview:{}] {}", source_id_log, line);
                    }
                }
                log::debug!("[Preview:{}] stderr reader finished", source_id_log);
            }))
        } else {
            None
        };

        // Store the process with frame cache and thread handles
        self.source_previews.insert(source_id, PreviewProcess {
            child,
            last_accessed: Instant::now(),
            latest_frame,
            last_frame_time,
            is_alive,
            reader_handle: Some(reader_handle),
            stderr_handle,
        });

        Ok(rx)
    }

    /// Read MJPEG frames from FFmpeg stdout, broadcast them, and cache the latest.
    ///
    /// When no subscribers are listening, the encoder enters a throttled idle state:
    /// - After 30 consecutive no-subscriber frames (~2s at 15fps): sleep 500ms between reads
    /// - After 60s total idle: exit the loop and mark the reader as dead
    /// This prevents orphaned FFmpeg MJPEG processes from consuming 5-15% CPU each.
    fn read_mjpeg_stream(
        mut stdout: std::process::ChildStdout,
        tx: broadcast::Sender<Bytes>,
        latest_frame: Arc<Mutex<Option<Bytes>>>,
        last_frame_time: Arc<Mutex<Instant>>,
        is_alive: Arc<AtomicBool>,
        source_id: String,
        idle_flag: Arc<AtomicBool>,
    ) {
        let mut buffer = Vec::with_capacity(64 * 1024);
        let mut temp = [0u8; 8192];
        let boundary = format!("--{}", MJPEG_BOUNDARY);
        let boundary_bytes = boundary.as_bytes();
        let mut frame_count = 0u64;
        let mut total_bytes = 0usize;
        let mut idle_skip_counter = 0u32;

        // No-subscriber tracking: throttle then stop orphaned encoders
        let mut no_subscriber_count: u32 = 0;
        let mut idle_since: Option<Instant> = None;
        const NO_SUB_THROTTLE_THRESHOLD: u32 = 30;  // ~2s at 15fps
        const NO_SUB_MAX_IDLE_SECS: u64 = 60;       // Stop after 60s with no subscribers

        log::debug!("[Preview:{}] Starting MJPEG reader, looking for boundary: {:?}", source_id, boundary);

        loop {
            // Check for stall: if no frame received for READER_STALL_TIMEOUT_SECS, exit
            // This prevents orphaned reader threads when FFmpeg stalls or produces no output
            {
                let elapsed = last_frame_time.lock().elapsed();
                if elapsed > Duration::from_secs(READER_STALL_TIMEOUT_SECS) {
                    log::warn!(
                        "[Preview:{}] No frames for {}s, stopping stale preview reader",
                        source_id, elapsed.as_secs()
                    );
                    break;
                }
            }

            // Check if we've been idle (no subscribers) for too long
            if let Some(since) = idle_since {
                if since.elapsed() > Duration::from_secs(NO_SUB_MAX_IDLE_SECS) {
                    log::info!(
                        "[Preview:{}] No subscribers for {}s, stopping orphaned encoder",
                        source_id, NO_SUB_MAX_IDLE_SECS
                    );
                    break;
                }
            }

            // Throttle reads when no subscribers: sleep to let FFmpeg back-pressure naturally
            if no_subscriber_count >= NO_SUB_THROTTLE_THRESHOLD {
                std::thread::sleep(Duration::from_millis(500));
            }

            match stdout.read(&mut temp) {
                Ok(0) => {
                    // EOF
                    log::info!("[Preview:{}] Stream ended after {} frames, {} bytes total",
                        source_id, frame_count, total_bytes);
                    break;
                }
                Ok(n) => {
                    total_bytes += n;
                    buffer.extend_from_slice(&temp[..n]);

                    // Look for complete frames (boundary to boundary)
                    while let Some(frame) = extract_jpeg_frame(&mut buffer, boundary_bytes) {
                        frame_count += 1;

                        // When idle (UI hidden), skip 3 out of 4 frames to save CPU
                        if idle_flag.load(Ordering::Relaxed) {
                            idle_skip_counter += 1;
                            if idle_skip_counter % 4 != 0 {
                                continue;
                            }
                        } else {
                            idle_skip_counter = 0;
                        }

                        // Validate JPEG magic bytes (FF D8 FF)
                        let is_valid_jpeg = frame.len() >= 3
                            && frame[0] == 0xFF
                            && frame[1] == 0xD8
                            && frame[2] == 0xFF;

                        if frame_count <= 5 || frame_count % 100 == 0 {
                            log::info!("[Preview:{}] Frame {} ({} bytes, valid_jpeg={})",
                                source_id, frame_count, frame.len(), is_valid_jpeg);
                        }

                        if !is_valid_jpeg && frame_count <= 3 {
                            // Log first few bytes for debugging
                            let preview: Vec<u8> = frame.iter().take(16).copied().collect();
                            log::warn!("[Preview:{}] Frame {} not valid JPEG. First 16 bytes: {:02X?}",
                                source_id, frame_count, preview);
                        }

                        let frame_bytes = Bytes::from(frame);

                        // Cache the latest frame and update timestamp for staleness detection
                        {
                            let mut cached = latest_frame.lock();
                            *cached = Some(frame_bytes.clone());
                        }
                        {
                            let mut time = last_frame_time.lock();
                            *time = Instant::now();
                        }

                        match tx.send(frame_bytes) {
                            Ok(receiver_count) => {
                                if frame_count == 1 {
                                    log::info!("[Preview:{}] First frame broadcast to {} receivers",
                                        source_id, receiver_count);
                                }
                                // Reset no-subscriber tracking on successful delivery
                                no_subscriber_count = 0;
                                idle_since = None;
                            }
                            Err(_) => {
                                // No receivers — track consecutive no-subscriber frames
                                no_subscriber_count += 1;
                                if idle_since.is_none() {
                                    idle_since = Some(Instant::now());
                                }
                                if no_subscriber_count == NO_SUB_THROTTLE_THRESHOLD {
                                    log::info!(
                                        "[Preview:{}] No subscribers for ~2s, throttling reads",
                                        source_id
                                    );
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    log::warn!("[Preview:{}] Read error after {} frames: {}", source_id, frame_count, e);
                    break;
                }
            }
        }

        // Mark reader as dead so snapshot requests know to restart preview
        is_alive.store(false, Ordering::SeqCst);
        log::info!("[Preview:{}] Reader thread exiting, marked as not alive", source_id);
    }

    /// Extract a complete JPEG frame from the buffer

    /// Get cached frame from a running preview (if available)
    /// Returns None if the preview is dead or frame is stale
    pub fn get_cached_frame(&self, source_id: &str) -> Option<Vec<u8>> {
        let preview = self.source_previews.get(source_id)?;

        // Check if the reader thread is still alive
        if !preview.value().is_alive.load(Ordering::SeqCst) {
            log::debug!("[Preview:{}] Reader thread is dead, returning None", source_id);
            return None;
        }

        // Check if the frame is stale (no new frames for too long)
        {
            let last_time = preview.value().last_frame_time.lock();
            let age = last_time.elapsed();
            if age.as_secs() > STALE_FRAME_THRESHOLD_SECS {
                log::debug!("[Preview:{}] Cached frame is stale ({:.1}s old), returning None",
                    source_id, age.as_secs_f32());
                return None;
            }
        }

        let frame = preview.value().latest_frame.lock();
        frame.as_ref().map(|b| b.to_vec())
    }

    /// Check if a preview is running for a source (and actually alive)
    pub fn is_preview_running(&self, source_id: &str) -> bool {
        self.source_previews.get(source_id)
            .map(|preview| preview.value().is_alive.load(Ordering::SeqCst))
            .unwrap_or(false)
    }

    /// Clean up dead preview processes from the hashmap
    pub fn cleanup_dead_preview(&self, source_id: &str) {
        if let Some(preview) = self.source_previews.get(source_id) {
            if !preview.value().is_alive.load(Ordering::SeqCst) {
                log::info!("[Preview:{}] Removing dead preview from cache", source_id);
                drop(preview); // Drop the reference before removing
                self.source_previews.remove(source_id);
            }
        }
    }

    /// Capture a single JPEG snapshot from a source
    /// First tries to get a cached frame from a running preview.
    /// If no preview is running, starts one and waits for the first frame.
    /// Falls back to spawning a one-shot FFmpeg process if preview fails.
    pub async fn capture_snapshot(
        &self,
        source: &Source,
        params: &PreviewParams,
    ) -> Result<Vec<u8>, String> {
        let source_id = source.id().to_string();

        // First, try to get a cached frame from a running preview
        // This is much faster and doesn't require spawning a new process
        if let Some(cached_frame) = self.get_cached_frame(&source_id) {
            log::debug!("Returning cached frame for source {} ({} bytes)", source_id, cached_frame.len());
            return Ok(cached_frame);
        }

        // No cached frame - clean up dead preview if exists, then start a new one
        if !self.is_preview_running(&source_id) {
            // Clean up any dead preview entry before starting a new one
            self.cleanup_dead_preview(&source_id);
            log::info!("Starting persistent preview for source {} (triggered by snapshot request)", source_id);

            // Start the preview with high quality params for caching
            // Use at least 720p for good quality, 15fps for smooth preview
            let preview_params = PreviewParams {
                width: params.width.max(1280),
                height: params.height.max(720),
                fps: 15,
                quality: params.quality.min(3),  // Ensure good quality (lower = better)
            };

            match self.start_source_preview(source, preview_params) {
                Ok(_rx) => {
                    // Preview started - wait briefly for first frame
                    for _ in 0..30 {  // Wait up to 3 seconds (30 * 100ms)
                        tokio::time::sleep(Duration::from_millis(100)).await;
                        if let Some(cached_frame) = self.get_cached_frame(&source_id) {
                            log::debug!("Got first cached frame for source {} ({} bytes)", source_id, cached_frame.len());
                            return Ok(cached_frame);
                        }
                    }
                    log::warn!("Preview started but no frame received within 3 seconds for source {}", source_id);
                }
                Err(e) => {
                    log::warn!("Failed to start preview for source {}: {}", source_id, e);
                }
            }
        }

        // Fall back to spawning a one-shot FFmpeg process
        log::debug!("Falling back to one-shot FFmpeg for source {}", source_id);
        let ffmpeg_path = self.ffmpeg_path.clone();

        // Build FFmpeg command for single frame capture
        let mut input_args = self.build_source_input_args(source)?;

        let mut args = Vec::new();
        args.push("-hide_banner".to_string());
        args.push("-loglevel".to_string());
        args.push("error".to_string());
        args.append(&mut input_args);

        // Output args for single JPEG frame
        args.extend([
            "-vf".to_string(),
            format!("scale={}:{}", params.width, params.height),
            "-vframes".to_string(),
            "1".to_string(),
            "-q:v".to_string(),
            params.quality.to_string(),
            "-f".to_string(),
            "image2".to_string(),
            "-c:v".to_string(),
            "mjpeg".to_string(),
            "pipe:1".to_string(),
        ]);

        log::debug!("Capturing snapshot for source {}: {} {}",
            source_id, ffmpeg_path, args.join(" "));

        // Use tokio::process::Command for async execution with timeout
        let capture_future = async {
            let mut cmd = tokio::process::Command::new(&ffmpeg_path);
            cmd.args(&args)
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());

            crate::services::process_util::configure_hidden_window_tokio(&mut cmd);

            // kill_on_drop ensures the process is killed if the future is dropped (e.g., on timeout)
            cmd.kill_on_drop(true);

            let output = cmd.output()
                .await
                .map_err(|e| format!("Failed to run FFmpeg snapshot: {}", e))?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(format!("FFmpeg snapshot failed: {}", stderr.trim()));
            }

            Ok::<Vec<u8>, String>(output.stdout)
        };

        // Apply timeout to prevent indefinite blocking
        let jpeg_data = match timeout(
            Duration::from_secs(SNAPSHOT_TIMEOUT_SECS),
            capture_future
        ).await {
            Ok(result) => result?,
            Err(_) => {
                log::warn!("Snapshot capture timed out after {}s for source {}",
                    SNAPSHOT_TIMEOUT_SECS, source_id);
                return Err(format!(
                    "Snapshot capture timed out after {} seconds. Device may be unavailable or permission denied.",
                    SNAPSHOT_TIMEOUT_SECS
                ));
            }
        };

        // Validate JPEG magic bytes
        if jpeg_data.len() < 3 || jpeg_data[0] != 0xFF || jpeg_data[1] != 0xD8 {
            return Err("Invalid JPEG data from FFmpeg".to_string());
        }

        log::debug!("Captured snapshot for source {}: {} bytes", source_id, jpeg_data.len());
        Ok(jpeg_data)
    }

    /// Join thread handles after killing the child process.
    /// Killing the child closes pipes, causing reader threads to exit naturally.
    fn join_preview_threads(preview: &mut PreviewProcess) {
        if let Some(h) = preview.reader_handle.take() {
            let _ = h.join();
        }
        if let Some(h) = preview.stderr_handle.take() {
            let _ = h.join();
        }
    }

    /// Stop a source preview
    pub fn stop_source_preview(&self, source_id: &str) {
        if let Some((_, mut preview)) = self.source_previews.remove(source_id) {
            kill_and_wait(&mut preview.child);
            Self::join_preview_threads(&mut preview);
            log::info!("Stopped preview for source: {}", source_id);
        }
    }

    /// Start a composed scene preview using Compositor service
    /// Returns a broadcast receiver for the MJPEG frames
    pub fn start_scene_preview(
        &self,
        scene: &Scene,
        sources: &[Source],
        params: PreviewParams,
    ) -> Result<broadcast::Receiver<Bytes>, String> {
        let scene_id = scene.id.clone();

        // Stop any existing scene preview
        self.stop_scene_preview();

        // Build FFmpeg command using Compositor
        // 1. Build input args for all sources used in the scene
        let input_args = Compositor::build_input_args(scene, sources);

        if input_args.is_empty() {
            return Err("No sources configured for scene".to_string());
        }

        // 2. Build video filter_complex for compositing (video only for preview)
        let video_filter = Compositor::build_video_filter(scene, sources);

        // 3. Build MJPEG output args
        let mjpeg_output_args = vec![
            // Map the composed video output
            "-map".to_string(), "[vout]".to_string(),
            // Scale to preview size
            "-vf".to_string(), format!("scale={}:{}", params.width, params.height),
            // MJPEG output codec
            "-c:v".to_string(), "mjpeg".to_string(),
            // Quality (1-31, lower is better)
            "-q:v".to_string(), params.quality.to_string(),
            // Frame rate
            "-r".to_string(), params.fps.to_string(),
            // Disable audio for preview
            "-an".to_string(),
            // Output format: motion JPEG with multipart boundary
            "-f".to_string(), "mpjpeg".to_string(),
            "-boundary_tag".to_string(), MJPEG_BOUNDARY.to_string(),
            // Output to stdout
            "pipe:1".to_string(),
        ];

        // Assemble full command
        let mut args = Vec::new();
        args.push("-hide_banner".to_string());
        args.push("-loglevel".to_string());
        args.push("warning".to_string());
        args.extend(input_args);
        args.extend(["-filter_complex".to_string(), video_filter]);
        args.extend(mjpeg_output_args);

        log::info!("Starting scene preview for {}: {} {}",
            scene_id, self.ffmpeg_path, args.join(" "));

        // Verify FFmpeg exists
        if !std::path::Path::new(&self.ffmpeg_path).exists() {
            return Err(format!("FFmpeg not found at path: {}", self.ffmpeg_path));
        }

        // Spawn FFmpeg process
        let mut cmd = Command::new(&self.ffmpeg_path);
        cmd.args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        configure_hidden_window(&mut cmd);

        let mut child = cmd.spawn()
            .map_err(|e| format!("Failed to start FFmpeg scene preview: {} (path: {})", e, self.ffmpeg_path))?;

        log::info!("FFmpeg scene preview process started with PID: {}", child.id());

        let stdout = child.stdout.take()
            .ok_or_else(|| "Failed to capture FFmpeg stdout".to_string())?;

        // Create broadcast channel for frames
        let (tx, rx) = broadcast::channel::<Bytes>(16);

        // Create shared cache for latest frame and liveness tracking
        let latest_frame: Arc<Mutex<Option<Bytes>>> = Arc::new(Mutex::new(None));
        let latest_frame_clone = Arc::clone(&latest_frame);
        let last_frame_time: Arc<Mutex<Instant>> = Arc::new(Mutex::new(Instant::now()));
        let last_frame_time_clone = Arc::clone(&last_frame_time);
        let is_alive: Arc<AtomicBool> = Arc::new(AtomicBool::new(true));
        let is_alive_clone = Arc::clone(&is_alive);

        // Spawn reader thread
        let scene_id_clone = scene_id.clone();
        let idle_flag_clone = Arc::clone(&self.idle_flag);
        let reader_handle = std::thread::spawn(move || {
            Self::read_mjpeg_stream(
                stdout,
                tx,
                latest_frame_clone,
                last_frame_time_clone,
                is_alive_clone,
                format!("scene:{}", scene_id_clone),
                idle_flag_clone,
            );
        });

        // Log stderr in background
        let stderr_handle = if let Some(stderr) = child.stderr.take() {
            let scene_id_log = scene_id.clone();
            Some(std::thread::spawn(move || {
                let reader = BufReader::new(stderr);
                for line in reader.lines().map_while(Result::ok) {
                    if line.contains("error") || line.contains("Error") || line.contains("Invalid") {
                        log::warn!("[ScenePreview:{}] {}", scene_id_log, line);
                    } else if !line.trim().is_empty() {
                        log::debug!("[ScenePreview:{}] {}", scene_id_log, line);
                    }
                }
                log::debug!("[ScenePreview:{}] stderr reader finished", scene_id_log);
            }))
        } else {
            None
        };

        // Store the process with frame cache and thread handles
        {
            let mut scene_preview = self.scene_preview.lock();

            *scene_preview = Some(PreviewProcess {
                child,
                last_accessed: Instant::now(),
                latest_frame,
                last_frame_time,
                is_alive,
                reader_handle: Some(reader_handle),
                stderr_handle,
            });
        }

        Ok(rx)
    }

    /// Get cached frame from the scene preview (if available)
    /// Returns None if the preview is dead or frame is stale
    pub fn get_scene_cached_frame(&self) -> Option<Vec<u8>> {
        let preview = self.scene_preview.lock();
        let proc = preview.as_ref()?;

        // Check if the reader thread is still alive
        if !proc.is_alive.load(Ordering::SeqCst) {
            log::debug!("[ScenePreview] Reader thread is dead, returning None");
            return None;
        }

        // Check if the frame is stale (no new frames for too long)
        {
            let last_time = proc.last_frame_time.lock();
            let age = last_time.elapsed();
            if age.as_secs() > STALE_FRAME_THRESHOLD_SECS {
                log::debug!("[ScenePreview] Cached frame is stale ({:.1}s old), returning None",
                    age.as_secs_f32());
                return None;
            }
        }

        let frame = proc.latest_frame.lock();
        frame.as_ref().map(|b| b.to_vec())
    }

    /// Check if scene preview is running (and actually alive)
    pub fn is_scene_preview_running(&self) -> bool {
        let p = self.scene_preview.lock();
        p.as_ref()
            .map(|proc| proc.is_alive.load(Ordering::SeqCst))
            .unwrap_or(false)
    }

    /// Clean up dead scene preview
    pub fn cleanup_dead_scene_preview(&self) {
        let mut preview = self.scene_preview.lock();
        if let Some(proc) = preview.as_ref() {
            if !proc.is_alive.load(Ordering::SeqCst) {
                log::info!("[ScenePreview] Removing dead scene preview from cache");
                *preview = None;
            }
        }
    }

    /// Capture a scene snapshot - tries cached frame first, else starts preview
    pub async fn capture_scene_snapshot(
        &self,
        scene: &Scene,
        sources: &[Source],
        params: &PreviewParams,
    ) -> Result<Vec<u8>, String> {
        // First, try to get a cached frame from a running scene preview
        if let Some(cached_frame) = self.get_scene_cached_frame() {
            log::debug!("Returning cached scene frame ({} bytes)", cached_frame.len());
            return Ok(cached_frame);
        }

        // No cached frame - clean up dead preview if exists, then start a new one
        if !self.is_scene_preview_running() {
            // Clean up any dead scene preview entry before starting a new one
            self.cleanup_dead_scene_preview();
            log::info!("Starting persistent scene preview (triggered by snapshot request)");

            // Start the preview with good quality params for caching
            let preview_params = PreviewParams {
                width: params.width.max(1280),
                height: params.height.max(720),
                fps: 15,
                quality: params.quality.min(3),
            };

            match self.start_scene_preview(scene, sources, preview_params) {
                Ok(_rx) => {
                    // Preview started - wait briefly for first frame
                    for _ in 0..30 { // Wait up to 3 seconds (30 * 100ms)
                        tokio::time::sleep(Duration::from_millis(100)).await;
                        if let Some(cached_frame) = self.get_scene_cached_frame() {
                            log::debug!("Got first cached scene frame ({} bytes)", cached_frame.len());
                            return Ok(cached_frame);
                        }
                    }
                    log::warn!("Scene preview started but no frame received within 3 seconds");
                }
                Err(e) => {
                    // Return placeholder for scenes with no sources configured
                    log::debug!("Scene preview not available ({}), returning placeholder", e);
                    return Ok(PLACEHOLDER_JPEG.to_vec());
                }
            }
        }

        // Return placeholder if we couldn't get a frame
        log::debug!("Scene snapshot fallback to placeholder");
        Ok(PLACEHOLDER_JPEG.to_vec())
    }

    /// Stop the scene preview
    pub fn stop_scene_preview(&self) {
        let mut scene = self.scene_preview.lock();
        if let Some(mut preview) = scene.take() {
            kill_and_wait(&mut preview.child);
            Self::join_preview_threads(&mut preview);
            log::info!("Stopped scene preview");
        }
    }

    /// Stop all previews
    pub fn stop_all_previews(&self) {
        // Stop scene preview
        {
            let mut scene = self.scene_preview.lock();
            if let Some(mut preview) = scene.take() {
                kill_and_wait(&mut preview.child);
                Self::join_preview_threads(&mut preview);
            }
        }

        // Stop all source previews
        // DashMap doesn't have drain, so collect keys first, then remove individually
        let keys: Vec<String> = self.source_previews.iter()
            .map(|entry| entry.key().clone())
            .collect();

        for id in keys {
            if let Some((_, mut preview)) = self.source_previews.remove(&id) {
                kill_and_wait(&mut preview.child);
                Self::join_preview_threads(&mut preview);
                log::info!("Stopped preview for source: {}", id);
            }
        }
    }

    /// Cleanup orphaned previews (not accessed in ORPHAN_TIMEOUT_SECS)
    pub fn cleanup_orphaned_previews(&self) {
        let timeout = Duration::from_secs(ORPHAN_TIMEOUT_SECS);
        let now = Instant::now();

        // Collect orphan keys first, then remove them to avoid holding iterator across remove
        let orphans: Vec<String> = self.source_previews.iter()
            .filter(|entry| now.duration_since(entry.value().last_accessed) > timeout)
            .map(|entry| entry.key().clone())
            .collect();

        for id in orphans {
            if let Some((_, mut preview)) = self.source_previews.remove(&id) {
                kill_and_wait(&mut preview.child);
                Self::join_preview_threads(&mut preview);
                log::info!("Cleaned up orphaned preview: {}", id);
            }
        }
    }

    /// Get active preview count
    pub fn active_preview_count(&self) -> usize {
        let s = self.scene_preview.lock();
        let scene_count = if s.is_some() { 1 } else { 0 };

        let source_count = self.source_previews.len();

        scene_count + source_count
    }
}

impl Drop for PreviewHandler {
    fn drop(&mut self) {
        self.stop_all_previews();
    }
}
