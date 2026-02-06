// H264 Capture Service
// Captures screen frames via scap and encodes to H264 using FFmpeg
// Supports two output modes:
// 1. HTTP/MPEG-TS streaming (for go2rtc with #video=copy passthrough)
// 2. RTSP push to go2rtc (alternative low-latency mode)

use std::collections::HashMap;
use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use bytes::Bytes;
use scap::capturer::Resolution;
use scap::frame::Frame;
use tokio::sync::broadcast;

use super::capture_frame::{CaptureFrame, PixelFormat};
use super::screen_capture::{ScreenCaptureConfig, ScreenCaptureService};
use crate::models::ScreenCaptureSource;

const DEFAULT_CHANNEL_CAPACITY: usize = 64;
const ORPHAN_TIMEOUT_SECS: u64 = 60;
const MAX_ENCODER_RESTARTS: u32 = 5;

/// Health status for a capture session
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureHealthStatus {
    pub encoder_alive: bool,
    pub frames_written: u64,
    pub frames_dropped: u64,
    pub width: u32,
    pub height: u32,
}

/// Get current time as milliseconds since UNIX epoch
fn epoch_millis_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// Active H264 capture session
struct H264CaptureSession {
    /// Stop flag for graceful shutdown
    stop_flag: Arc<AtomicBool>,
    /// Flag set when first MPEG-TS data has been produced
    data_ready: Arc<AtomicBool>,
    /// Output broadcast sender (for HTTP mode)
    output_tx: Option<broadcast::Sender<Bytes>>,
    /// Last time this session was accessed (epoch millis, lock-free)
    last_accessed: Arc<AtomicU64>,
    /// Screen capture thread handle
    _capture_handle: std::thread::JoinHandle<()>,
    /// Current width of captured frames (atomic — updated on resolution change)
    width: Arc<AtomicU32>,
    /// Current height of captured frames (atomic — updated on resolution change)
    height: Arc<AtomicU32>,
    /// Display ID for stopping the underlying screen capture
    display_id: u32,
    /// Whether the FFmpeg encoder is alive
    encoder_alive: Arc<AtomicBool>,
    /// Count of frames written to encoder
    frames_written: Arc<AtomicU64>,
    /// Count of frames dropped due to backpressure
    frames_dropped: Arc<AtomicU64>,
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
        Self {
            bitrate_kbps: 4000,
            keyframe_interval: 5, // ~160ms at 30fps for faster preview switching
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
        let data_ready = Arc::new(AtomicBool::new(false));
        let last_accessed = Arc::new(AtomicU64::new(epoch_millis_now()));
        let session_width = Arc::new(AtomicU32::new(width));
        let session_height = Arc::new(AtomicU32::new(height));
        let encoder_alive = Arc::new(AtomicBool::new(true));
        let frames_written = Arc::new(AtomicU64::new(0));
        let frames_dropped = Arc::new(AtomicU64::new(0));

        // Clone values for the capture thread
        let stop_flag_clone = stop_flag.clone();
        let data_ready_clone = data_ready.clone();
        let last_accessed_clone = last_accessed.clone();
        let session_width_clone = session_width.clone();
        let session_height_clone = session_height.clone();
        let encoder_alive_clone = encoder_alive.clone();
        let frames_written_clone = frames_written.clone();
        let frames_dropped_clone = frames_dropped.clone();
        let ffmpeg_path = self.ffmpeg_path.clone();
        let fps = source.fps;
        let source_id_clone = source_id.clone();
        let capture_audio = source.capture_audio;

        // Spawn the capture + encoding thread with elevated priority
        let capture_handle = super::thread_config::CaptureThreadKind::Encoding
            .spawn(&format!("ss-h264-rtsp-{}", source_id), move || {
                run_capture_encoding_loop(
                    frame_rx,
                    stop_flag_clone,
                    data_ready_clone,
                    last_accessed_clone,
                    session_width_clone,
                    session_height_clone,
                    encoder_alive_clone,
                    frames_written_clone,
                    frames_dropped_clone,
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
                    data_ready,
                    output_tx: None, // RTSP mode doesn't use broadcast
                    last_accessed,
                    _capture_handle: capture_handle,
                    width: session_width,
                    height: session_height,
                    display_id: scap_display_id,
                    encoder_alive,
                    frames_written,
                    frames_dropped,
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
                    session.last_accessed.store(epoch_millis_now(), Ordering::Relaxed);
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
        let data_ready = Arc::new(AtomicBool::new(false));
        let last_accessed = Arc::new(AtomicU64::new(epoch_millis_now()));
        let session_width = Arc::new(AtomicU32::new(width));
        let session_height = Arc::new(AtomicU32::new(height));
        let encoder_alive = Arc::new(AtomicBool::new(true));
        let frames_written = Arc::new(AtomicU64::new(0));
        let frames_dropped = Arc::new(AtomicU64::new(0));

        // Clone values for the capture thread
        let stop_flag_clone = stop_flag.clone();
        let data_ready_clone = data_ready.clone();
        let output_tx_clone = output_tx.clone();
        let last_accessed_clone = last_accessed.clone();
        let session_width_clone = session_width.clone();
        let session_height_clone = session_height.clone();
        let encoder_alive_clone = encoder_alive.clone();
        let frames_written_clone = frames_written.clone();
        let frames_dropped_clone = frames_dropped.clone();
        let ffmpeg_path = self.ffmpeg_path.clone();
        let fps = source.fps;
        let source_id_clone = source_id.clone();
        let capture_audio = source.capture_audio;

        // Spawn the capture + encoding thread (HTTP mode) with elevated priority
        let capture_handle = super::thread_config::CaptureThreadKind::Encoding
            .spawn(&format!("ss-h264-http-{}", source_id), move || {
                run_capture_encoding_loop_http(
                    frame_rx,
                    output_tx_clone,
                    stop_flag_clone,
                    data_ready_clone,
                    last_accessed_clone,
                    session_width_clone,
                    session_height_clone,
                    encoder_alive_clone,
                    frames_written_clone,
                    frames_dropped_clone,
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
                    data_ready,
                    output_tx: Some(output_tx),
                    last_accessed,
                    _capture_handle: capture_handle,
                    width: session_width,
                    height: session_height,
                    display_id: scap_display_id,
                    encoder_alive,
                    frames_written,
                    frames_dropped,
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
                    session.last_accessed.store(epoch_millis_now(), Ordering::Relaxed);
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
                session.last_accessed.store(epoch_millis_now(), Ordering::Relaxed);
                return Some(tx.subscribe());
            }
        }
        None
    }

    /// Start compositing a scene: spawns FFmpeg with filter_complex
    /// reading individual source streams from HTTP, outputting MPEG-TS.
    ///
    /// The session key is `scene_{scene_id}`. Each source layer in the scene
    /// must already have an active HTTP capture (at `/api/capture/{source_id}/stream`).
    ///
    /// Returns a broadcast receiver for the composited MPEG-TS stream.
    pub fn start_scene_capture(
        &self,
        scene_id: &str,
        scene: &crate::models::Scene,
        sources: &[crate::models::Source],
        server_port: u16,
        encoding_config: Option<H264EncodingConfig>,
    ) -> Result<broadcast::Receiver<Bytes>, String> {
        let session_key = format!("scene_{}", scene_id);
        let encoding = encoding_config.unwrap_or_default();

        // Check if already compositing this scene
        {
            let sessions = self.sessions.lock().unwrap();
            if let Some(session) = sessions.get(&session_key) {
                if let Some(ref tx) = session.output_tx {
                    session.last_accessed.store(epoch_millis_now(), Ordering::Relaxed);
                    return Ok(tx.subscribe());
                }
            }
        }

        log::info!("Starting scene composite for scene: {} (key: {})", scene_id, session_key);

        // Build FFmpeg args using HTTP inputs from individual source streams
        let ffmpeg_args = super::compositor::Compositor::build_scene_composite_args(
            scene,
            sources,
            server_port,
            &encoding.preset,
            encoding.bitrate_kbps,
            encoding.keyframe_interval,
            encoding.use_hw_accel,
        );

        log::debug!("Scene composite FFmpeg command: {} {:?}", self.ffmpeg_path, ffmpeg_args);

        // Spawn FFmpeg
        let mut ffmpeg = Command::new(&self.ffmpeg_path)
            .args(&ffmpeg_args)
            .stdin(Stdio::null()) // No stdin — inputs come from HTTP
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("Failed to spawn FFmpeg for scene composite: {}", e))?;

        let ffmpeg_pid = ffmpeg.id();
        log::info!("Scene composite FFmpeg started (PID: {}) for {}", ffmpeg_pid, session_key);

        let stdout = ffmpeg.stdout.take().expect("stdout");

        // Stderr reader
        if let Some(stderr) = ffmpeg.stderr.take() {
            spawn_stderr_reader(stderr, format!("scene:{}", scene_id));
        }

        // Create output broadcast channel
        let (output_tx, output_rx) = broadcast::channel::<Bytes>(DEFAULT_CHANNEL_CAPACITY);

        // Session state
        let stop_flag = Arc::new(AtomicBool::new(false));
        let data_ready = Arc::new(AtomicBool::new(false));
        let last_accessed = Arc::new(AtomicU64::new(epoch_millis_now()));
        let session_width = Arc::new(AtomicU32::new(scene.canvas_width));
        let session_height = Arc::new(AtomicU32::new(scene.canvas_height));
        let encoder_alive = Arc::new(AtomicBool::new(true));
        let frames_written = Arc::new(AtomicU64::new(0));
        let frames_dropped = Arc::new(AtomicU64::new(0));

        // Output reader thread
        let stop_clone = stop_flag.clone();
        let data_ready_clone = data_ready.clone();
        let session_key_clone = session_key.clone();
        let output_tx_clone = output_tx.clone();
        let output_thread_handle = super::thread_config::CaptureThreadKind::Encoding
            .spawn(&format!("ss-scene-out-{}", scene_id), move || {
                read_mpegts_output(stdout, output_tx_clone, stop_clone, data_ready_clone, session_key_clone);
            });

        // Lifecycle monitor
        let monitor_alive = encoder_alive.clone();
        let monitor_stop = stop_flag.clone();
        let monitor_source = session_key.clone();
        let mut ffmpeg_for_monitor = ffmpeg;
        let _monitor_handle = super::thread_config::CaptureThreadKind::Monitor
            .spawn(&format!("ss-scene-mon-{}", scene_id), move || {
                loop {
                    if monitor_stop.load(Ordering::SeqCst) {
                        break;
                    }
                    match ffmpeg_for_monitor.try_wait() {
                        Ok(Some(status)) => {
                            log::error!("[SceneComposite:{}] FFmpeg exited: {:?}", monitor_source, status);
                            monitor_alive.store(false, Ordering::SeqCst);
                            break;
                        }
                        Ok(None) => {}
                        Err(e) => {
                            log::error!("[SceneComposite:{}] Monitor error: {}", monitor_source, e);
                            break;
                        }
                    }
                    std::thread::sleep(Duration::from_millis(500));
                }
                ffmpeg_for_monitor
            });

        // The capture handle wraps the output thread (so we have a JoinHandle to store)
        let capture_handle = output_thread_handle;

        // Store the session
        {
            let mut sessions = self.sessions.lock().unwrap();
            sessions.insert(
                session_key.clone(),
                H264CaptureSession {
                    stop_flag,
                    data_ready,
                    output_tx: Some(output_tx),
                    last_accessed,
                    _capture_handle: capture_handle,
                    width: session_width,
                    height: session_height,
                    display_id: 0, // Not a display capture
                    encoder_alive,
                    frames_written,
                    frames_dropped,
                },
            );
        }

        log::info!("Scene composite session created: {} ({}x{})", session_key, scene.canvas_width, scene.canvas_height);
        Ok(output_rx)
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
            .map(|(id, session)| {
                (
                    id.clone(),
                    session.width.load(Ordering::Relaxed),
                    session.height.load(Ordering::Relaxed),
                )
            })
            .collect()
    }

    /// Get health status for a capture session
    pub fn capture_health(&self, source_id: &str) -> Option<CaptureHealthStatus> {
        let sessions = self.sessions.lock().unwrap();
        sessions.get(source_id).map(|session| CaptureHealthStatus {
            encoder_alive: session.encoder_alive.load(Ordering::Relaxed),
            frames_written: session.frames_written.load(Ordering::Relaxed),
            frames_dropped: session.frames_dropped.load(Ordering::Relaxed),
            width: session.width.load(Ordering::Relaxed),
            height: session.height.load(Ordering::Relaxed),
        })
    }

    /// Clean up orphaned sessions that have been inactive too long
    pub fn cleanup_orphans(&self) {
        let mut sessions = self.sessions.lock().unwrap();
        let now_millis = epoch_millis_now();

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
            }
        }
    }

    /// Start encoding from a generic CaptureFrame receiver (E1).
    /// Used by camera capture and other sources that produce CaptureFrame.
    /// Returns a broadcast receiver for the MPEG-TS stream.
    pub fn start_capture_from_frames(
        &self,
        source_id: &str,
        frame_rx: broadcast::Receiver<Arc<CaptureFrame>>,
        initial_width: u32,
        initial_height: u32,
        initial_pix_fmt: PixelFormat,
        fps: u32,
        encoding_config: Option<H264EncodingConfig>,
    ) -> Result<broadcast::Receiver<Bytes>, String> {
        // Check if already capturing
        {
            let sessions = self.sessions.lock().unwrap();
            if let Some(session) = sessions.get(source_id) {
                if let Some(ref tx) = session.output_tx {
                    session.last_accessed.store(epoch_millis_now(), Ordering::Relaxed);
                    return Ok(tx.subscribe());
                }
            }
        }

        let encoding = encoding_config.unwrap_or_default();
        let (output_tx, output_rx) = broadcast::channel::<Bytes>(DEFAULT_CHANNEL_CAPACITY);

        let stop_flag = Arc::new(AtomicBool::new(false));
        let data_ready = Arc::new(AtomicBool::new(false));
        let last_accessed = Arc::new(AtomicU64::new(epoch_millis_now()));
        let session_width = Arc::new(AtomicU32::new(initial_width));
        let session_height = Arc::new(AtomicU32::new(initial_height));
        let encoder_alive = Arc::new(AtomicBool::new(true));
        let frames_written = Arc::new(AtomicU64::new(0));
        let frames_dropped = Arc::new(AtomicU64::new(0));

        let stop_clone = stop_flag.clone();
        let data_ready_clone = data_ready.clone();
        let last_accessed_clone = last_accessed.clone();
        let width_clone = session_width.clone();
        let height_clone = session_height.clone();
        let alive_clone = encoder_alive.clone();
        let written_clone = frames_written.clone();
        let dropped_clone = frames_dropped.clone();
        let output_tx_clone = output_tx.clone();
        let ffmpeg_path = self.ffmpeg_path.clone();
        let source_id_owned = source_id.to_string();

        let capture_handle = super::thread_config::CaptureThreadKind::Encoding
            .spawn(&format!("ss-h264-cf-{}", source_id), move || {
                run_capture_frame_encoding_loop(
                    frame_rx,
                    output_tx_clone,
                    stop_clone,
                    data_ready_clone,
                    last_accessed_clone,
                    width_clone,
                    height_clone,
                    alive_clone,
                    written_clone,
                    dropped_clone,
                    ffmpeg_path,
                    initial_width,
                    initial_height,
                    initial_pix_fmt,
                    fps,
                    encoding,
                    source_id_owned,
                );
            });

        {
            let mut sessions = self.sessions.lock().unwrap();
            sessions.insert(
                source_id.to_string(),
                H264CaptureSession {
                    stop_flag,
                    data_ready,
                    output_tx: Some(output_tx),
                    last_accessed,
                    _capture_handle: capture_handle,
                    width: session_width,
                    height: session_height,
                    display_id: 0, // Not a display capture
                    encoder_alive,
                    frames_written,
                    frames_dropped,
                },
            );
        }

        log::info!("Started CaptureFrame encoding for {} ({}x{} @ {}fps)", source_id, initial_width, initial_height, fps);
        Ok(output_rx)
    }

    /// Wait for the capture session to start producing MPEG-TS data.
    /// Returns Ok(()) when the first chunk has been sent, or Err on timeout.
    /// This prevents go2rtc from connecting before FFmpeg is ready.
    pub fn wait_for_data(&self, source_id: &str, timeout: Duration) -> Result<(), String> {
        let start = Instant::now();
        loop {
            if start.elapsed() > timeout {
                return Err(format!("Timeout waiting for capture data: {}", source_id));
            }

            let sessions = self.sessions.lock().unwrap();
            if let Some(session) = sessions.get(source_id) {
                if session.data_ready.load(Ordering::SeqCst) {
                    return Ok(());
                }
            } else {
                return Err(format!("No active capture session for: {}", source_id));
            }
            drop(sessions);
            std::thread::sleep(Duration::from_millis(50));
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
        // Note: AVFoundation indices include cameras before screens, so an index of "1"
        // with 1 camera + 1 display means the display is actually at scap index 0.
        if let Ok(index) = source.display_id.parse::<usize>() {
            if index < displays.len() {
                let display = &displays[index];
                log::debug!(
                    "Matched display by index {}: scap ID {} ('{}')",
                    index, display.id, display.name
                );
                return Ok(display.id);
            }
            // AVFoundation index may be offset by cameras listed before screens.
            // For single-display systems, just use the first (only) display.
            if displays.len() == 1 {
                log::debug!(
                    "Single display system, using first display regardless of AVFoundation index {}",
                    index
                );
                return Ok(displays[0].id);
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

/// Build FFmpeg args for H264 encoding of rawvideo input.
/// Shared between RTSP and HTTP modes — only the output section differs.
fn build_ffmpeg_encoding_args(
    width: u32,
    height: u32,
    pix_fmt: &str,
    fps: u32,
    encoding: &H264EncodingConfig,
    capture_audio: bool,
) -> Vec<String> {
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

    let mut args = vec![
        "-hide_banner".to_string(),
        "-v".to_string(), "error".to_string(),
        "-fflags".to_string(), "+genpts+nobuffer".to_string(),
        "-flags".to_string(), "low_delay".to_string(),
        "-f".to_string(), "rawvideo".to_string(),
        "-pix_fmt".to_string(), pix_fmt.to_string(),
        "-s".to_string(), format!("{}x{}", width, height),
        "-r".to_string(), fps.to_string(),
        "-i".to_string(), "pipe:0".to_string(),
    ];

    if capture_audio {
        log::info!("Screen capture audio requested but not yet implemented");
    }

    if needs_scale {
        args.extend(["-vf".to_string(),
            format!("scale={}:{}:force_original_aspect_ratio=decrease,pad={}:{}:(ow-iw)/2:(oh-ih)/2",
                    target_width, target_height, target_width, target_height)]);
    }

    if encoding.use_hw_accel && cfg!(target_os = "macos") {
        args.extend([
            "-c:v".to_string(), "h264_videotoolbox".to_string(),
            "-realtime".to_string(), "1".to_string(),
            "-prio_speed".to_string(), "1".to_string(),
            "-allow_sw".to_string(), "1".to_string(),
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

    args.push("-an".to_string());
    args
}

/// Spawn an FFmpeg stderr reader thread to prevent pipe buffer fill
fn spawn_stderr_reader(stderr: std::process::ChildStderr, label: String) {
    super::thread_config::CaptureThreadKind::StderrReader
        .spawn(&format!("ss-stderr-{}", label), move || {
            let reader = std::io::BufReader::new(stderr);
            use std::io::BufRead;
            for line in reader.lines() {
                match line {
                    Ok(line) if !line.trim().is_empty() => {
                        log::warn!("[{}] FFmpeg: {}", label, line.trim());
                    }
                    Err(_) => break,
                    _ => {}
                }
            }
        });
}

/// Drain the broadcast receiver to get the most recent frame (C1: drain-to-latest).
/// Returns the newest available frame, dropping intermediate frames.
/// Falls back to blocking_recv if no frames are buffered.
fn recv_latest_frame(
    frame_rx: &mut broadcast::Receiver<Arc<Frame>>,
    frames_dropped: &AtomicU64,
) -> Result<Arc<Frame>, broadcast::error::RecvError> {
    // Try to drain all buffered frames, keeping only the latest
    let mut latest: Option<Arc<Frame>> = None;
    let mut drained = 0u64;

    loop {
        match frame_rx.try_recv() {
            Ok(frame) => {
                if latest.is_some() {
                    drained += 1;
                }
                latest = Some(frame);
            }
            Err(broadcast::error::TryRecvError::Empty) => break,
            Err(broadcast::error::TryRecvError::Lagged(n)) => {
                // Channel overflowed — those frames are gone
                drained += n;
                continue;
            }
            Err(broadcast::error::TryRecvError::Closed) => {
                return Err(broadcast::error::RecvError::Closed);
            }
        }
    }

    if drained > 0 {
        frames_dropped.fetch_add(drained, Ordering::Relaxed);
    }

    match latest {
        Some(frame) => Ok(frame),
        None => {
            // No buffered frames — block for next one
            frame_rx.blocking_recv()
        }
    }
}

/// Run the inner encoding loop: receive frames, write to FFmpeg stdin.
/// Returns `Some((new_width, new_height))` if resolution changed, `None` on stop/error.
fn run_encoding_inner_loop(
    frame_rx: &mut broadcast::Receiver<Arc<Frame>>,
    stdin: &mut dyn Write,
    stop_flag: &AtomicBool,
    last_accessed: &AtomicU64,
    data_ready: &AtomicBool,
    encoder_alive: &AtomicBool,
    frames_written: &AtomicU64,
    frames_dropped: &AtomicU64,
    width: u32,
    height: u32,
    fps: u32,
    source_id: &str,
    mode_label: &str,
) -> Option<(u32, u32)> {
    let frame_size = (width * height * 4) as usize; // BGRA
    let mut first_frame_written = false;
    let frame_interval = Duration::from_millis((1000 / fps.max(1)) as u64);
    let mut slow_write_streak = 0u32;
    let mut last_drop_log = Instant::now();

    while !stop_flag.load(Ordering::SeqCst) {
        // Check if encoder is still alive
        if !encoder_alive.load(Ordering::Relaxed) {
            log::error!("[{}:{}] Encoder died, stopping frame loop", mode_label, source_id);
            return None;
        }

        last_accessed.store(epoch_millis_now(), Ordering::Relaxed);

        // C1: Drain-to-latest pattern
        let frame = match recv_latest_frame(frame_rx, frames_dropped) {
            Ok(f) => f,
            Err(broadcast::error::RecvError::Lagged(n)) => {
                frames_dropped.fetch_add(n, Ordering::Relaxed);
                continue;
            }
            Err(broadcast::error::RecvError::Closed) => {
                log::info!("[{}:{}] Capture channel closed", mode_label, source_id);
                return None;
            }
        };

        // B1: Per-frame dimension tracking
        let (frame_w, frame_h) = get_frame_dimensions(&frame);
        if frame_w != width || frame_h != height {
            if frame_w > 0 && frame_h > 0 {
                log::info!(
                    "[{}:{}] Resolution changed: {}x{} -> {}x{}, restarting encoder",
                    mode_label, source_id, width, height, frame_w, frame_h
                );
                return Some((frame_w, frame_h));
            }
            // Zero-dim frame — skip it
            continue;
        }

        // Extract raw frame data
        if let Some(data) = extract_frame_data(&frame, frame_size) {
            // C2: Backpressure detection
            let write_start = Instant::now();
            if let Err(e) = stdin.write_all(data) {
                log::error!("[{}:{}] Failed to write frame: {}", mode_label, source_id, e);
                return None;
            }
            let write_duration = write_start.elapsed();

            frames_written.fetch_add(1, Ordering::Relaxed);

            // C2: Track slow writes
            if write_duration > frame_interval {
                slow_write_streak += 1;
                if slow_write_streak >= 3 {
                    // Skip next frame to let encoder catch up
                    frames_dropped.fetch_add(1, Ordering::Relaxed);
                    slow_write_streak = 0;
                }
            } else {
                slow_write_streak = 0;
            }

            if !first_frame_written {
                first_frame_written = true;
                data_ready.store(true, Ordering::SeqCst);
                log::debug!("[{}:{}] First frame written to FFmpeg", mode_label, source_id);
            }
        }

        // Periodic drop count logging (every 5s)
        if last_drop_log.elapsed() > Duration::from_secs(5) {
            let dropped = frames_dropped.load(Ordering::Relaxed);
            let written = frames_written.load(Ordering::Relaxed);
            if dropped > 0 {
                log::info!(
                    "[{}:{}] Stats: {} written, {} dropped",
                    mode_label, source_id, written, dropped
                );
            }
            last_drop_log = Instant::now();
        }
    }

    None // Stopped via flag
}

/// Main capture and encoding loop running in a separate thread (RTSP output mode)
/// Wraps the inner loop with retry logic for resolution changes (B2).
fn run_capture_encoding_loop(
    mut frame_rx: broadcast::Receiver<Arc<Frame>>,
    stop_flag: Arc<AtomicBool>,
    data_ready: Arc<AtomicBool>,
    last_accessed: Arc<AtomicU64>,
    session_width: Arc<AtomicU32>,
    session_height: Arc<AtomicU32>,
    encoder_alive: Arc<AtomicBool>,
    frames_written: Arc<AtomicU64>,
    frames_dropped: Arc<AtomicU64>,
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
    let (mut width, mut height) = match wait_for_first_frame(&mut frame_rx, &stop_flag) {
        Some((w, h)) => (w, h),
        None => {
            log::warn!("No frames received for H264 capture: {}", source_id);
            encoder_alive.store(false, Ordering::SeqCst);
            return;
        }
    };

    log::debug!(
        "[{:?}] First frame for {}: {}x{} (initial estimate {}x{}, RTSP)",
        encoding_start.elapsed(), source_id, width, height, initial_width, initial_height
    );

    // B2: Outer retry loop for resolution changes
    let mut restart_count = 0u32;

    loop {
        if stop_flag.load(Ordering::SeqCst) || restart_count >= MAX_ENCODER_RESTARTS {
            if restart_count >= MAX_ENCODER_RESTARTS {
                log::error!("[RTSP:{}] Max encoder restarts ({}) reached", source_id, MAX_ENCODER_RESTARTS);
            }
            break;
        }

        // B3: Update atomic dimensions
        session_width.store(width, Ordering::Relaxed);
        session_height.store(height, Ordering::Relaxed);

        // Build FFmpeg args
        let mut ffmpeg_args = build_ffmpeg_encoding_args(width, height, "bgra", fps, &encoding, capture_audio);

        // RTSP output
        ffmpeg_args.extend([
            "-rtsp_transport".to_string(), "tcp".to_string(),
            "-f".to_string(), "rtsp".to_string(),
            rtsp_output_url.clone(),
        ]);

        log::debug!("FFmpeg command: {} {:?}", ffmpeg_path, ffmpeg_args);

        // Spawn FFmpeg
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
                encoder_alive.store(false, Ordering::SeqCst);
                return;
            }
        };

        let ffmpeg_pid = ffmpeg.id();
        log::info!("[{:?}] FFmpeg started (PID: {}, RTSP)", encoding_start.elapsed(), ffmpeg_pid);
        encoder_alive.store(true, Ordering::SeqCst);

        let mut stdin = ffmpeg.stdin.take().expect("stdin");

        // D2: Stderr reader
        if let Some(stderr) = ffmpeg.stderr.take() {
            spawn_stderr_reader(stderr, format!("H264/RTSP:{}", source_id));
        }

        // D3: FFmpeg lifecycle monitor
        let monitor_alive = encoder_alive.clone();
        let monitor_stop = stop_flag.clone();
        let monitor_source = source_id.clone();
        let mut ffmpeg_for_monitor = ffmpeg;
        let monitor_thread = super::thread_config::CaptureThreadKind::Monitor
            .spawn(&format!("ss-h264-mon-{}", monitor_source), move || {
                loop {
                    if monitor_stop.load(Ordering::SeqCst) {
                        break;
                    }
                    match ffmpeg_for_monitor.try_wait() {
                        Ok(Some(status)) => {
                            log::error!("[H264:{}] FFmpeg exited unexpectedly: {:?}", monitor_source, status);
                            monitor_alive.store(false, Ordering::SeqCst);
                            break;
                        }
                        Ok(None) => {}
                        Err(e) => {
                            log::error!("[H264:{}] Failed to check FFmpeg status: {}", monitor_source, e);
                            break;
                        }
                    }
                    std::thread::sleep(Duration::from_millis(500));
                }
                ffmpeg_for_monitor
            });

        // Run inner encoding loop
        let result = run_encoding_inner_loop(
            &mut frame_rx, &mut stdin,
            &stop_flag, &last_accessed, &data_ready, &encoder_alive,
            &frames_written, &frames_dropped,
            width, height, fps, &source_id, "RTSP",
        );

        // Cleanup this encoder instance
        drop(stdin);
        if let Ok(mut ffmpeg) = monitor_thread.join() {
            let _ = ffmpeg.kill();
            let _ = ffmpeg.wait();
        }

        match result {
            Some((new_w, new_h)) => {
                // B2: Resolution changed — restart with new dimensions
                width = new_w;
                height = new_h;
                restart_count += 1;
                log::info!("[RTSP:{}] Restarting encoder #{} for {}x{}", source_id, restart_count, width, height);
            }
            None => break, // Stopped or error
        }
    }

    encoder_alive.store(false, Ordering::SeqCst);
    log::info!("H264 RTSP capture stopped for {}", source_id);
}

/// Main capture and encoding loop for HTTP/MPEG-TS output mode.
/// Wraps the inner loop with retry logic for resolution changes (B2).
fn run_capture_encoding_loop_http(
    mut frame_rx: broadcast::Receiver<Arc<Frame>>,
    output_tx: broadcast::Sender<Bytes>,
    stop_flag: Arc<AtomicBool>,
    data_ready: Arc<AtomicBool>,
    last_accessed: Arc<AtomicU64>,
    session_width: Arc<AtomicU32>,
    session_height: Arc<AtomicU32>,
    encoder_alive: Arc<AtomicBool>,
    frames_written: Arc<AtomicU64>,
    frames_dropped: Arc<AtomicU64>,
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
    let (mut width, mut height) = match wait_for_first_frame(&mut frame_rx, &stop_flag) {
        Some((w, h)) => (w, h),
        None => {
            log::warn!("No frames received for H264 capture: {}", source_id);
            encoder_alive.store(false, Ordering::SeqCst);
            return;
        }
    };

    log::debug!(
        "[{:?}] First frame for {}: {}x{} (initial estimate {}x{}, HTTP/MPEG-TS)",
        encoding_start.elapsed(), source_id, width, height, initial_width, initial_height
    );

    // B2: Outer retry loop for resolution changes
    let mut restart_count = 0u32;

    loop {
        if stop_flag.load(Ordering::SeqCst) || restart_count >= MAX_ENCODER_RESTARTS {
            if restart_count >= MAX_ENCODER_RESTARTS {
                log::error!("[HTTP:{}] Max encoder restarts ({}) reached", source_id, MAX_ENCODER_RESTARTS);
            }
            break;
        }

        // B3: Update atomic dimensions
        session_width.store(width, Ordering::Relaxed);
        session_height.store(height, Ordering::Relaxed);

        // Build FFmpeg args
        let mut ffmpeg_args = build_ffmpeg_encoding_args(width, height, "bgra", fps, &encoding, capture_audio);

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
                encoder_alive.store(false, Ordering::SeqCst);
                return;
            }
        };

        let ffmpeg_pid = ffmpeg.id();
        log::info!("[{:?}] FFmpeg started (PID: {}, HTTP/MPEG-TS)", encoding_start.elapsed(), ffmpeg_pid);
        encoder_alive.store(true, Ordering::SeqCst);

        let mut stdin = ffmpeg.stdin.take().expect("stdin");
        let stdout = ffmpeg.stdout.take().expect("stdout");

        // D2: Stderr reader
        if let Some(stderr) = ffmpeg.stderr.take() {
            spawn_stderr_reader(stderr, format!("H264/HTTP:{}", source_id));
        }

        // Spawn MPEG-TS output reader
        let stop_clone = stop_flag.clone();
        let data_ready_clone = data_ready.clone();
        let source_id_clone = source_id.clone();
        let output_tx_clone = output_tx.clone();
        let output_thread = super::thread_config::CaptureThreadKind::Encoding
            .spawn(&format!("ss-mpegts-http-{}", source_id), move || {
                read_mpegts_output(stdout, output_tx_clone, stop_clone, data_ready_clone, source_id_clone);
            });

        // D3: FFmpeg lifecycle monitor
        let monitor_alive = encoder_alive.clone();
        let monitor_stop = stop_flag.clone();
        let monitor_source = source_id.clone();
        let mut ffmpeg_for_monitor = ffmpeg;
        let monitor_thread = super::thread_config::CaptureThreadKind::Monitor
            .spawn(&format!("ss-h264-mon-{}", monitor_source), move || {
                loop {
                    if monitor_stop.load(Ordering::SeqCst) {
                        break;
                    }
                    match ffmpeg_for_monitor.try_wait() {
                        Ok(Some(status)) => {
                            log::error!("[H264:{}] FFmpeg exited unexpectedly: {:?}", monitor_source, status);
                            monitor_alive.store(false, Ordering::SeqCst);
                            break;
                        }
                        Ok(None) => {}
                        Err(e) => {
                            log::error!("[H264:{}] Failed to check FFmpeg status: {}", monitor_source, e);
                            break;
                        }
                    }
                    std::thread::sleep(Duration::from_millis(500));
                }
                ffmpeg_for_monitor
            });

        // Run inner encoding loop
        let result = run_encoding_inner_loop(
            &mut frame_rx, &mut stdin,
            &stop_flag, &last_accessed, &data_ready, &encoder_alive,
            &frames_written, &frames_dropped,
            width, height, fps, &source_id, "HTTP",
        );

        // Cleanup this encoder instance
        drop(stdin);
        if let Ok(mut ffmpeg) = monitor_thread.join() {
            let _ = ffmpeg.kill();
            let _ = ffmpeg.wait();
        }
        let _ = output_thread.join();

        match result {
            Some((new_w, new_h)) => {
                // B2: Resolution changed — restart with new dimensions
                width = new_w;
                height = new_h;
                restart_count += 1;
                log::info!("[HTTP:{}] Restarting encoder #{} for {}x{}", source_id, restart_count, width, height);
            }
            None => break,
        }
    }

    encoder_alive.store(false, Ordering::SeqCst);
    log::info!("H264 HTTP capture stopped for {}", source_id);
}

/// Read MPEG-TS output from FFmpeg and broadcast chunks
fn read_mpegts_output(
    mut stdout: std::process::ChildStdout,
    output_tx: broadcast::Sender<Bytes>,
    stop_flag: Arc<AtomicBool>,
    data_ready: Arc<AtomicBool>,
    source_id: String,
) {
    const CHUNK_SIZE: usize = 188 * 7; // TS packet multiples
    let mut buffer = vec![0u8; CHUNK_SIZE];
    let mut first_chunk = true;

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
                if first_chunk {
                    first_chunk = false;
                    data_ready.store(true, Ordering::SeqCst);
                    log::debug!("First MPEG-TS chunk produced for {}", source_id);
                }
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

/// Wait for the first valid frame to determine actual dimensions.
/// Skips zero-dimension frames, retrying up to 10 frames before giving up.
fn wait_for_first_frame(
    frame_rx: &mut broadcast::Receiver<Arc<Frame>>,
    stop_flag: &Arc<AtomicBool>,
) -> Option<(u32, u32)> {
    let start = Instant::now();
    let timeout = Duration::from_secs(5);
    let mut zero_dim_count = 0u32;
    const MAX_ZERO_DIM_RETRIES: u32 = 10;

    loop {
        if stop_flag.load(Ordering::SeqCst) {
            return None;
        }

        if start.elapsed() > timeout {
            log::warn!("Timeout waiting for first frame (got {} zero-dim frames)", zero_dim_count);
            return None;
        }

        match frame_rx.blocking_recv() {
            Ok(frame) => {
                let (width, height) = get_frame_dimensions(&frame);

                // Skip zero-dimension frames (can happen during display init)
                if width == 0 || height == 0 {
                    zero_dim_count += 1;
                    if zero_dim_count >= MAX_ZERO_DIM_RETRIES {
                        log::error!("Got {} consecutive zero-dimension frames, giving up", zero_dim_count);
                        return None;
                    }
                    log::debug!("Skipping zero-dimension frame ({}/{})", zero_dim_count, MAX_ZERO_DIM_RETRIES);
                    continue;
                }

                let elapsed = start.elapsed();
                log::debug!("First valid frame received in {:?}: {}x{}", elapsed, width, height);
                return Some((width, height));
            }
            Err(broadcast::error::RecvError::Lagged(n)) => {
                log::debug!("Lagged by {} frames while waiting for first frame", n);
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

/// Extract raw frame data from a scap Frame (zero-copy borrow)
/// Returns None for invalid frames: empty data, undersized data, or non-BGRA format
fn extract_frame_data(frame: &Frame, expected_size: usize) -> Option<&[u8]> {
    let data = match frame {
        Frame::BGRA(bgra) => &bgra.data,
        _ => {
            log::warn!("Unexpected frame format, expected BGRA");
            return None;
        }
    };

    // Reject empty frames
    if data.is_empty() {
        log::warn!("Empty frame data, skipping");
        return None;
    }

    // Reject undersized frames — writing truncated data to FFmpeg's rawvideo
    // stream corrupts the pixel grid and can cause SIGSEGV/SIGFPE
    if data.len() < expected_size {
        log::warn!(
            "Frame data undersized: expected {} bytes, got {} — skipping",
            expected_size,
            data.len()
        );
        return None;
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

/// Drain CaptureFrame broadcast receiver to get the latest frame.
fn recv_latest_capture_frame(
    frame_rx: &mut broadcast::Receiver<Arc<CaptureFrame>>,
    frames_dropped: &AtomicU64,
) -> Result<Arc<CaptureFrame>, broadcast::error::RecvError> {
    let mut latest: Option<Arc<CaptureFrame>> = None;
    let mut drained = 0u64;

    loop {
        match frame_rx.try_recv() {
            Ok(frame) => {
                if latest.is_some() {
                    drained += 1;
                }
                latest = Some(frame);
            }
            Err(broadcast::error::TryRecvError::Empty) => break,
            Err(broadcast::error::TryRecvError::Lagged(n)) => {
                drained += n;
                continue;
            }
            Err(broadcast::error::TryRecvError::Closed) => {
                return Err(broadcast::error::RecvError::Closed);
            }
        }
    }

    if drained > 0 {
        frames_dropped.fetch_add(drained, Ordering::Relaxed);
    }

    match latest {
        Some(frame) => Ok(frame),
        None => frame_rx.blocking_recv(),
    }
}

/// Encoding loop for CaptureFrame sources (cameras, capture cards).
/// Supports resolution change detection and encoder restart.
fn run_capture_frame_encoding_loop(
    mut frame_rx: broadcast::Receiver<Arc<CaptureFrame>>,
    output_tx: broadcast::Sender<Bytes>,
    stop_flag: Arc<AtomicBool>,
    data_ready: Arc<AtomicBool>,
    last_accessed: Arc<AtomicU64>,
    session_width: Arc<AtomicU32>,
    session_height: Arc<AtomicU32>,
    encoder_alive: Arc<AtomicBool>,
    frames_written: Arc<AtomicU64>,
    frames_dropped: Arc<AtomicU64>,
    ffmpeg_path: String,
    initial_width: u32,
    initial_height: u32,
    _initial_pix_fmt: PixelFormat,
    fps: u32,
    encoding: H264EncodingConfig,
    source_id: String,
) {
    let _ = (initial_width, initial_height); // Used as initial estimates only

    // Wait for first valid frame
    let mut width;
    let mut height;
    let first_frame = loop {
        if stop_flag.load(Ordering::SeqCst) {
            encoder_alive.store(false, Ordering::SeqCst);
            return;
        }
        match frame_rx.blocking_recv() {
            Ok(frame) => {
                if frame.validate().is_ok() {
                    width = frame.width;
                    height = frame.height;
                    break frame;
                }
            }
            Err(broadcast::error::RecvError::Lagged(_)) => continue,
            Err(broadcast::error::RecvError::Closed) => {
                encoder_alive.store(false, Ordering::SeqCst);
                return;
            }
        }
    };

    let mut pix_fmt = first_frame.pixel_format;
    let mut restart_count = 0u32;

    loop {
        if stop_flag.load(Ordering::SeqCst) || restart_count >= MAX_ENCODER_RESTARTS {
            break;
        }

        session_width.store(width, Ordering::Relaxed);
        session_height.store(height, Ordering::Relaxed);

        let mut ffmpeg_args = build_ffmpeg_encoding_args(width, height, pix_fmt.ffmpeg_pix_fmt(), fps, &encoding, false);

        // MPEG-TS output to stdout
        ffmpeg_args.extend([
            "-flush_packets".to_string(), "1".to_string(),
            "-f".to_string(), "mpegts".to_string(),
            "-muxdelay".to_string(), "0".to_string(),
            "pipe:1".to_string(),
        ]);

        let mut ffmpeg = match Command::new(&ffmpeg_path)
            .args(&ffmpeg_args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
        {
            Ok(child) => child,
            Err(e) => {
                log::error!("[CaptureFrame:{}] Failed to spawn FFmpeg: {}", source_id, e);
                encoder_alive.store(false, Ordering::SeqCst);
                return;
            }
        };

        log::info!("[CaptureFrame:{}] FFmpeg started (PID: {}, {}x{}, {})",
            source_id, ffmpeg.id(), width, height, pix_fmt.ffmpeg_pix_fmt());
        encoder_alive.store(true, Ordering::SeqCst);

        let mut stdin = ffmpeg.stdin.take().expect("stdin");
        let stdout = ffmpeg.stdout.take().expect("stdout");

        if let Some(stderr) = ffmpeg.stderr.take() {
            spawn_stderr_reader(stderr, format!("CaptureFrame:{}", source_id));
        }

        // Output reader
        let stop_clone = stop_flag.clone();
        let data_ready_clone = data_ready.clone();
        let source_clone = source_id.clone();
        let output_tx_clone = output_tx.clone();
        let output_thread = super::thread_config::CaptureThreadKind::Encoding
            .spawn(&format!("ss-mpegts-cf-{}", source_id), move || {
                read_mpegts_output(stdout, output_tx_clone, stop_clone, data_ready_clone, source_clone);
            });

        // Lifecycle monitor
        let monitor_alive = encoder_alive.clone();
        let monitor_stop = stop_flag.clone();
        let monitor_source = source_id.clone();
        let mut ffmpeg_for_monitor = ffmpeg;
        let monitor_thread = super::thread_config::CaptureThreadKind::Monitor
            .spawn(&format!("ss-h264-mon-{}", monitor_source), move || {
                loop {
                    if monitor_stop.load(Ordering::SeqCst) { break; }
                    match ffmpeg_for_monitor.try_wait() {
                        Ok(Some(status)) => {
                            log::error!("[CaptureFrame:{}] FFmpeg exited: {:?}", monitor_source, status);
                            monitor_alive.store(false, Ordering::SeqCst);
                            break;
                        }
                        Ok(None) => {}
                        Err(_) => break,
                    }
                    std::thread::sleep(Duration::from_millis(500));
                }
                ffmpeg_for_monitor
            });

        // Inner encoding loop
        let frame_interval = Duration::from_millis((1000 / fps.max(1)) as u64);
        let mut slow_write_streak = 0u32;
        let mut last_drop_log = Instant::now();
        let mut resolution_changed: Option<(u32, u32, PixelFormat)> = None;

        while !stop_flag.load(Ordering::SeqCst) && encoder_alive.load(Ordering::Relaxed) {
            last_accessed.store(epoch_millis_now(), Ordering::Relaxed);

            let frame = match recv_latest_capture_frame(&mut frame_rx, &frames_dropped) {
                Ok(f) => f,
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    frames_dropped.fetch_add(n, Ordering::Relaxed);
                    continue;
                }
                Err(broadcast::error::RecvError::Closed) => break,
            };

            if frame.validate().is_err() {
                continue;
            }

            // Resolution/format change detection
            if frame.width != width || frame.height != height || frame.pixel_format != pix_fmt {
                if frame.width > 0 && frame.height > 0 {
                    resolution_changed = Some((frame.width, frame.height, frame.pixel_format));
                    break;
                }
                continue;
            }

            let expected_size = pix_fmt.expected_size(width, height);
            if expected_size > 0 && frame.data.len() < expected_size {
                continue;
            }

            // Write frame
            let write_start = Instant::now();
            if let Err(e) = stdin.write_all(&frame.data) {
                log::error!("[CaptureFrame:{}] Write error: {}", source_id, e);
                break;
            }

            frames_written.fetch_add(1, Ordering::Relaxed);

            if write_start.elapsed() > frame_interval {
                slow_write_streak += 1;
                if slow_write_streak >= 3 {
                    frames_dropped.fetch_add(1, Ordering::Relaxed);
                    slow_write_streak = 0;
                }
            } else {
                slow_write_streak = 0;
            }

            if last_drop_log.elapsed() > Duration::from_secs(5) {
                let dropped = frames_dropped.load(Ordering::Relaxed);
                if dropped > 0 {
                    log::info!("[CaptureFrame:{}] Stats: {} written, {} dropped",
                        source_id, frames_written.load(Ordering::Relaxed), dropped);
                }
                last_drop_log = Instant::now();
            }
        }

        // Cleanup
        drop(stdin);
        if let Ok(mut ffmpeg) = monitor_thread.join() {
            let _ = ffmpeg.kill();
            let _ = ffmpeg.wait();
        }
        let _ = output_thread.join();

        match resolution_changed {
            Some((new_w, new_h, new_fmt)) => {
                width = new_w;
                height = new_h;
                pix_fmt = new_fmt;
                restart_count += 1;
                log::info!("[CaptureFrame:{}] Restarting encoder #{} for {}x{} {}",
                    source_id, restart_count, width, height, pix_fmt.ffmpeg_pix_fmt());
            }
            None => break,
        }
    }

    encoder_alive.store(false, Ordering::SeqCst);
    log::info!("[CaptureFrame:{}] Encoding loop stopped", source_id);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encoding_config_defaults() {
        let config = H264EncodingConfig::default();
        assert_eq!(config.bitrate_kbps, 4000);
        assert_eq!(config.keyframe_interval, 5); // ~160ms at 30fps for faster preview switching
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
