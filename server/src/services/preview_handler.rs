// Preview Handler Service
// Manages FFmpeg processes for MJPEG preview streams

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::broadcast;
use tokio::time::timeout;
use bytes::Bytes;

use crate::models::{Scene, Source};
use crate::services::Compositor;

/// Timeout for snapshot capture (prevents indefinite blocking)
const SNAPSHOT_TIMEOUT_SECS: u64 = 10;

// Windows: Hide console windows for spawned processes
#[cfg(windows)]
use std::os::windows::process::CommandExt;
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

/// MJPEG frame boundary marker
const MJPEG_BOUNDARY: &str = "frame";

/// Default preview settings
const DEFAULT_PREVIEW_WIDTH: u32 = 640;
const DEFAULT_PREVIEW_HEIGHT: u32 = 360;
const DEFAULT_PREVIEW_FPS: u32 = 15;
const DEFAULT_PREVIEW_QUALITY: u32 = 5;

/// Maximum concurrent source previews (LRU eviction)
const MAX_SOURCE_PREVIEWS: usize = 5;

/// Cleanup timeout for orphaned previews (seconds)
const ORPHAN_TIMEOUT_SECS: u64 = 30;

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
    source_previews: Arc<Mutex<HashMap<String, PreviewProcess>>>,
}

impl PreviewHandler {
    /// Create a new preview handler with the given FFmpeg path
    pub fn new(ffmpeg_path: String) -> Self {
        Self {
            ffmpeg_path,
            scene_preview: Arc::new(Mutex::new(None)),
            source_previews: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Build FFmpeg input args for a source
    fn build_source_input_args(&self, source: &Source) -> Result<Vec<String>, String> {
        match source {
            Source::Camera(cam) => {
                // Validate device ID is not empty
                if cam.device_id.is_empty() {
                    return Err("Camera device not selected".to_string());
                }

                let mut args = Vec::new();

                // Platform-specific capture
                #[cfg(target_os = "macos")]
                {
                    args.extend([
                        "-f".to_string(),
                        "avfoundation".to_string(),
                        "-framerate".to_string(),
                        cam.fps.unwrap_or(30).to_string(),
                        "-pixel_format".to_string(),
                        "uyvy422".to_string(),
                    ]);

                    if let (Some(w), Some(h)) = (cam.width, cam.height) {
                        args.extend([
                            "-video_size".to_string(),
                            format!("{}x{}", w, h),
                        ]);
                    }

                    // AVFoundation uses "VIDEO_INDEX:AUDIO_INDEX" format
                    // Use "INDEX:none" for video-only (no audio)
                    let device_input = if cam.device_id.contains(':') {
                        cam.device_id.clone()
                    } else {
                        format!("{}:none", cam.device_id)
                    };
                    args.extend([
                        "-i".to_string(),
                        device_input,
                    ]);
                }

                #[cfg(target_os = "windows")]
                {
                    args.extend([
                        "-f".to_string(),
                        "dshow".to_string(),
                    ]);

                    if let Some(fps) = cam.fps {
                        args.extend(["-framerate".to_string(), fps.to_string()]);
                    }

                    if let (Some(w), Some(h)) = (cam.width, cam.height) {
                        args.extend(["-video_size".to_string(), format!("{}x{}", w, h)]);
                    }

                    args.extend([
                        "-i".to_string(),
                        format!("video={}", cam.device_id),
                    ]);
                }

                #[cfg(target_os = "linux")]
                {
                    args.extend([
                        "-f".to_string(),
                        "v4l2".to_string(),
                    ]);

                    if let Some(fps) = cam.fps {
                        args.extend(["-framerate".to_string(), fps.to_string()]);
                    }

                    if let (Some(w), Some(h)) = (cam.width, cam.height) {
                        args.extend(["-video_size".to_string(), format!("{}x{}", w, h)]);
                    }

                    args.extend([
                        "-i".to_string(),
                        cam.device_id.clone(),
                    ]);
                }

                Ok(args)
            }

            Source::ScreenCapture(screen) => {
                // Validate display ID is not empty
                if screen.display_id.is_empty() {
                    return Err("Display not selected".to_string());
                }

                let mut args = Vec::new();

                #[cfg(target_os = "macos")]
                {
                    args.extend([
                        "-f".to_string(),
                        "avfoundation".to_string(),
                        "-framerate".to_string(),
                        screen.fps.to_string(),
                        "-capture_cursor".to_string(),
                        if screen.capture_cursor { "1" } else { "0" }.to_string(),
                        "-pixel_format".to_string(),
                        "uyvy422".to_string(),
                    ]);

                    // AVFoundation screen capture: "VIDEO_INDEX:AUDIO_INDEX" or "VIDEO_INDEX:none"
                    let screen_input = if screen.capture_audio {
                        format!("{}:", screen.display_id)
                    } else {
                        format!("{}:none", screen.display_id)
                    };
                    args.extend([
                        "-i".to_string(),
                        screen_input,
                    ]);
                }

                #[cfg(target_os = "windows")]
                {
                    args.extend([
                        "-f".to_string(),
                        "gdigrab".to_string(),
                        "-framerate".to_string(),
                        screen.fps.to_string(),
                    ]);

                    if screen.capture_cursor {
                        args.extend(["-draw_mouse".to_string(), "1".to_string()]);
                    }

                    args.extend([
                        "-i".to_string(),
                        "desktop".to_string(),
                    ]);
                }

                #[cfg(target_os = "linux")]
                {
                    args.extend([
                        "-f".to_string(),
                        "x11grab".to_string(),
                        "-framerate".to_string(),
                        screen.fps.to_string(),
                    ]);

                    if screen.capture_cursor {
                        args.extend(["-draw_mouse".to_string(), "1".to_string()]);
                    }

                    // X11 display format
                    args.extend([
                        "-i".to_string(),
                        format!(":{}", screen.display_id),
                    ]);
                }

                Ok(args)
            }

            Source::MediaFile(media) => {
                // Validate file path is not empty
                if media.file_path.is_empty() {
                    return Err("Media file path not specified".to_string());
                }

                Ok(vec![
                    "-stream_loop".to_string(),
                    if media.loop_playback { "-1" } else { "0" }.to_string(),
                    "-i".to_string(),
                    media.file_path.clone(),
                ])
            }

            Source::CaptureCard(card) => {
                // Validate device ID is not empty
                if card.device_id.is_empty() {
                    return Err("Capture card device not selected".to_string());
                }

                let mut args = Vec::new();

                #[cfg(target_os = "macos")]
                {
                    args.extend([
                        "-f".to_string(),
                        "avfoundation".to_string(),
                        "-pixel_format".to_string(),
                        "uyvy422".to_string(),
                    ]);

                    // Capture cards may have both video and audio
                    let device_input = if card.device_id.contains(':') {
                        card.device_id.clone()
                    } else {
                        format!("{}:none", card.device_id)
                    };
                    args.extend([
                        "-i".to_string(),
                        device_input,
                    ]);
                }

                #[cfg(target_os = "windows")]
                {
                    args.extend([
                        "-f".to_string(),
                        "dshow".to_string(),
                        "-i".to_string(),
                        format!("video={}:audio={}", card.device_id, card.device_id),
                    ]);
                }

                #[cfg(target_os = "linux")]
                {
                    args.extend([
                        "-f".to_string(),
                        "v4l2".to_string(),
                        "-i".to_string(),
                        card.device_id.clone(),
                    ]);
                }

                Ok(args)
            }

            Source::Rtmp(rtmp) => {
                // Build RTMP input URL from source configuration
                let host = if rtmp.bind_address == "0.0.0.0" {
                    "127.0.0.1"
                } else {
                    &rtmp.bind_address
                };
                let rtmp_url = format!("rtmp://{}:{}/{}", host, rtmp.port, rtmp.application);

                Ok(vec![
                    "-rtmp_live".to_string(),
                    "live".to_string(),
                    "-i".to_string(),
                    rtmp_url,
                ])
            }

            Source::AudioDevice(_) => {
                // Audio-only devices get a placeholder visual
                Ok(vec![
                    "-f".to_string(),
                    "lavfi".to_string(),
                    "-i".to_string(),
                    format!("color=c=darkblue:s={}x{}:d=3600", DEFAULT_PREVIEW_WIDTH, DEFAULT_PREVIEW_HEIGHT),
                ])
            }
        }
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
            let mut previews = self.source_previews.lock()
                .map_err(|e| format!("Lock poisoned: {}", e))?;

            if let Some(preview) = previews.get_mut(&source_id) {
                preview.last_accessed = Instant::now();
                // Preview already running - we need to create a new broadcast for this request
            }

            // Enforce max source previews (LRU eviction)
            if previews.len() >= MAX_SOURCE_PREVIEWS && !previews.contains_key(&source_id) {
                // Find oldest preview
                let oldest = previews.iter()
                    .min_by_key(|(_, p)| p.last_accessed)
                    .map(|(id, _)| id.clone());

                if let Some(oldest_id) = oldest {
                    if let Some(mut old_preview) = previews.remove(&oldest_id) {
                        let _ = old_preview.child.kill();
                        let _ = old_preview.child.wait();
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

        #[cfg(windows)]
        cmd.creation_flags(CREATE_NO_WINDOW);

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
        std::thread::spawn(move || {
            Self::read_mjpeg_stream(
                stdout, tx, latest_frame_clone, last_frame_time_clone,
                is_alive_clone, source_id_clone
            );
        });

        // Log stderr in background - capture all output for debugging
        if let Some(stderr) = child.stderr.take() {
            let source_id_log = source_id.clone();
            std::thread::spawn(move || {
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
            });
        }

        // Store the process with frame cache
        {
            let mut previews = self.source_previews.lock()
                .map_err(|e| format!("Lock poisoned: {}", e))?;

            previews.insert(source_id, PreviewProcess {
                child,
                last_accessed: Instant::now(),
                latest_frame,
                last_frame_time,
                is_alive,
            });
        }

        Ok(rx)
    }

    /// Read MJPEG frames from FFmpeg stdout, broadcast them, and cache the latest
    fn read_mjpeg_stream(
        mut stdout: std::process::ChildStdout,
        tx: broadcast::Sender<Bytes>,
        latest_frame: Arc<Mutex<Option<Bytes>>>,
        last_frame_time: Arc<Mutex<Instant>>,
        is_alive: Arc<AtomicBool>,
        source_id: String,
    ) {
        let mut buffer = Vec::with_capacity(64 * 1024);
        let mut temp = [0u8; 8192];
        let boundary = format!("--{}", MJPEG_BOUNDARY);
        let boundary_bytes = boundary.as_bytes();
        let mut frame_count = 0u64;
        let mut total_bytes = 0usize;

        log::debug!("[Preview:{}] Starting MJPEG reader, looking for boundary: {:?}", source_id, boundary);

        loop {
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
                    while let Some(frame) = Self::extract_jpeg_frame(&mut buffer, boundary_bytes) {
                        frame_count += 1;

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
                        if let Ok(mut cached) = latest_frame.lock() {
                            *cached = Some(frame_bytes.clone());
                        }
                        if let Ok(mut time) = last_frame_time.lock() {
                            *time = Instant::now();
                        }

                        match tx.send(frame_bytes) {
                            Ok(receiver_count) => {
                                if frame_count == 1 {
                                    log::info!("[Preview:{}] First frame broadcast to {} receivers",
                                        source_id, receiver_count);
                                }
                            }
                            Err(_) => {
                                // No receivers, but keep running for snapshot cache
                                // Don't stop - snapshots may still need the cached frame
                                if frame_count == 1 {
                                    log::debug!("[Preview:{}] No broadcast receivers, continuing for snapshot cache",
                                        source_id);
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
    fn extract_jpeg_frame(buffer: &mut Vec<u8>, boundary: &[u8]) -> Option<Vec<u8>> {
        // Find first boundary
        let first = Self::find_subsequence(buffer, boundary)?;

        // Find content type line end (after boundary)
        let header_start = first + boundary.len();
        let header_end = Self::find_subsequence(&buffer[header_start..], b"\r\n\r\n")?;
        let content_start = header_start + header_end + 4;

        // Find next boundary
        let next_boundary = Self::find_subsequence(&buffer[content_start..], boundary)?;
        let content_end = content_start + next_boundary;

        // Check for JPEG markers
        if content_end - content_start < 2 {
            // Not enough data
            buffer.drain(..first + boundary.len());
            return None;
        }

        // Extract JPEG data (trim trailing \r\n before boundary)
        let mut jpeg_end = content_end;
        while jpeg_end > content_start && (buffer[jpeg_end - 1] == b'\n' || buffer[jpeg_end - 1] == b'\r') {
            jpeg_end -= 1;
        }

        let frame = buffer[content_start..jpeg_end].to_vec();
        buffer.drain(..content_end);

        Some(frame)
    }

    /// Find subsequence in slice
    fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
        haystack.windows(needle.len()).position(|w| w == needle)
    }

    /// Get cached frame from a running preview (if available)
    /// Returns None if the preview is dead or frame is stale
    pub fn get_cached_frame(&self, source_id: &str) -> Option<Vec<u8>> {
        let previews = self.source_previews.lock().ok()?;
        let preview = previews.get(source_id)?;

        // Check if the reader thread is still alive
        if !preview.is_alive.load(Ordering::SeqCst) {
            log::debug!("[Preview:{}] Reader thread is dead, returning None", source_id);
            return None;
        }

        // Check if the frame is stale (no new frames for too long)
        if let Ok(last_time) = preview.last_frame_time.lock() {
            let age = last_time.elapsed();
            if age.as_secs() > STALE_FRAME_THRESHOLD_SECS {
                log::debug!("[Preview:{}] Cached frame is stale ({:.1}s old), returning None",
                    source_id, age.as_secs_f32());
                return None;
            }
        }

        let frame = preview.latest_frame.lock().ok()?;
        frame.as_ref().map(|b| b.to_vec())
    }

    /// Check if a preview is running for a source (and actually alive)
    pub fn is_preview_running(&self, source_id: &str) -> bool {
        self.source_previews.lock()
            .map(|p| {
                p.get(source_id)
                    .map(|preview| preview.is_alive.load(Ordering::SeqCst))
                    .unwrap_or(false)
            })
            .unwrap_or(false)
    }

    /// Clean up dead preview processes from the hashmap
    pub fn cleanup_dead_preview(&self, source_id: &str) {
        if let Ok(mut previews) = self.source_previews.lock() {
            if let Some(preview) = previews.get(source_id) {
                if !preview.is_alive.load(Ordering::SeqCst) {
                    log::info!("[Preview:{}] Removing dead preview from cache", source_id);
                    previews.remove(source_id);
                }
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

            #[cfg(windows)]
            cmd.creation_flags(CREATE_NO_WINDOW);

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

    /// Stop a source preview
    pub fn stop_source_preview(&self, source_id: &str) {
        if let Ok(mut previews) = self.source_previews.lock() {
            if let Some(mut preview) = previews.remove(source_id) {
                let _ = preview.child.kill();
                let _ = preview.child.wait();
                log::info!("Stopped preview for source: {}", source_id);
            }
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

        #[cfg(windows)]
        cmd.creation_flags(CREATE_NO_WINDOW);

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
        std::thread::spawn(move || {
            Self::read_mjpeg_stream(
                stdout,
                tx,
                latest_frame_clone,
                last_frame_time_clone,
                is_alive_clone,
                format!("scene:{}", scene_id_clone),
            );
        });

        // Log stderr in background
        if let Some(stderr) = child.stderr.take() {
            let scene_id_log = scene_id.clone();
            std::thread::spawn(move || {
                let reader = BufReader::new(stderr);
                for line in reader.lines().map_while(Result::ok) {
                    if line.contains("error") || line.contains("Error") || line.contains("Invalid") {
                        log::warn!("[ScenePreview:{}] {}", scene_id_log, line);
                    } else if !line.trim().is_empty() {
                        log::debug!("[ScenePreview:{}] {}", scene_id_log, line);
                    }
                }
                log::debug!("[ScenePreview:{}] stderr reader finished", scene_id_log);
            });
        }

        // Store the process with frame cache
        {
            let mut scene_preview = self.scene_preview.lock()
                .map_err(|e| format!("Lock poisoned: {}", e))?;

            *scene_preview = Some(PreviewProcess {
                child,
                last_accessed: Instant::now(),
                latest_frame,
                last_frame_time,
                is_alive,
            });
        }

        Ok(rx)
    }

    /// Get cached frame from the scene preview (if available)
    /// Returns None if the preview is dead or frame is stale
    pub fn get_scene_cached_frame(&self) -> Option<Vec<u8>> {
        let preview = self.scene_preview.lock().ok()?;
        let proc = preview.as_ref()?;

        // Check if the reader thread is still alive
        if !proc.is_alive.load(Ordering::SeqCst) {
            log::debug!("[ScenePreview] Reader thread is dead, returning None");
            return None;
        }

        // Check if the frame is stale (no new frames for too long)
        if let Ok(last_time) = proc.last_frame_time.lock() {
            let age = last_time.elapsed();
            if age.as_secs() > STALE_FRAME_THRESHOLD_SECS {
                log::debug!("[ScenePreview] Cached frame is stale ({:.1}s old), returning None",
                    age.as_secs_f32());
                return None;
            }
        }

        let frame = proc.latest_frame.lock().ok()?;
        frame.as_ref().map(|b| b.to_vec())
    }

    /// Check if scene preview is running (and actually alive)
    pub fn is_scene_preview_running(&self) -> bool {
        self.scene_preview.lock()
            .map(|p| {
                p.as_ref()
                    .map(|proc| proc.is_alive.load(Ordering::SeqCst))
                    .unwrap_or(false)
            })
            .unwrap_or(false)
    }

    /// Clean up dead scene preview
    pub fn cleanup_dead_scene_preview(&self) {
        if let Ok(mut preview) = self.scene_preview.lock() {
            if let Some(proc) = preview.as_ref() {
                if !proc.is_alive.load(Ordering::SeqCst) {
                    log::info!("[ScenePreview] Removing dead scene preview from cache");
                    *preview = None;
                }
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
                    log::warn!("Failed to start scene preview: {}", e);
                    return Err(e);
                }
            }
        }

        Err("Failed to capture scene snapshot".to_string())
    }

    /// Stop the scene preview
    pub fn stop_scene_preview(&self) {
        if let Ok(mut scene) = self.scene_preview.lock() {
            if let Some(mut preview) = scene.take() {
                let _ = preview.child.kill();
                let _ = preview.child.wait();
                log::info!("Stopped scene preview");
            }
        }
    }

    /// Stop all previews
    pub fn stop_all_previews(&self) {
        // Stop scene preview
        if let Ok(mut scene) = self.scene_preview.lock() {
            if let Some(mut preview) = scene.take() {
                let _ = preview.child.kill();
                let _ = preview.child.wait();
            }
        }

        // Stop all source previews
        if let Ok(mut previews) = self.source_previews.lock() {
            for (id, mut preview) in previews.drain() {
                let _ = preview.child.kill();
                let _ = preview.child.wait();
                log::info!("Stopped preview for source: {}", id);
            }
        }
    }

    /// Cleanup orphaned previews (not accessed in ORPHAN_TIMEOUT_SECS)
    pub fn cleanup_orphaned_previews(&self) {
        let timeout = Duration::from_secs(ORPHAN_TIMEOUT_SECS);
        let now = Instant::now();

        if let Ok(mut previews) = self.source_previews.lock() {
            let orphans: Vec<String> = previews.iter()
                .filter(|(_, p)| now.duration_since(p.last_accessed) > timeout)
                .map(|(id, _)| id.clone())
                .collect();

            for id in orphans {
                if let Some(mut preview) = previews.remove(&id) {
                    let _ = preview.child.kill();
                    let _ = preview.child.wait();
                    log::info!("Cleaned up orphaned preview: {}", id);
                }
            }
        }
    }

    /// Get active preview count
    pub fn active_preview_count(&self) -> usize {
        let scene_count = self.scene_preview.lock()
            .map(|s| if s.is_some() { 1 } else { 0 })
            .unwrap_or(0);

        let source_count = self.source_previews.lock()
            .map(|p| p.len())
            .unwrap_or(0);

        scene_count + source_count
    }
}

impl Drop for PreviewHandler {
    fn drop(&mut self) {
        self.stop_all_previews();
    }
}
