// FFmpeg libs pipeline (in-process).
// This module is feature-gated so we can build the new pipeline without
// touching the existing FFmpeg CLI flow.

#![cfg(feature = "ffmpeg-libs")]

use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::ffi::c_void;
use std::os::raw::c_int;
use std::ptr;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU64, Ordering},
    mpsc::{self, Sender, Receiver},
};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use ffmpeg_sys_next as ffi;

use crate::models::{OutputGroup, StreamStats, StreamTarget};
use crate::services::{emit_event, EventSink};

// ============================================================================
// Per-Group Control
// ============================================================================

/// Control state for an individual output group.
/// Allows stopping/enabling groups without restarting the entire pipeline.
#[derive(Clone)]
pub struct GroupControl {
    /// When true, stop this group and clean up its resources
    stop_flag: Arc<AtomicBool>,
    /// When false, skip writing packets to this group (soft disable)
    enabled: Arc<AtomicBool>,
}

impl GroupControl {
    fn new() -> Self {
        Self {
            stop_flag: Arc::new(AtomicBool::new(false)),
            enabled: Arc::new(AtomicBool::new(true)),
        }
    }

    /// Check if this group should stop
    pub fn should_stop(&self) -> bool {
        self.stop_flag.load(Ordering::SeqCst)
    }

    /// Check if this group is enabled for packet writing
    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::SeqCst)
    }

    /// Signal this group to stop
    pub fn stop(&self) {
        self.stop_flag.store(true, Ordering::SeqCst);
    }

    /// Enable packet writing to this group
    pub fn enable(&self) {
        self.enabled.store(true, Ordering::SeqCst);
    }

    /// Disable packet writing to this group (soft stop - keeps connection)
    pub fn disable(&self) {
        self.enabled.store(false, Ordering::SeqCst);
    }
}

impl Default for GroupControl {
    fn default() -> Self {
        Self::new()
    }
}

/// External handle to control a group from outside the pipeline thread
#[derive(Clone)]
pub struct GroupHandle {
    pub group_id: String,
    pub mode: OutputGroupMode,
    control: GroupControl,
}

impl GroupHandle {
    /// Stop this group and clean up its resources
    pub fn stop(&self) {
        log::info!("Stopping group {} via handle", self.group_id);
        self.control.stop();
    }

    /// Check if this group is stopped
    pub fn is_stopped(&self) -> bool {
        self.control.should_stop()
    }

    /// Enable packet writing to this group
    pub fn enable(&self) {
        log::info!("Enabling group {} via handle", self.group_id);
        self.control.enable();
    }

    /// Disable packet writing (soft stop - keeps RTMP connection)
    pub fn disable(&self) {
        log::info!("Disabling group {} via handle", self.group_id);
        self.control.disable();
    }

    /// Check if this group is enabled
    pub fn is_enabled(&self) -> bool {
        self.control.is_enabled()
    }
}

/// Commands that can be sent to the pipeline thread for runtime control
#[derive(Debug)]
pub enum PipelineCommand {
    /// Stop a specific group by ID
    StopGroup(String),
    /// Add a new group to the running pipeline
    AddGroup(OutputGroupConfig),
}

// ============================================================================
// Per-Group Stats Tracking
// ============================================================================

/// Tracks statistics for a single output group during pipeline execution.
/// Used to accumulate data that will be emitted as StreamStats events.
struct GroupStatsTracker {
    group_id: String,
    /// Total video frames written
    frame_count: u64,
    /// Total bytes written (video + audio)
    total_bytes: u64,
    /// When the group started
    start_time: Instant,
    /// When stats were last emitted
    last_emit: Instant,
    /// FPS calculation: frames in the last second
    recent_frames: u64,
    /// Timestamp of last FPS calculation
    last_fps_time: Instant,
    /// Calculated FPS value
    current_fps: f64,
}

impl GroupStatsTracker {
    fn new(group_id: String) -> Self {
        let now = Instant::now();
        Self {
            group_id,
            frame_count: 0,
            total_bytes: 0,
            start_time: now,
            last_emit: now,
            recent_frames: 0,
            last_fps_time: now,
            current_fps: 0.0,
        }
    }

    /// Record a video frame written
    fn record_video_frame(&mut self, size_bytes: u64) {
        self.frame_count += 1;
        self.total_bytes += size_bytes;
        self.recent_frames += 1;
    }

    /// Record an audio packet written
    fn record_audio_packet(&mut self, size_bytes: u64) {
        self.total_bytes += size_bytes;
    }

    /// Update FPS calculation if enough time has passed
    fn update_fps(&mut self) {
        let elapsed = self.last_fps_time.elapsed();
        if elapsed >= Duration::from_secs(1) {
            let secs = elapsed.as_secs_f64();
            self.current_fps = self.recent_frames as f64 / secs;
            self.recent_frames = 0;
            self.last_fps_time = Instant::now();
        }
    }

    /// Check if stats should be emitted (every ~1 second)
    fn should_emit(&self) -> bool {
        self.last_emit.elapsed() >= Duration::from_secs(1)
    }

    /// Generate StreamStats and reset emit timer
    fn emit_stats(&mut self, dropped_frames: u64) -> StreamStats {
        self.update_fps();
        self.last_emit = Instant::now();

        let elapsed_secs = self.start_time.elapsed().as_secs_f64();
        // Calculate bitrate in kbps: (bytes * 8) / (seconds * 1000)
        let bitrate_kbps = if elapsed_secs > 0.0 {
            (self.total_bytes as f64 * 8.0) / (elapsed_secs * 1000.0)
        } else {
            0.0
        };

        // Calculate speed: how fast we're processing relative to real-time
        // For a real-time stream, this should be ~1.0x
        let speed = if elapsed_secs > 0.0 && self.current_fps > 0.0 {
            // Assume 30fps as baseline for speed calculation
            // A better approach would be to get actual input fps
            1.0
        } else {
            0.0
        };

        StreamStats {
            group_id: self.group_id.clone(),
            frame: self.frame_count,
            fps: self.current_fps,
            bitrate: bitrate_kbps,
            speed,
            size: self.total_bytes,
            time: elapsed_secs,
            dropped_frames,
            dup_frames: 0,
        }
    }
}

// ============================================================================
// Bounded Packet Queue
// ============================================================================

/// Configuration for packet queue buffering
#[derive(Debug, Clone)]
pub struct QueueConfig {
    /// Maximum number of packets to buffer per target
    pub max_packets: usize,
    /// Strategy for handling queue overflow
    pub drop_strategy: DropStrategy,
}

impl Default for QueueConfig {
    fn default() -> Self {
        Self {
            max_packets: 300, // ~10 seconds at 30fps
            drop_strategy: DropStrategy::DropOldest,
        }
    }
}

/// Strategy for dropping packets when queue is full
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DropStrategy {
    /// Drop oldest packets first (better for live streaming - keeps up with real-time)
    DropOldest,
    /// Drop newest packets (preserves continuity but falls behind)
    DropNewest,
}

/// A bounded packet queue that prevents unbounded memory growth
struct PacketQueue {
    /// Buffered packets (each is a cloned AVPacket pointer)
    packets: std::collections::VecDeque<*mut ffi::AVPacket>,
    /// Maximum queue size
    max_size: usize,
    /// Drop strategy when full
    drop_strategy: DropStrategy,
    /// Total packets dropped due to overflow
    dropped_count: u64,
}

impl PacketQueue {
    fn new(config: &QueueConfig) -> Self {
        Self {
            packets: std::collections::VecDeque::with_capacity(config.max_packets),
            max_size: config.max_packets,
            drop_strategy: config.drop_strategy,
            dropped_count: 0,
        }
    }

    /// Push a packet to the queue, dropping if necessary
    /// Returns the number of packets dropped (0 or 1)
    fn push(&mut self, packet: *mut ffi::AVPacket) -> u64 {
        if self.packets.len() >= self.max_size {
            match self.drop_strategy {
                DropStrategy::DropOldest => {
                    // Drop oldest packet
                    if let Some(old) = self.packets.pop_front() {
                        unsafe { ffi::av_packet_free(&mut (old as *mut _)) };
                    }
                    self.packets.push_back(packet);
                    self.dropped_count += 1;
                    1
                }
                DropStrategy::DropNewest => {
                    // Drop the incoming packet
                    unsafe { ffi::av_packet_free(&mut (packet as *mut _)) };
                    self.dropped_count += 1;
                    1
                }
            }
        } else {
            self.packets.push_back(packet);
            0
        }
    }

    /// Pop the next packet from the queue
    fn pop(&mut self) -> Option<*mut ffi::AVPacket> {
        self.packets.pop_front()
    }

    /// Get the current queue length
    fn len(&self) -> usize {
        self.packets.len()
    }

    /// Check if queue is empty
    fn is_empty(&self) -> bool {
        self.packets.is_empty()
    }

    /// Get total dropped packet count
    fn dropped(&self) -> u64 {
        self.dropped_count
    }

    /// Clear the queue, freeing all packets
    fn clear(&mut self) {
        while let Some(pkt) = self.packets.pop_front() {
            unsafe { ffi::av_packet_free(&mut (pkt as *mut _)) };
        }
    }
}

impl Drop for PacketQueue {
    fn drop(&mut self) {
        self.clear();
    }
}

// ============================================================================
// Error Recovery and Reconnection
// ============================================================================

/// Configuration for automatic reconnection attempts
#[derive(Debug, Clone)]
pub struct ReconnectionConfig {
    /// Maximum number of retry attempts before giving up
    pub max_retries: u32,
    /// Initial delay between retries in seconds
    pub initial_delay_secs: u64,
    /// Maximum delay between retries in seconds (caps exponential growth)
    pub max_delay_secs: u64,
    /// Number of consecutive write failures before marking target as failed
    pub failure_threshold: u32,
    /// Whether to automatically attempt reconnection
    pub auto_reconnect: bool,
}

impl Default for ReconnectionConfig {
    fn default() -> Self {
        Self {
            max_retries: 5,
            initial_delay_secs: 2,
            max_delay_secs: 60,
            failure_threshold: 10,
            auto_reconnect: true,
        }
    }
}

/// Tracks the state of a single output target for error recovery
struct TargetState {
    /// URL for reconnection
    url: String,
    /// Consecutive write failures
    consecutive_failures: u32,
    /// Whether this target is currently failed/disconnected
    is_failed: bool,
    /// Current reconnection attempt (0 = not reconnecting)
    reconnect_attempt: u32,
    /// Time of last reconnection attempt
    last_reconnect: Option<Instant>,
    /// Whether reconnection is in progress
    reconnecting: bool,
}

impl TargetState {
    fn new(url: String) -> Self {
        Self {
            url,
            consecutive_failures: 0,
            is_failed: false,
            reconnect_attempt: 0,
            last_reconnect: None,
            reconnecting: false,
        }
    }

    /// Record a successful write (resets failure counter)
    fn record_success(&mut self) {
        self.consecutive_failures = 0;
        if self.is_failed {
            self.is_failed = false;
            self.reconnect_attempt = 0;
            log::info!("Target {} recovered", self.url);
        }
    }

    /// Record a write failure, returns true if target should be marked as failed
    fn record_failure(&mut self, threshold: u32) -> bool {
        self.consecutive_failures += 1;
        if !self.is_failed && self.consecutive_failures >= threshold {
            self.is_failed = true;
            log::warn!(
                "Target {} marked as failed after {} consecutive failures",
                self.url,
                self.consecutive_failures
            );
            true
        } else {
            false
        }
    }

    /// Check if enough time has passed for next reconnection attempt
    fn should_attempt_reconnect(&self, config: &ReconnectionConfig) -> bool {
        if !self.is_failed || self.reconnecting {
            return false;
        }
        if self.reconnect_attempt >= config.max_retries {
            return false;
        }
        match self.last_reconnect {
            None => true,
            Some(last) => {
                let delay = self.next_delay(config);
                last.elapsed() >= delay
            }
        }
    }

    /// Calculate next reconnection delay with exponential backoff
    fn next_delay(&self, config: &ReconnectionConfig) -> Duration {
        let delay_secs = config.initial_delay_secs * (1u64 << self.reconnect_attempt.min(6));
        Duration::from_secs(delay_secs.min(config.max_delay_secs))
    }

    /// Mark reconnection attempt started
    fn start_reconnect(&mut self) {
        self.reconnecting = true;
        self.reconnect_attempt += 1;
        self.last_reconnect = Some(Instant::now());
    }

    /// Mark reconnection attempt finished
    fn finish_reconnect(&mut self, success: bool) {
        self.reconnecting = false;
        if success {
            self.is_failed = false;
            self.consecutive_failures = 0;
            self.reconnect_attempt = 0;
        }
    }
}

#[derive(Debug, Clone)]
pub struct InputPipelineConfig {
    pub input_id: String,
    pub input_url: String,
    /// Expected stream key for RTMP listen mode.
    /// If set, only streams with this key will be accepted.
    /// If None/empty, any stream key will be accepted.
    pub expected_stream_key: Option<String>,
}

#[derive(Debug, Clone)]
pub struct OutputGroupConfig {
    pub group_id: String,
    pub mode: OutputGroupMode,
    pub targets: Vec<String>,
    pub group: Option<OutputGroup>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputGroupMode {
    Passthrough,
    Transcode,
}

pub struct InputPipeline {
    input_id: String,
    input_url: String,
    expected_stream_key: Option<String>,
    groups: Vec<OutputGroupConfig>,
    stop_flag: Arc<AtomicBool>,
    thread: Option<JoinHandle<Result<(), String>>>,
    /// Command sender for runtime control (created on start)
    command_tx: Option<Sender<PipelineCommand>>,
    /// Handles for controlling individual groups
    group_handles: Arc<Mutex<HashMap<String, GroupHandle>>>,
    /// Event sink for emitting stats and lifecycle events
    event_sink: Option<Arc<dyn EventSink>>,
}

impl InputPipeline {
    pub fn new(config: InputPipelineConfig) -> Self {
        Self {
            input_id: config.input_id,
            input_url: config.input_url,
            expected_stream_key: config.expected_stream_key,
            groups: Vec::new(),
            stop_flag: Arc::new(AtomicBool::new(false)),
            thread: None,
            command_tx: None,
            group_handles: Arc::new(Mutex::new(HashMap::new())),
            event_sink: None,
        }
    }

    /// Set the event sink for emitting stats and lifecycle events
    pub fn set_event_sink(&mut self, sink: Arc<dyn EventSink>) {
        self.event_sink = Some(sink);
    }

    pub fn input_id(&self) -> &str {
        &self.input_id
    }

    pub fn add_group(&mut self, group: OutputGroup, targets: Vec<String>) -> Result<(), String> {
        let mode = if group.video.codec.eq_ignore_ascii_case("copy")
            && group.audio.codec.eq_ignore_ascii_case("copy") {
            OutputGroupMode::Passthrough
        } else {
            OutputGroupMode::Transcode
        };

        self.groups.push(OutputGroupConfig {
            group_id: group.id.clone(),
            mode,
            targets,
            group: Some(group),
        });

        Ok(())
    }

    pub fn add_group_config(&mut self, config: OutputGroupConfig) {
        self.groups.push(config);
    }

    pub fn start(&mut self) -> Result<(), String> {
        if self.thread.is_some() {
            return Err("FFmpeg libs pipeline already started".to_string());
        }

        let transcode_count = self.groups.iter()
            .filter(|group| group.mode == OutputGroupMode::Transcode)
            .count();
        if transcode_count > 1 {
            return Err("Only one transcode group is supported in ffmpeg-libs pipeline for now".to_string());
        }

        // Create command channel for runtime control
        let (command_tx, command_rx) = mpsc::channel();
        self.command_tx = Some(command_tx);

        // Create control handles for each group
        let mut group_controls = HashMap::new();
        {
            let mut handles = self.group_handles.lock()
                .map_err(|e| format!("Lock poisoned: {e}"))?;
            handles.clear();

            for group_config in &self.groups {
                let control = GroupControl::new();
                let handle = GroupHandle {
                    group_id: group_config.group_id.clone(),
                    mode: group_config.mode,
                    control: control.clone(),
                };
                handles.insert(group_config.group_id.clone(), handle);
                group_controls.insert(group_config.group_id.clone(), control);
            }
        }

        let input_url = self.input_url.clone();
        let expected_stream_key = self.expected_stream_key.clone();
        let stop_flag = Arc::clone(&self.stop_flag);
        let groups = self.groups.clone();
        let event_sink = self.event_sink.clone();

        let handle = thread::spawn(move || {
            run_pipeline_loop(
                &input_url,
                expected_stream_key.as_deref(),
                groups,
                group_controls,
                stop_flag,
                command_rx,
                event_sink,
            )
        });
        self.thread = Some(handle);
        Ok(())
    }

    /// Stop the entire pipeline (all groups and input)
    pub fn stop(&mut self) {
        self.stop_flag.store(true, Ordering::SeqCst);
    }

    /// Stop a specific group without stopping the pipeline
    pub fn stop_group(&self, group_id: &str) -> Result<(), String> {
        let handles = self.group_handles.lock()
            .map_err(|e| format!("Lock poisoned: {e}"))?;

        if let Some(handle) = handles.get(group_id) {
            handle.stop();
            Ok(())
        } else {
            Err(format!("Group not found: {}", group_id))
        }
    }

    /// Enable a specific group (resume packet writing)
    pub fn enable_group(&self, group_id: &str) -> Result<(), String> {
        let handles = self.group_handles.lock()
            .map_err(|e| format!("Lock poisoned: {e}"))?;

        if let Some(handle) = handles.get(group_id) {
            handle.enable();
            Ok(())
        } else {
            Err(format!("Group not found: {}", group_id))
        }
    }

    /// Disable a specific group (pause packet writing, keep connection)
    pub fn disable_group(&self, group_id: &str) -> Result<(), String> {
        let handles = self.group_handles.lock()
            .map_err(|e| format!("Lock poisoned: {e}"))?;

        if let Some(handle) = handles.get(group_id) {
            handle.disable();
            Ok(())
        } else {
            Err(format!("Group not found: {}", group_id))
        }
    }

    /// Get a handle to control a specific group
    pub fn get_group_handle(&self, group_id: &str) -> Option<GroupHandle> {
        let handles = self.group_handles.lock().ok()?;
        handles.get(group_id).cloned()
    }

    /// Get handles for all groups
    pub fn get_all_group_handles(&self) -> Vec<GroupHandle> {
        let handles = match self.group_handles.lock() {
            Ok(h) => h,
            Err(_) => return Vec::new(),
        };
        handles.values().cloned().collect()
    }

    /// Check if a specific group is running (not stopped)
    pub fn is_group_running(&self, group_id: &str) -> bool {
        let handles = match self.group_handles.lock() {
            Ok(h) => h,
            Err(_) => return false,
        };
        handles.get(group_id)
            .map(|h| !h.is_stopped())
            .unwrap_or(false)
    }

    pub fn join(&mut self) -> Result<(), String> {
        if let Some(handle) = self.thread.take() {
            handle.join().map_err(|_| "FFmpeg libs pipeline thread panicked".to_string())?
        } else {
            Ok(())
        }
    }
}

struct TargetOutput {
    ctx: *mut ffi::AVFormatContext,
    out_streams: Vec<*mut ffi::AVStream>,
    /// Error tracking and reconnection state
    state: TargetState,
    /// Bitstream filter for AAC ADTS to ASC conversion (required for FLV/RTMP)
    audio_bsf_ctx: Option<*mut ffi::AVBSFContext>,
    /// Index of audio stream in out_streams (for BSF application)
    audio_stream_index: Option<usize>,
}

struct GroupOutputs {
    group_id: String,
    control: GroupControl,
    targets: Vec<TargetOutput>,
    /// Reconnection configuration for this group
    reconnect_config: ReconnectionConfig,
    /// Queue configuration for this group
    queue_config: QueueConfig,
    /// Total dropped frames across all targets (for stats)
    dropped_frames: u64,
    /// Track if this group has been cleaned up
    cleaned_up: bool,
}

struct TranscodeGroup {
    group_id: String,
    control: GroupControl,
    video_stream_index: usize,
    audio_stream_index: Option<usize>,
    video_dec_ctx: *mut ffi::AVCodecContext,
    audio_dec_ctx: Option<*mut ffi::AVCodecContext>,
    video_enc_ctx: *mut ffi::AVCodecContext,
    audio_enc_ctx: Option<*mut ffi::AVCodecContext>,
    video_hw_device: Option<*mut ffi::AVBufferRef>,
    video_hw_frames_ctx: Option<*mut ffi::AVBufferRef>,
    /// Decoder hardware frames context (for zero-copy path)
    video_dec_hw_frames_ctx: Option<*mut ffi::AVBufferRef>,
    sws_ctx: *mut ffi::SwsContext,
    swr_ctx: Option<*mut ffi::SwrContext>,
    video_dec_frame: *mut ffi::AVFrame,
    video_sw_frame: *mut ffi::AVFrame,
    /// Software frame used to download hardware-decoded frames (input size)
    video_hw_sw_frame: Option<*mut ffi::AVFrame>,
    video_hw_frame: Option<*mut ffi::AVFrame>,
    audio_dec_frame: *mut ffi::AVFrame,
    outputs: Vec<TranscodeOutput>,
    /// True if using hardware decoder (zero-copy path enabled)
    using_hw_decode: bool,
    /// Track if this group has been cleaned up
    cleaned_up: bool,
}

struct TranscodeOutput {
    ctx: *mut ffi::AVFormatContext,
    video_out_index: i32,
    audio_out_index: Option<i32>,
    /// Bitstream filter for extradata (dump_extra for QSV encoders to FLV)
    video_bsf_ctx: Option<*mut ffi::AVBSFContext>,
}

fn parse_rtmp_listen_url(url: &str) -> (String, Option<String>, Option<String>) {
    if !(url.starts_with("rtmp://") || url.starts_with("rtmps://")) {
        return (url.to_string(), None, None);
    }

    let (without_query, query) = match url.split_once('?') {
        Some(parts) => parts,
        None => (url, ""),
    };

    let (scheme, rest) = match without_query.split_once("://") {
        Some(parts) => parts,
        None => return (url.to_string(), None, None),
    };

    let (host, path) = match rest.split_once('/') {
        Some(parts) => parts,
        None => {
            let base = if query.is_empty() {
                format!("{scheme}://{rest}")
            } else {
                format!("{scheme}://{rest}?{query}")
            };
            return (base, None, None);
        }
    };

    let segments: Vec<&str> = path.split('/').filter(|segment| !segment.is_empty()).collect();
    let app = segments.first().map(|segment| (*segment).to_string());
    let playpath = if segments.len() > 1 {
        Some(segments[1..].join("/"))
    } else {
        None
    };

    let base = if query.is_empty() {
        format!("{scheme}://{host}")
    } else {
        format!("{scheme}://{host}?{query}")
    };

    (base, app, playpath)
}

unsafe extern "C" fn should_interrupt(opaque: *mut c_void) -> c_int {
    if opaque.is_null() {
        return 0;
    }
    let flag = &*(opaque as *const AtomicBool);
    if flag.load(Ordering::SeqCst) {
        1
    } else {
        0
    }
}

fn run_pipeline_loop(
    input_url: &str,
    expected_stream_key: Option<&str>,
    groups: Vec<OutputGroupConfig>,
    group_controls: HashMap<String, GroupControl>,
    stop_flag: Arc<AtomicBool>,
    command_rx: Receiver<PipelineCommand>,
    event_sink: Option<Arc<dyn EventSink>>,
) -> Result<(), String> {
    unsafe {
        ffi::avformat_network_init();
    }

    // Initialize per-group stats trackers
    let mut stats_trackers: HashMap<String, GroupStatsTracker> = groups
        .iter()
        .map(|g| (g.group_id.clone(), GroupStatsTracker::new(g.group_id.clone())))
        .collect();

    let mut input_ctx = unsafe { ffi::avformat_alloc_context() };
    if input_ctx.is_null() {
        return Err("Failed to allocate input context".to_string());
    }

    unsafe {
        (*input_ctx).interrupt_callback = ffi::AVIOInterruptCB {
            callback: Some(should_interrupt),
            opaque: Arc::as_ptr(&stop_flag) as *mut c_void,
        };
    }
    let input_url = input_url.to_string();
    let mut open_url = input_url.clone();

    let mut opts: *mut ffi::AVDictionary = ptr::null_mut();
    if input_url.starts_with("rtmp://") || input_url.starts_with("rtmps://") {
        // Set listen mode - this makes FFmpeg act as an RTMP server
        let listen_key = CString::new("listen").unwrap_or_default();
        let listen_val = CString::new("1").unwrap_or_default();
        let rtmp_listen_key = CString::new("rtmp_listen").unwrap_or_default();
        unsafe {
            ffi::av_dict_set(&mut opts, listen_key.as_ptr(), listen_val.as_ptr(), 0);
            ffi::av_dict_set(&mut opts, rtmp_listen_key.as_ptr(), listen_val.as_ptr(), 0);
        }

        // Parse the URL so we can open on the base host and set app/playpath explicitly.
        let (base_url, app, _) = parse_rtmp_listen_url(&input_url);
        open_url = base_url.clone();

        if let Some(ref app_name) = app {
            let rtmp_app_key = CString::new("rtmp_app").unwrap_or_default();
            let rtmp_app_val = CString::new(app_name.as_str()).unwrap_or_default();
            unsafe {
                ffi::av_dict_set(&mut opts, rtmp_app_key.as_ptr(), rtmp_app_val.as_ptr(), 0);
            }
        }

        if let Some(key) = expected_stream_key {
            if !key.is_empty() {
                log::info!("RTMP listener expecting stream key (filtered mode)");
                let rtmp_playpath_key = CString::new("rtmp_playpath").unwrap_or_default();
                let rtmp_playpath_val = CString::new(key).unwrap_or_default();
                unsafe {
                    ffi::av_dict_set(&mut opts, rtmp_playpath_key.as_ptr(), rtmp_playpath_val.as_ptr(), 0);
                }
            } else {
                log::info!("RTMP listener accepting any stream key (permissive mode)");
            }
        } else {
            log::info!("RTMP listener accepting any stream key (permissive mode)");
        }
        log::debug!("RTMP listen base URL: {}", base_url);
    }

    let input_url_c = CString::new(open_url)
        .map_err(|_| "Input URL contains null byte".to_string())?;

    let open_ret = unsafe {
        ffi::avformat_open_input(
            &mut input_ctx,
            input_url_c.as_ptr(),
            ptr::null_mut(),
            &mut opts,
        )
    };
    unsafe { ffi::av_dict_free(&mut opts) };
    if open_ret < 0 {
        unsafe { ffi::avformat_free_context(input_ctx) };
        return Err(format!("Failed to open input: {}", ffmpeg_err(open_ret)));
    }

    let info_ret = unsafe { ffi::avformat_find_stream_info(input_ctx, ptr::null_mut()) };
    if info_ret < 0 {
        unsafe { ffi::avformat_close_input(&mut input_ctx) };
        return Err(format!("Failed to read stream info: {}", ffmpeg_err(info_ret)));
    }

    let (video_stream_index, audio_stream_index) = find_stream_indices(input_ctx)?;
    let mut group_outputs = create_group_outputs(input_ctx, &groups, &group_controls)?;
    let transcode_group_config = groups.iter()
        .find(|group| group.mode == OutputGroupMode::Transcode);
    let mut transcode_group = if let Some(config) = transcode_group_config {
        let control = group_controls.get(&config.group_id)
            .cloned()
            .unwrap_or_default();
        Some(create_transcode_group(
            input_ctx,
            config,
            control,
            video_stream_index,
            audio_stream_index,
        )?)
    } else {
        None
    };

    let mut packet = unsafe { ffi::av_packet_alloc() };
    if packet.is_null() {
        cleanup_outputs(&mut group_outputs);
        if let Some(group) = transcode_group.take() {
            cleanup_transcode_group(group);
        }
        unsafe { ffi::avformat_close_input(&mut input_ctx) };
        return Err("Failed to allocate AVPacket".to_string());
    }

    loop {
        // Check global stop flag
        if stop_flag.load(Ordering::SeqCst) {
            break;
        }

        // Process any pending commands (non-blocking)
        while let Ok(cmd) = command_rx.try_recv() {
            match cmd {
                PipelineCommand::StopGroup(group_id) => {
                    log::info!("Received command to stop group: {}", group_id);
                    // Mark the group as stopped via control flag
                    if let Some(control) = group_controls.get(&group_id) {
                        control.stop();
                    }
                }
                PipelineCommand::AddGroup(_config) => {
                    // Adding groups at runtime requires more complex handling
                    // For now, log and skip - groups must be added before start()
                    log::warn!("AddGroup command received but runtime group addition not yet supported");
                }
            }
        }

        // Check per-group stop flags and clean up stopped groups
        for group_out in group_outputs.iter_mut() {
            if !group_out.cleaned_up && group_out.control.should_stop() {
                log::info!("Cleaning up stopped passthrough group: {}", group_out.group_id);
                cleanup_single_passthrough_group(group_out);
                // Emit stream_ended event
                if let Some(ref sink) = event_sink {
                    emit_event(sink.as_ref(), "stream_ended", &group_out.group_id);
                }
            }
        }

        if let Some(ref mut tg) = transcode_group {
            if !tg.cleaned_up && tg.control.should_stop() {
                log::info!("Cleaning up stopped transcode group: {}", tg.group_id);
                flush_transcode_group(tg)?;
                cleanup_transcode_group_outputs(tg);
                tg.cleaned_up = true;
                // Emit stream_ended event
                if let Some(ref sink) = event_sink {
                    emit_event(sink.as_ref(), "stream_ended", &tg.group_id);
                }
            }
        }

        // Check if all groups are stopped - if so, exit the loop
        let all_passthrough_stopped = group_outputs.iter().all(|g| g.cleaned_up);
        let transcode_stopped = transcode_group.as_ref().map(|g| g.cleaned_up).unwrap_or(true);
        if all_passthrough_stopped && transcode_stopped {
            log::info!("All groups stopped, exiting pipeline loop");
            break;
        }

        let read_ret = unsafe { ffi::av_read_frame(input_ctx, packet) };
        if read_ret == ffi::AVERROR_EOF {
            break;
        }
        if read_ret < 0 {
            // Avoid tight loop on transient errors.
            std::thread::sleep(std::time::Duration::from_millis(5));
            continue;
        }

        let stream_index = unsafe { (*packet).stream_index as usize };
        let in_stream = unsafe { *(*input_ctx).streams.add(stream_index) };
        let codecpar = unsafe { (*in_stream).codecpar };
        let codec_type = unsafe { (*codecpar).codec_type };
        if codec_type != ffi::AVMediaType::AVMEDIA_TYPE_VIDEO && codec_type != ffi::AVMediaType::AVMEDIA_TYPE_AUDIO {
            unsafe { ffi::av_packet_unref(packet) };
            continue;
        }

        let packet_size = unsafe { (*packet).size as u64 };
        let is_video = stream_index == video_stream_index;

        if is_video {
            let failures = write_passthrough_packet(packet, in_stream, &mut group_outputs);
            // Emit stream_error events for newly failed targets
            if let Some(ref sink) = event_sink {
                for failure in failures {
                    emit_event(
                        sink.as_ref(),
                        "stream_error",
                        &serde_json::json!({
                            "groupId": failure.group_id,
                            "error": format!("Target {} failed: {}", failure.target_url, failure.error),
                            "canRetry": true,
                            "suggestion": "Stream connection lost. Will attempt automatic reconnection."
                        }),
                    );
                }
            }
            // Track stats for passthrough groups
            for group_out in &group_outputs {
                if !group_out.cleaned_up && group_out.control.is_enabled() {
                    if let Some(tracker) = stats_trackers.get_mut(&group_out.group_id) {
                        tracker.record_video_frame(packet_size);
                    }
                }
            }
            if let Some(group) = transcode_group.as_mut() {
                if !group.cleaned_up && group.control.is_enabled() {
                    transcode_video_packet(group, in_stream, packet)?;
                    // Track stats for transcode group
                    if let Some(tracker) = stats_trackers.get_mut(&group.group_id) {
                        tracker.record_video_frame(packet_size);
                    }
                }
            }
        } else if audio_stream_index == Some(stream_index) {
            let failures = write_passthrough_packet(packet, in_stream, &mut group_outputs);
            // Emit stream_error events for newly failed targets
            if let Some(ref sink) = event_sink {
                for failure in failures {
                    emit_event(
                        sink.as_ref(),
                        "stream_error",
                        &serde_json::json!({
                            "groupId": failure.group_id,
                            "error": format!("Target {} failed: {}", failure.target_url, failure.error),
                            "canRetry": true,
                            "suggestion": "Stream connection lost. Will attempt automatic reconnection."
                        }),
                    );
                }
            }
            // Track stats for passthrough groups
            for group_out in &group_outputs {
                if !group_out.cleaned_up && group_out.control.is_enabled() {
                    if let Some(tracker) = stats_trackers.get_mut(&group_out.group_id) {
                        tracker.record_audio_packet(packet_size);
                    }
                }
            }
            if let Some(group) = transcode_group.as_mut() {
                if !group.cleaned_up && group.control.is_enabled() {
                    transcode_audio_packet(group, in_stream, packet)?;
                    // Track stats for transcode group
                    if let Some(tracker) = stats_trackers.get_mut(&group.group_id) {
                        tracker.record_audio_packet(packet_size);
                    }
                }
            }
        }

        // Attempt reconnection for failed targets
        for group in &mut group_outputs {
            if group.cleaned_up {
                continue;
            }
            for target in &mut group.targets {
                if target.state.should_attempt_reconnect(&group.reconnect_config) {
                    target.state.start_reconnect();
                    log::info!(
                        "Attempting reconnection for target {} (attempt {}/{})",
                        target.state.url,
                        target.state.reconnect_attempt,
                        group.reconnect_config.max_retries
                    );

                    // Emit reconnecting event
                    if let Some(ref sink) = event_sink {
                        emit_event(
                            sink.as_ref(),
                            "stream_reconnecting",
                            &serde_json::json!({
                                "groupId": group.group_id,
                                "targetUrl": target.state.url,
                                "attempt": target.state.reconnect_attempt,
                                "maxAttempts": group.reconnect_config.max_retries,
                                "delaySecs": target.state.next_delay(&group.reconnect_config).as_secs()
                            }),
                        );
                    }

                    // Try to reconnect (recreate the output context)
                    match try_reconnect_target(input_ctx, &target.state.url) {
                        Ok(new_target) => {
                            log::info!("Reconnected target {}", target.state.url);
                            // Preserve the URL but update the connection
                            let url = target.state.url.clone();
                            *target = new_target;
                            target.state.url = url;
                            target.state.finish_reconnect(true);
                        }
                        Err(e) => {
                            log::warn!("Reconnection failed for {}: {}", target.state.url, e);
                            target.state.finish_reconnect(false);

                            // Check if we've exhausted retries
                            if target.state.reconnect_attempt >= group.reconnect_config.max_retries {
                                log::error!(
                                    "Target {} has exhausted all {} reconnection attempts",
                                    target.state.url,
                                    group.reconnect_config.max_retries
                                );
                                if let Some(ref sink) = event_sink {
                                    emit_event(
                                        sink.as_ref(),
                                        "stream_error",
                                        &serde_json::json!({
                                            "groupId": group.group_id,
                                            "error": format!("Target {} failed permanently after {} attempts", target.state.url, group.reconnect_config.max_retries),
                                            "canRetry": false,
                                            "suggestion": "Maximum reconnection attempts reached. Manual intervention required."
                                        }),
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }

        // Emit stats periodically for all active groups
        if let Some(ref sink) = event_sink {
            for (group_id, tracker) in stats_trackers.iter_mut() {
                // Check if this group is still active and get dropped frames count
                let group_info = group_outputs.iter()
                    .find(|g| &g.group_id == group_id && !g.cleaned_up)
                    .map(|g| (true, g.dropped_frames));

                let (is_active, dropped_frames) = match group_info {
                    Some((active, dropped)) => (active, dropped),
                    None => {
                        // Check transcode group
                        let transcode_active = transcode_group.as_ref()
                            .map(|g| &g.group_id == group_id && !g.cleaned_up)
                            .unwrap_or(false);
                        (transcode_active, 0) // Transcode group doesn't track dropped frames yet
                    }
                };

                if is_active && tracker.should_emit() {
                    let stats = tracker.emit_stats(dropped_frames);
                    emit_event(sink.as_ref(), "stream_stats", &stats);
                }
            }
        }

        unsafe { ffi::av_packet_unref(packet) };
    }

    // Final cleanup - flush and close any groups that weren't stopped individually
    if let Some(ref mut group) = transcode_group {
        if !group.cleaned_up {
            flush_transcode_group(group)?;
        }
    }

    // Emit final stream_ended events for groups that completed normally
    if let Some(ref sink) = event_sink {
        for group_out in &group_outputs {
            if !group_out.cleaned_up {
                emit_event(sink.as_ref(), "stream_ended", &group_out.group_id);
            }
        }
        if let Some(ref tg) = transcode_group {
            if !tg.cleaned_up {
                emit_event(sink.as_ref(), "stream_ended", &tg.group_id);
            }
        }
    }

    unsafe { ffi::av_packet_free(&mut packet) };
    cleanup_outputs(&mut group_outputs);
    if let Some(group) = transcode_group.take() {
        cleanup_transcode_group(group);
    }
    unsafe { ffi::avformat_close_input(&mut input_ctx) };

    Ok(())
}

fn create_group_outputs(
    input_ctx: *mut ffi::AVFormatContext,
    groups: &[OutputGroupConfig],
    group_controls: &HashMap<String, GroupControl>,
) -> Result<Vec<GroupOutputs>, String> {
    let mut outputs = Vec::new();
    for group in groups {
        if group.mode != OutputGroupMode::Passthrough {
            continue;
        }

        let mut targets = Vec::new();
        for target_url in &group.targets {
            let target = create_flv_output(input_ctx, target_url)?;
            targets.push(target);
        }

        let control = group_controls.get(&group.group_id)
            .cloned()
            .unwrap_or_default();

        outputs.push(GroupOutputs {
            group_id: group.group_id.clone(),
            control,
            targets,
            reconnect_config: ReconnectionConfig::default(),
            queue_config: QueueConfig::default(),
            dropped_frames: 0,
            cleaned_up: false,
        });
    }

    Ok(outputs)
}

fn find_stream_indices(input_ctx: *mut ffi::AVFormatContext) -> Result<(usize, Option<usize>), String> {
    let stream_count = unsafe { (*input_ctx).nb_streams as usize };
    let mut video_stream_index = None;
    let mut audio_stream_index = None;
    for idx in 0..stream_count {
        let stream = unsafe { *(*input_ctx).streams.add(idx) };
        let codecpar = unsafe { (*stream).codecpar };
        let codec_type = unsafe { (*codecpar).codec_type };
        if codec_type == ffi::AVMediaType::AVMEDIA_TYPE_VIDEO && video_stream_index.is_none() {
            video_stream_index = Some(idx);
        } else if codec_type == ffi::AVMediaType::AVMEDIA_TYPE_AUDIO && audio_stream_index.is_none() {
            audio_stream_index = Some(idx);
        }
    }

    let video_index = video_stream_index.ok_or_else(|| "No video stream found".to_string())?;
    Ok((video_index, audio_stream_index))
}

/// Add RTMP protocol options for connection resilience
/// These match OBS Studio's RTMP configuration for maximum stability
unsafe fn add_rtmp_options(opts: &mut *mut ffi::AVDictionary, url: &str) {
    if !url.starts_with("rtmp://") && !url.starts_with("rtmps://") {
        return;
    }

    // timeout: 30 seconds in microseconds (receive timeout)
    let key = CString::new("timeout").unwrap();
    let val = CString::new("30000000").unwrap();
    ffi::av_dict_set(opts, key.as_ptr(), val.as_ptr(), 0);

    // rtmp_buffer: 30 seconds in milliseconds (client buffer)
    let key = CString::new("rtmp_buffer").unwrap();
    let val = CString::new("30000").unwrap();
    ffi::av_dict_set(opts, key.as_ptr(), val.as_ptr(), 0);

    // tcp_keepalive: Enable TCP keepalive probes to detect dead connections
    let key = CString::new("tcp_keepalive").unwrap();
    let val = CString::new("1").unwrap();
    ffi::av_dict_set(opts, key.as_ptr(), val.as_ptr(), 0);

    // rtmp_live: Live stream mode (optimizes for live rather than VOD)
    let key = CString::new("rtmp_live").unwrap();
    let val = CString::new("live").unwrap();
    ffi::av_dict_set(opts, key.as_ptr(), val.as_ptr(), 0);

    log::debug!("RTMP output: added resilience options (timeout=30s, buffer=30s, keepalive=1, live=live)");
}

fn create_flv_output(
    input_ctx: *mut ffi::AVFormatContext,
    url: &str,
) -> Result<TargetOutput, String> {
    let mut output_ctx: *mut ffi::AVFormatContext = ptr::null_mut();
    let url_c = CString::new(url)
        .map_err(|_| "Output URL contains null byte".to_string())?;

    let alloc_ret = unsafe {
        ffi::avformat_alloc_output_context2(
            &mut output_ctx,
            ptr::null_mut(),
            CString::new("flv").unwrap().as_ptr(),
            url_c.as_ptr(),
        )
    };
    if alloc_ret < 0 || output_ctx.is_null() {
        return Err(format!("Failed to allocate output context: {}", ffmpeg_err(alloc_ret)));
    }

    let stream_count = unsafe { (*input_ctx).nb_streams as usize };
    let mut out_streams = vec![ptr::null_mut(); stream_count];
    let mut audio_stream_index: Option<usize> = None;
    let mut is_aac_audio = false;

    for idx in 0..stream_count {
        let in_stream = unsafe { *(*input_ctx).streams.add(idx) };
        let codecpar = unsafe { (*in_stream).codecpar };
        let codec_type = unsafe { (*codecpar).codec_type };
        if codec_type != ffi::AVMediaType::AVMEDIA_TYPE_VIDEO && codec_type != ffi::AVMediaType::AVMEDIA_TYPE_AUDIO {
            continue;
        }

        let out_stream = unsafe { ffi::avformat_new_stream(output_ctx, ptr::null()) };
        if out_stream.is_null() {
            unsafe { ffi::avformat_free_context(output_ctx) };
            return Err("Failed to create output stream".to_string());
        }

        let copy_ret = unsafe { ffi::avcodec_parameters_copy((*out_stream).codecpar, codecpar) };
        if copy_ret < 0 {
            unsafe { ffi::avformat_free_context(output_ctx) };
            return Err(format!("Failed to copy codec parameters: {}", ffmpeg_err(copy_ret)));
        }

        // Set FLV codec tags for RTMP compatibility
        // FLV uses specific tags: 7 for H.264 video, 10 for AAC audio
        unsafe {
            let out_codecpar = (*out_stream).codecpar;
            if codec_type == ffi::AVMediaType::AVMEDIA_TYPE_VIDEO {
                // Check if H.264 (AVC)
                if (*codecpar).codec_id == ffi::AVCodecID::AV_CODEC_ID_H264 {
                    (*out_codecpar).codec_tag = 7;
                }
            } else if codec_type == ffi::AVMediaType::AVMEDIA_TYPE_AUDIO {
                audio_stream_index = Some(idx);
                // Check if AAC
                if (*codecpar).codec_id == ffi::AVCodecID::AV_CODEC_ID_AAC {
                    (*out_codecpar).codec_tag = 10;
                    is_aac_audio = true;
                }
            }
            (*out_stream).time_base = (*in_stream).time_base;
        }
        out_streams[idx] = out_stream;
    }

    // Create aac_adtstoasc bitstream filter for AAC audio (converts ADTS to ASC for FLV)
    let audio_bsf_ctx = if is_aac_audio {
        if let Some(audio_idx) = audio_stream_index {
            let in_stream = unsafe { *(*input_ctx).streams.add(audio_idx) };
            match create_aac_bsf(unsafe { (*in_stream).codecpar }) {
                Ok(bsf) => Some(bsf),
                Err(e) => {
                    log::warn!("Failed to create aac_adtstoasc BSF, continuing without it: {}", e);
                    None
                }
            }
        } else {
            None
        }
    } else {
        None
    };

    let mut opts: *mut ffi::AVDictionary = ptr::null_mut();
    let flvflags = CString::new("no_duration_filesize").unwrap();
    unsafe {
        ffi::av_dict_set(&mut opts, CString::new("flvflags").unwrap().as_ptr(), flvflags.as_ptr(), 0);
        // Add RTMP resilience options for RTMP/RTMPS URLs
        add_rtmp_options(&mut opts, url);
    }

    let open_ret = unsafe {
        if (*(*output_ctx).oformat).flags & ffi::AVFMT_NOFILE == 0 {
            ffi::avio_open2(&mut (*output_ctx).pb, url_c.as_ptr(), ffi::AVIO_FLAG_WRITE, ptr::null_mut(), &mut opts)
        } else {
            0
        }
    };
    if open_ret < 0 {
        unsafe { ffi::avformat_free_context(output_ctx) };
        return Err(format!("Failed to open output: {}", ffmpeg_err(open_ret)));
    }

    let header_ret = unsafe { ffi::avformat_write_header(output_ctx, &mut opts) };
    if header_ret < 0 {
        unsafe {
            if (*(*output_ctx).oformat).flags & ffi::AVFMT_NOFILE == 0 {
                ffi::avio_closep(&mut (*output_ctx).pb);
            }
            ffi::avformat_free_context(output_ctx);
        }
        return Err(format!("Failed to write output header: {}", ffmpeg_err(header_ret)));
    }

    Ok(TargetOutput {
        ctx: output_ctx,
        out_streams,
        state: TargetState::new(url.to_string()),
        audio_bsf_ctx,
        audio_stream_index,
    })
}

/// Create an aac_adtstoasc bitstream filter for converting ADTS-wrapped AAC to ASC format
unsafe fn create_aac_bsf(codecpar: *const ffi::AVCodecParameters) -> Result<*mut ffi::AVBSFContext, String> {
    let filter_name = CString::new("aac_adtstoasc").unwrap();
    let filter = ffi::av_bsf_get_by_name(filter_name.as_ptr());
    if filter.is_null() {
        return Err("aac_adtstoasc filter not found".to_string());
    }

    let mut bsf_ctx: *mut ffi::AVBSFContext = ptr::null_mut();
    let alloc_ret = ffi::av_bsf_alloc(filter, &mut bsf_ctx);
    if alloc_ret < 0 {
        return Err(format!("Failed to allocate BSF context: {}", ffmpeg_err(alloc_ret)));
    }

    let copy_ret = ffi::avcodec_parameters_copy((*bsf_ctx).par_in, codecpar);
    if copy_ret < 0 {
        ffi::av_bsf_free(&mut bsf_ctx);
        return Err(format!("Failed to copy BSF input params: {}", ffmpeg_err(copy_ret)));
    }

    let init_ret = ffi::av_bsf_init(bsf_ctx);
    if init_ret < 0 {
        ffi::av_bsf_free(&mut bsf_ctx);
        return Err(format!("Failed to init BSF: {}", ffmpeg_err(init_ret)));
    }

    log::debug!("Created aac_adtstoasc bitstream filter for AAC passthrough");
    Ok(bsf_ctx)
}

/// Create a dump_extra bitstream filter for ensuring SPS/PPS NAL units in each packet
/// Required for QSV encoders outputting to FLV/RTMP to allow mid-stream joins
unsafe fn create_dump_extra_bsf(codecpar: *const ffi::AVCodecParameters) -> Result<*mut ffi::AVBSFContext, String> {
    let filter_name = CString::new("dump_extra").unwrap();
    let filter = ffi::av_bsf_get_by_name(filter_name.as_ptr());
    if filter.is_null() {
        return Err("dump_extra filter not found".to_string());
    }

    let mut bsf_ctx: *mut ffi::AVBSFContext = ptr::null_mut();
    let alloc_ret = ffi::av_bsf_alloc(filter, &mut bsf_ctx);
    if alloc_ret < 0 {
        return Err(format!("Failed to allocate BSF context: {}", ffmpeg_err(alloc_ret)));
    }

    let copy_ret = ffi::avcodec_parameters_copy((*bsf_ctx).par_in, codecpar);
    if copy_ret < 0 {
        ffi::av_bsf_free(&mut bsf_ctx);
        return Err(format!("Failed to copy BSF input params: {}", ffmpeg_err(copy_ret)));
    }

    let init_ret = ffi::av_bsf_init(bsf_ctx);
    if init_ret < 0 {
        ffi::av_bsf_free(&mut bsf_ctx);
        return Err(format!("Failed to init BSF: {}", ffmpeg_err(init_ret)));
    }

    log::debug!("Created dump_extra bitstream filter for QSV video");
    Ok(bsf_ctx)
}

/// Attempt to reconnect a failed target by creating a new output context
fn try_reconnect_target(
    input_ctx: *mut ffi::AVFormatContext,
    url: &str,
) -> Result<TargetOutput, String> {
    // This reuses create_flv_output which will create a fresh connection
    create_flv_output(input_ctx, url)
}

/// Information about a target that just failed
struct TargetFailure {
    group_id: String,
    target_url: String,
    error: String,
}

fn write_passthrough_packet(
    packet: *mut ffi::AVPacket,
    in_stream: *mut ffi::AVStream,
    group_outputs: &mut [GroupOutputs],
) -> Vec<TargetFailure> {
    let mut failures = Vec::new();

    for group in group_outputs.iter_mut() {
        // Skip groups that are stopped or disabled
        if group.cleaned_up || !group.control.is_enabled() {
            continue;
        }

        let threshold = group.reconnect_config.failure_threshold;

        for target in &mut group.targets {
            // Skip failed targets (they need reconnection) - count as dropped frame
            if target.state.is_failed {
                group.dropped_frames += 1;
                continue;
            }

            let stream_index = unsafe { (*packet).stream_index as usize };
            if stream_index >= target.out_streams.len() {
                continue;
            }

            let out_stream = target.out_streams[stream_index];
            if out_stream.is_null() {
                continue;
            }

            let mut pkt_clone = unsafe { ffi::av_packet_clone(packet) };
            if pkt_clone.is_null() {
                continue;
            }

            // Check if this is an audio packet that needs BSF filtering
            let is_audio_packet = target.audio_stream_index == Some(stream_index);
            let needs_bsf = is_audio_packet && target.audio_bsf_ctx.is_some();

            unsafe {
                if needs_bsf {
                    // Apply aac_adtstoasc bitstream filter
                    let bsf_ctx = target.audio_bsf_ctx.unwrap();
                    let send_ret = ffi::av_bsf_send_packet(bsf_ctx, pkt_clone);
                    if send_ret < 0 {
                        log::warn!("BSF send failed: {}", ffmpeg_err(send_ret));
                        ffi::av_packet_free(&mut pkt_clone);
                        continue;
                    }

                    // Receive and write filtered packets
                    loop {
                        let mut filtered_pkt = ffi::av_packet_alloc();
                        if filtered_pkt.is_null() {
                            break;
                        }
                        let recv_ret = ffi::av_bsf_receive_packet(bsf_ctx, filtered_pkt);
                        if recv_ret < 0 {
                            ffi::av_packet_free(&mut filtered_pkt);
                            break;
                        }

                        ffi::av_packet_rescale_ts(filtered_pkt, (*in_stream).time_base, (*out_stream).time_base);
                        (*filtered_pkt).stream_index = (*out_stream).index;
                        let write_ret = ffi::av_interleaved_write_frame(target.ctx, filtered_pkt);
                        if write_ret < 0 {
                            let error = ffmpeg_err(write_ret);
                            log::warn!(
                                "FFmpeg libs write failed for group {}: {}",
                                group.group_id,
                                error
                            );
                            if target.state.record_failure(threshold) {
                                failures.push(TargetFailure {
                                    group_id: group.group_id.clone(),
                                    target_url: target.state.url.clone(),
                                    error,
                                });
                            }
                        } else {
                            target.state.record_success();
                        }
                        ffi::av_packet_free(&mut filtered_pkt);
                    }
                    ffi::av_packet_free(&mut pkt_clone);
                } else {
                    // No BSF needed, write directly
                    ffi::av_packet_rescale_ts(pkt_clone, (*in_stream).time_base, (*out_stream).time_base);
                    (*pkt_clone).stream_index = (*out_stream).index;
                    let write_ret = ffi::av_interleaved_write_frame(target.ctx, pkt_clone);
                    if write_ret < 0 {
                        let error = ffmpeg_err(write_ret);
                        log::warn!(
                            "FFmpeg libs write failed for group {}: {}",
                            group.group_id,
                            error
                        );
                        // Track failure and check if target should be marked as failed
                        if target.state.record_failure(threshold) {
                            failures.push(TargetFailure {
                                group_id: group.group_id.clone(),
                                target_url: target.state.url.clone(),
                                error,
                            });
                        }
                    } else {
                        // Successful write - reset failure counter
                        target.state.record_success();
                    }
                    ffi::av_packet_free(&mut pkt_clone);
                }
            }
        }
    }

    failures
}

fn create_transcode_group(
    input_ctx: *mut ffi::AVFormatContext,
    config: &OutputGroupConfig,
    control: GroupControl,
    video_stream_index: usize,
    audio_stream_index: Option<usize>,
) -> Result<TranscodeGroup, String> {
    let group = config.group.as_ref()
        .ok_or_else(|| "Transcode group settings are missing".to_string())?;

    let video_in_stream = unsafe { *(*input_ctx).streams.add(video_stream_index) };
    let video_codecpar = unsafe { (*video_in_stream).codecpar };
    let input_width = unsafe { (*video_codecpar).width };
    let input_height = unsafe { (*video_codecpar).height };

    let video_encoder_name = CString::new(group.video.codec.clone())
        .map_err(|_| "Video codec contains null byte".to_string())?;
    let video_encoder = unsafe { ffi::avcodec_find_encoder_by_name(video_encoder_name.as_ptr()) };
    if video_encoder.is_null() {
        return Err(format!("Video encoder not found: {}", group.video.codec));
    }

    let video_enc_ctx = unsafe { ffi::avcodec_alloc_context3(video_encoder) };
    if video_enc_ctx.is_null() {
        return Err("Failed to allocate video encoder context".to_string());
    }

    let input_fps = unsafe {
        let fr = (*video_in_stream).avg_frame_rate;
        if fr.num > 0 && fr.den > 0 {
            fr
        } else {
            ffi::AVRational { num: 30, den: 1 }
        }
    };
    let output_fps = if group.video.fps > 0 {
        ffi::AVRational { num: group.video.fps as i32, den: 1 }
    } else {
        input_fps
    };
    let output_width = if group.video.width > 0 { group.video.width as i32 } else { input_width };
    let output_height = if group.video.height > 0 { group.video.height as i32 } else { input_height };

    let prefer_hw = is_hw_encoder(&group.video.codec);
    let sw_pix_fmt = select_pix_fmt(video_encoder, prefer_hw);
    let mut video_hw_device = attach_hw_device(&group.video.codec, video_enc_ctx);
    let hw_pix_fmt = if video_hw_device.is_some() {
        hw_pix_fmt_for_encoder(&group.video.codec)
    } else {
        None
    };
    let mut video_hw_frames_ctx = None;
    let mut video_hw_frame = None;
    let enc_pix_fmt = hw_pix_fmt.unwrap_or(sw_pix_fmt);
    unsafe {
        (*video_enc_ctx).width = output_width;
        (*video_enc_ctx).height = output_height;
        (*video_enc_ctx).time_base = ffi::AVRational { num: output_fps.den, den: output_fps.num };
        (*video_enc_ctx).framerate = output_fps;
        (*video_enc_ctx).pix_fmt = enc_pix_fmt;
        if let Some(bit_rate) = parse_bitrate_to_bits(&group.video.bitrate) {
            (*video_enc_ctx).bit_rate = bit_rate;
            // CBR enforcement: set min/max rate equal to target bitrate
            // Buffer size is 2x bitrate (2 seconds of buffer at target rate)
            (*video_enc_ctx).rc_min_rate = bit_rate;
            (*video_enc_ctx).rc_max_rate = bit_rate;
            (*video_enc_ctx).rc_buffer_size = (bit_rate * 2) as i32;
        }
        if let Some(interval) = group.video.keyframe_interval_seconds {
            if output_fps.num > 0 {
                let gop_size = (output_fps.num as i32).saturating_mul(interval as i32);
                (*video_enc_ctx).gop_size = gop_size;

                // For software encoders (libx264/libx265), set additional keyframe options
                // to ensure consistent keyframe placement (matches CLI behavior)
                let codec_lower = group.video.codec.to_ascii_lowercase();
                if codec_lower == "libx264" || codec_lower == "libx265" {
                    // keyint_min ensures minimum GOP size (prevents scene-detect early keyframes)
                    (*video_enc_ctx).keyint_min = gop_size;

                    // Disable scene change detection for consistent keyframe intervals
                    // This is set via x264-params/x265-params in apply_encoder_options
                }
            }
        }
    }

    // Detect if any target is Twitch (for QSV overrides)
    let is_twitch = targets_contain_twitch(&config.targets);

    // Apply encoder options (preset, profile, Twitch-safe settings)
    unsafe {
        apply_encoder_options(
            video_enc_ctx,
            &group.video.codec,
            group.video.preset.as_deref(),
            group.video.profile.as_deref(),
            is_twitch,
        );
    }

    // Try to create hardware frames context - some encoders (like AMF) may fail here
    // but can still work with just the device context and software frames
    if let (Some(device_ref), Some(hw_fmt)) = (video_hw_device, hw_pix_fmt) {
        match create_hw_frames_ctx(
            device_ref,
            hw_fmt,
            sw_pix_fmt,
            output_width,
            output_height,
        ) {
            Ok(mut frames_ctx) => {
                let frames_ref = unsafe { ffi::av_buffer_ref(frames_ctx) };
                if frames_ref.is_null() {
                    log::warn!("Failed to reference hardware frames context, falling back to device-only mode");
                    unsafe {
                        ffi::av_buffer_unref(&mut frames_ctx);
                        // Use software pixel format when falling back
                        (*video_enc_ctx).pix_fmt = sw_pix_fmt;
                    }
                } else {
                    unsafe {
                        (*video_enc_ctx).hw_frames_ctx = frames_ref;
                    }
                    video_hw_frames_ctx = Some(frames_ctx);
                    let hw_frame = unsafe { ffi::av_frame_alloc() };
                    if !hw_frame.is_null() {
                        video_hw_frame = Some(hw_frame);
                    }
                }
            }
            Err(err) => {
                // AMF and some other encoders may fail to create frames context but still work
                // with device context only - the encoder will handle frame upload internally
                log::warn!("Hardware frames context creation failed: {}. Continuing with device-only mode.", err);
                // Use software pixel format when falling back
                unsafe {
                    (*video_enc_ctx).pix_fmt = sw_pix_fmt;
                }
            }
        };
    }

    let mut open_enc_ret = unsafe { ffi::avcodec_open2(video_enc_ctx, video_encoder, ptr::null_mut()) };
    if open_enc_ret < 0 && video_hw_device.is_some() {
        unsafe {
            if let Some(mut device_ref) = video_hw_device.take() {
                ffi::av_buffer_unref(&mut device_ref);
            }
            if let Some(mut frames_ref) = video_hw_frames_ctx.take() {
                ffi::av_buffer_unref(&mut frames_ref);
            }
            if let Some(mut hw_frame) = video_hw_frame.take() {
                ffi::av_frame_free(&mut (hw_frame as *mut _));
            }
            if !(*video_enc_ctx).hw_device_ctx.is_null() {
                ffi::av_buffer_unref(&mut (*video_enc_ctx).hw_device_ctx);
            }
            if !(*video_enc_ctx).hw_frames_ctx.is_null() {
                ffi::av_buffer_unref(&mut (*video_enc_ctx).hw_frames_ctx);
            }
            (*video_enc_ctx).hw_device_ctx = ptr::null_mut();
            (*video_enc_ctx).hw_frames_ctx = ptr::null_mut();
            (*video_enc_ctx).pix_fmt = sw_pix_fmt;
        }
        open_enc_ret = unsafe { ffi::avcodec_open2(video_enc_ctx, video_encoder, ptr::null_mut()) };
    }
    if open_enc_ret < 0 {
        unsafe {
            if let Some(mut hw_frame) = video_hw_frame {
                ffi::av_frame_free(&mut (hw_frame as *mut _));
            }
            if let Some(mut frames_ref) = video_hw_frames_ctx {
                ffi::av_buffer_unref(&mut frames_ref);
            }
            if let Some(mut device_ref) = video_hw_device {
                ffi::av_buffer_unref(&mut device_ref);
            }
            ffi::avcodec_free_context(&mut (video_enc_ctx as *mut _));
        }
        return Err(format!("Failed to open video encoder: {}", ffmpeg_err(open_enc_ret)));
    }

    // Initialize decoder (hardware if possible, fallback to software)
    let mut video_dec_ctx: *mut ffi::AVCodecContext = ptr::null_mut();
    let mut using_hw_decode = false;
    if prefer_hw {
        if let Some(device_ref) = video_hw_device {
            match unsafe { try_init_hw_decoder(&group.video.codec, video_codecpar, device_ref) } {
                Ok((dec_ctx, is_hw)) => {
                    video_dec_ctx = dec_ctx;
                    using_hw_decode = is_hw;
                }
                Err(err) => {
                    log::warn!("Hardware decoder init failed, falling back to software decode: {}", err);
                }
            }
        }
    }

    if video_dec_ctx.is_null() {
        let video_decoder = unsafe { ffi::avcodec_find_decoder((*video_codecpar).codec_id) };
        if video_decoder.is_null() {
            unsafe {
                if let Some(mut hw_frame) = video_hw_frame {
                    ffi::av_frame_free(&mut (hw_frame as *mut _));
                }
                if let Some(mut frames_ref) = video_hw_frames_ctx {
                    ffi::av_buffer_unref(&mut frames_ref);
                }
                if let Some(mut device_ref) = video_hw_device {
                    ffi::av_buffer_unref(&mut device_ref);
                }
                ffi::avcodec_free_context(&mut (video_enc_ctx as *mut _));
            }
            return Err("Failed to find video decoder".to_string());
        }

        let dec_ctx = unsafe { ffi::avcodec_alloc_context3(video_decoder) };
        if dec_ctx.is_null() {
            unsafe {
                if let Some(mut hw_frame) = video_hw_frame {
                    ffi::av_frame_free(&mut (hw_frame as *mut _));
                }
                if let Some(mut frames_ref) = video_hw_frames_ctx {
                    ffi::av_buffer_unref(&mut frames_ref);
                }
                if let Some(mut device_ref) = video_hw_device {
                    ffi::av_buffer_unref(&mut device_ref);
                }
                ffi::avcodec_free_context(&mut (video_enc_ctx as *mut _));
            }
            return Err("Failed to allocate video decoder context".to_string());
        }

        let dec_ret = unsafe { ffi::avcodec_parameters_to_context(dec_ctx, video_codecpar) };
        if dec_ret < 0 {
            unsafe {
                ffi::avcodec_free_context(&mut (dec_ctx as *mut _));
                if let Some(mut hw_frame) = video_hw_frame {
                    ffi::av_frame_free(&mut (hw_frame as *mut _));
                }
                if let Some(mut frames_ref) = video_hw_frames_ctx {
                    ffi::av_buffer_unref(&mut frames_ref);
                }
                if let Some(mut device_ref) = video_hw_device {
                    ffi::av_buffer_unref(&mut device_ref);
                }
                ffi::avcodec_free_context(&mut (video_enc_ctx as *mut _));
            }
            return Err(format!("Failed to copy video decoder parameters: {}", ffmpeg_err(dec_ret)));
        }

        let open_dec_ret = unsafe { ffi::avcodec_open2(dec_ctx, video_decoder, ptr::null_mut()) };
        if open_dec_ret < 0 {
            unsafe {
                ffi::avcodec_free_context(&mut (dec_ctx as *mut _));
                if let Some(mut hw_frame) = video_hw_frame {
                    ffi::av_frame_free(&mut (hw_frame as *mut _));
                }
                if let Some(mut frames_ref) = video_hw_frames_ctx {
                    ffi::av_buffer_unref(&mut frames_ref);
                }
                if let Some(mut device_ref) = video_hw_device {
                    ffi::av_buffer_unref(&mut device_ref);
                }
                ffi::avcodec_free_context(&mut (video_enc_ctx as *mut _));
            }
            return Err(format!("Failed to open video decoder: {}", ffmpeg_err(open_dec_ret)));
        }

        video_dec_ctx = dec_ctx;
        using_hw_decode = false;
    }

    let sws_input_fmt = if using_hw_decode {
        sw_pix_fmt
    } else {
        unsafe { (*video_dec_ctx).pix_fmt }
    };

    let sws_ctx = unsafe {
        ffi::sws_getContext(
            (*video_dec_ctx).width,
            (*video_dec_ctx).height,
            sws_input_fmt,
            output_width,
            output_height,
            sw_pix_fmt,
            ffi::SwsFlags::SWS_BILINEAR as i32,
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null(),
        )
    };
    if sws_ctx.is_null() {
        unsafe {
            ffi::avcodec_free_context(&mut (video_enc_ctx as *mut _));
            ffi::avcodec_free_context(&mut (video_dec_ctx as *mut _));
        }
        return Err("Failed to create sws context".to_string());
    }

    let video_dec_frame = unsafe { ffi::av_frame_alloc() };
    let video_sw_frame = unsafe { ffi::av_frame_alloc() };
    if video_dec_frame.is_null() || video_sw_frame.is_null() {
        unsafe {
            if !video_dec_frame.is_null() {
                ffi::av_frame_free(&mut (video_dec_frame as *mut _));
            }
            if !video_sw_frame.is_null() {
                ffi::av_frame_free(&mut (video_sw_frame as *mut _));
            }
            if let Some(mut hw_frame) = video_hw_frame {
                ffi::av_frame_free(&mut (hw_frame as *mut _));
            }
            if let Some(mut frames_ref) = video_hw_frames_ctx {
                ffi::av_buffer_unref(&mut frames_ref);
            }
            if let Some(mut device_ref) = video_hw_device {
                ffi::av_buffer_unref(&mut device_ref);
            }
            ffi::sws_freeContext(sws_ctx);
            ffi::avcodec_free_context(&mut (video_enc_ctx as *mut _));
            ffi::avcodec_free_context(&mut (video_dec_ctx as *mut _));
        }
        return Err("Failed to allocate video frames".to_string());
    }

    let mut video_hw_sw_frame: Option<*mut ffi::AVFrame> = None;
    unsafe {
        (*video_sw_frame).format = sw_pix_fmt as i32;
        (*video_sw_frame).width = output_width;
        (*video_sw_frame).height = output_height;
        let buffer_ret = ffi::av_frame_get_buffer(video_sw_frame, 32);
        if buffer_ret < 0 {
            ffi::av_frame_free(&mut (video_sw_frame as *mut _));
            ffi::av_frame_free(&mut (video_dec_frame as *mut _));
            if let Some(mut hw_frame) = video_hw_frame {
                ffi::av_frame_free(&mut (hw_frame as *mut _));
            }
            if let Some(mut frames_ref) = video_hw_frames_ctx {
                ffi::av_buffer_unref(&mut frames_ref);
            }
            if let Some(mut device_ref) = video_hw_device {
                ffi::av_buffer_unref(&mut device_ref);
            }
            ffi::sws_freeContext(sws_ctx);
            ffi::avcodec_free_context(&mut (video_enc_ctx as *mut _));
            ffi::avcodec_free_context(&mut (video_dec_ctx as *mut _));
            return Err(format!("Failed to allocate video frame buffer: {}", ffmpeg_err(buffer_ret)));
        }
    }

    if using_hw_decode {
        let hw_sw_frame = unsafe { ffi::av_frame_alloc() };
        if hw_sw_frame.is_null() {
            unsafe {
                ffi::av_frame_free(&mut (video_sw_frame as *mut _));
                ffi::av_frame_free(&mut (video_dec_frame as *mut _));
                if let Some(mut hw_frame) = video_hw_frame {
                    ffi::av_frame_free(&mut (hw_frame as *mut _));
                }
                if let Some(mut frames_ref) = video_hw_frames_ctx {
                    ffi::av_buffer_unref(&mut frames_ref);
                }
                if let Some(mut device_ref) = video_hw_device {
                    ffi::av_buffer_unref(&mut device_ref);
                }
                ffi::sws_freeContext(sws_ctx);
                ffi::avcodec_free_context(&mut (video_enc_ctx as *mut _));
                ffi::avcodec_free_context(&mut (video_dec_ctx as *mut _));
            }
            return Err("Failed to allocate hardware download frame".to_string());
        }
        unsafe {
            (*hw_sw_frame).format = sw_pix_fmt as i32;
            (*hw_sw_frame).width = input_width;
            (*hw_sw_frame).height = input_height;
            let buffer_ret = ffi::av_frame_get_buffer(hw_sw_frame, 32);
            if buffer_ret < 0 {
                ffi::av_frame_free(&mut (hw_sw_frame as *mut _));
                ffi::av_frame_free(&mut (video_sw_frame as *mut _));
                ffi::av_frame_free(&mut (video_dec_frame as *mut _));
                if let Some(mut hw_frame) = video_hw_frame {
                    ffi::av_frame_free(&mut (hw_frame as *mut _));
                }
                if let Some(mut frames_ref) = video_hw_frames_ctx {
                    ffi::av_buffer_unref(&mut frames_ref);
                }
                if let Some(mut device_ref) = video_hw_device {
                    ffi::av_buffer_unref(&mut device_ref);
                }
                ffi::sws_freeContext(sws_ctx);
                ffi::avcodec_free_context(&mut (video_enc_ctx as *mut _));
                ffi::avcodec_free_context(&mut (video_dec_ctx as *mut _));
                return Err(format!("Failed to allocate hw download frame buffer: {}", ffmpeg_err(buffer_ret)));
            }
        }
        video_hw_sw_frame = Some(hw_sw_frame);
    }

    let (audio_dec_ctx, audio_enc_ctx, swr_ctx, audio_dec_frame) = if let Some(audio_index) = audio_stream_index {
        let audio_in_stream = unsafe { *(*input_ctx).streams.add(audio_index) };
        let audio_codecpar = unsafe { (*audio_in_stream).codecpar };
        if group.audio.codec.eq_ignore_ascii_case("copy") {
            let (is_aac, is_mp3) = unsafe {
                (
                    (*audio_codecpar).codec_id == ffi::AVCodecID::AV_CODEC_ID_AAC,
                    (*audio_codecpar).codec_id == ffi::AVCodecID::AV_CODEC_ID_MP3,
                )
            };
            if !is_aac && !is_mp3 {
                return Err("Audio copy requires AAC or MP3 input".to_string());
            }
            (None, None, None, ptr::null_mut())
        } else {
            let audio_decoder = unsafe { ffi::avcodec_find_decoder((*audio_codecpar).codec_id) };
            if audio_decoder.is_null() {
                return Err("Failed to find audio decoder".to_string());
            }

            let audio_dec_ctx = unsafe { ffi::avcodec_alloc_context3(audio_decoder) };
            if audio_dec_ctx.is_null() {
                return Err("Failed to allocate audio decoder context".to_string());
            }

            let dec_ret = unsafe { ffi::avcodec_parameters_to_context(audio_dec_ctx, audio_codecpar) };
            if dec_ret < 0 {
                unsafe { ffi::avcodec_free_context(&mut (audio_dec_ctx as *mut _)) };
                return Err(format!("Failed to copy audio decoder parameters: {}", ffmpeg_err(dec_ret)));
            }

            let open_dec_ret = unsafe { ffi::avcodec_open2(audio_dec_ctx, audio_decoder, ptr::null_mut()) };
            if open_dec_ret < 0 {
                unsafe { ffi::avcodec_free_context(&mut (audio_dec_ctx as *mut _)) };
                return Err(format!("Failed to open audio decoder: {}", ffmpeg_err(open_dec_ret)));
            }

            let audio_encoder_name = CString::new(group.audio.codec.clone())
                .map_err(|_| "Audio codec contains null byte".to_string())?;
            let audio_encoder = unsafe { ffi::avcodec_find_encoder_by_name(audio_encoder_name.as_ptr()) };
            if audio_encoder.is_null() {
                unsafe { ffi::avcodec_free_context(&mut (audio_dec_ctx as *mut _)) };
                return Err(format!("Audio encoder not found: {}", group.audio.codec));
            }

            let audio_enc_ctx = unsafe { ffi::avcodec_alloc_context3(audio_encoder) };
            if audio_enc_ctx.is_null() {
                unsafe { ffi::avcodec_free_context(&mut (audio_dec_ctx as *mut _)) };
                return Err("Failed to allocate audio encoder context".to_string());
            }

            let output_sample_rate = if group.audio.sample_rate > 0 {
                group.audio.sample_rate as i32
            } else {
                unsafe { (*audio_dec_ctx).sample_rate }
            };
            let output_channels = if group.audio.channels > 0 {
                group.audio.channels as i32
            } else {
                unsafe { (*audio_dec_ctx).ch_layout.nb_channels }
            };

            unsafe {
                ffi::av_channel_layout_default(&mut (*audio_enc_ctx).ch_layout, output_channels);
                (*audio_enc_ctx).sample_rate = output_sample_rate;
                (*audio_enc_ctx).time_base = ffi::AVRational { num: 1, den: output_sample_rate };
                (*audio_enc_ctx).sample_fmt = select_sample_fmt(audio_encoder, ffi::AVSampleFormat::AV_SAMPLE_FMT_FLTP);
                if let Some(bit_rate) = parse_bitrate_to_bits(&group.audio.bitrate) {
                    (*audio_enc_ctx).bit_rate = bit_rate;
                }
            }

            let open_enc_ret = unsafe { ffi::avcodec_open2(audio_enc_ctx, audio_encoder, ptr::null_mut()) };
            if open_enc_ret < 0 {
                unsafe {
                    ffi::avcodec_free_context(&mut (audio_enc_ctx as *mut _));
                    ffi::avcodec_free_context(&mut (audio_dec_ctx as *mut _));
                }
                return Err(format!("Failed to open audio encoder: {}", ffmpeg_err(open_enc_ret)));
            }

            let mut swr_ctx: *mut ffi::SwrContext = ptr::null_mut();
            let swr_ret = unsafe {
                ffi::swr_alloc_set_opts2(
                    &mut swr_ctx,
                    &(*audio_enc_ctx).ch_layout,
                    (*audio_enc_ctx).sample_fmt,
                    (*audio_enc_ctx).sample_rate,
                    &(*audio_dec_ctx).ch_layout,
                    (*audio_dec_ctx).sample_fmt,
                    (*audio_dec_ctx).sample_rate,
                    0,
                    ptr::null_mut(),
                )
            };
            if swr_ret < 0 || swr_ctx.is_null() {
                unsafe {
                    ffi::avcodec_free_context(&mut (audio_enc_ctx as *mut _));
                    ffi::avcodec_free_context(&mut (audio_dec_ctx as *mut _));
                }
                return Err(format!("Failed to allocate swr context: {}", ffmpeg_err(swr_ret)));
            }
            let swr_init_ret = unsafe { ffi::swr_init(swr_ctx) };
            if swr_init_ret < 0 {
                unsafe {
                    ffi::swr_free(&mut swr_ctx);
                    ffi::avcodec_free_context(&mut (audio_enc_ctx as *mut _));
                    ffi::avcodec_free_context(&mut (audio_dec_ctx as *mut _));
                }
                return Err(format!("Failed to init swr: {}", ffmpeg_err(swr_init_ret)));
            }

            let audio_dec_frame = unsafe { ffi::av_frame_alloc() };
            if audio_dec_frame.is_null() {
                unsafe {
                    ffi::swr_free(&mut swr_ctx);
                    ffi::avcodec_free_context(&mut (audio_enc_ctx as *mut _));
                    ffi::avcodec_free_context(&mut (audio_dec_ctx as *mut _));
                }
                return Err("Failed to allocate audio frame".to_string());
            }

            (Some(audio_dec_ctx), Some(audio_enc_ctx), Some(swr_ctx), audio_dec_frame)
        }
    } else {
        (None, None, None, ptr::null_mut())
    };

    let outputs = create_transcode_outputs(
        input_ctx,
        group,
        video_enc_ctx,
        audio_enc_ctx,
        audio_stream_index,
        &config.targets,
    )?;

    Ok(TranscodeGroup {
        group_id: config.group_id.clone(),
        control,
        video_stream_index,
        audio_stream_index,
        video_dec_ctx,
        audio_dec_ctx,
        video_enc_ctx,
        audio_enc_ctx,
        video_hw_device,
        video_hw_frames_ctx,
        video_dec_hw_frames_ctx: None, // TODO: Set when hw decode is enabled
        sws_ctx,
        swr_ctx,
        video_dec_frame,
        video_sw_frame,
        video_hw_sw_frame,
        video_hw_frame,
        audio_dec_frame,
        outputs,
        using_hw_decode,
        cleaned_up: false,
    })
}

fn create_transcode_outputs(
    input_ctx: *mut ffi::AVFormatContext,
    group: &OutputGroup,
    video_enc_ctx: *mut ffi::AVCodecContext,
    audio_enc_ctx: Option<*mut ffi::AVCodecContext>,
    audio_stream_index: Option<usize>,
    targets: &[String],
) -> Result<Vec<TranscodeOutput>, String> {
    // Check if this is a QSV encoder - needs dump_extra BSF for FLV output
    let is_qsv = group.video.codec.to_ascii_lowercase().contains("qsv");

    let mut outputs = Vec::new();
    for target_url in targets {
        let mut output_ctx: *mut ffi::AVFormatContext = ptr::null_mut();
        let url_c = CString::new(target_url.as_str())
            .map_err(|_| "Output URL contains null byte".to_string())?;
        let alloc_ret = unsafe {
            ffi::avformat_alloc_output_context2(
                &mut output_ctx,
                ptr::null_mut(),
                CString::new("flv").unwrap().as_ptr(),
                url_c.as_ptr(),
            )
        };
        if alloc_ret < 0 || output_ctx.is_null() {
            return Err(format!("Failed to allocate output context: {}", ffmpeg_err(alloc_ret)));
        }

        let video_stream = unsafe { ffi::avformat_new_stream(output_ctx, ptr::null()) };
        if video_stream.is_null() {
            unsafe { ffi::avformat_free_context(output_ctx) };
            return Err("Failed to create video output stream".to_string());
        }
        let video_copy_ret = unsafe { ffi::avcodec_parameters_from_context((*video_stream).codecpar, video_enc_ctx) };
        if video_copy_ret < 0 {
            unsafe { ffi::avformat_free_context(output_ctx) };
            return Err(format!("Failed to copy video encoder params: {}", ffmpeg_err(video_copy_ret)));
        }
        unsafe {
            (*video_stream).time_base = (*video_enc_ctx).time_base;
            // Set FLV codec tag for H.264 video (tag 7)
            if (*video_enc_ctx).codec_id == ffi::AVCodecID::AV_CODEC_ID_H264 {
                (*(*video_stream).codecpar).codec_tag = 7;
            }
        }

        let mut audio_out_index = None;
        if let Some(audio_idx) = audio_stream_index {
            if group.audio.codec.eq_ignore_ascii_case("copy") {
                let in_stream = unsafe { *(*input_ctx).streams.add(audio_idx) };
                let out_stream = unsafe { ffi::avformat_new_stream(output_ctx, ptr::null()) };
                if out_stream.is_null() {
                    unsafe { ffi::avformat_free_context(output_ctx) };
                    return Err("Failed to create audio output stream".to_string());
                }
                let copy_ret = unsafe { ffi::avcodec_parameters_copy((*out_stream).codecpar, (*in_stream).codecpar) };
                if copy_ret < 0 {
                    unsafe { ffi::avformat_free_context(output_ctx) };
                    return Err(format!("Failed to copy audio codec params: {}", ffmpeg_err(copy_ret)));
                }
                unsafe {
                    (*out_stream).time_base = (*in_stream).time_base;
                    // Set FLV codec tag for AAC audio (tag 10)
                    if (*(*in_stream).codecpar).codec_id == ffi::AVCodecID::AV_CODEC_ID_AAC {
                        (*(*out_stream).codecpar).codec_tag = 10;
                    }
                }
                audio_out_index = Some(unsafe { (*out_stream).index });
            } else if let Some(audio_enc_ctx) = audio_enc_ctx {
                let out_stream = unsafe { ffi::avformat_new_stream(output_ctx, ptr::null()) };
                if out_stream.is_null() {
                    unsafe { ffi::avformat_free_context(output_ctx) };
                    return Err("Failed to create audio output stream".to_string());
                }
                let copy_ret = unsafe { ffi::avcodec_parameters_from_context((*out_stream).codecpar, audio_enc_ctx) };
                if copy_ret < 0 {
                    unsafe { ffi::avformat_free_context(output_ctx) };
                    return Err(format!("Failed to copy audio encoder params: {}", ffmpeg_err(copy_ret)));
                }
                unsafe {
                    (*out_stream).time_base = (*audio_enc_ctx).time_base;
                    // Set FLV codec tag for AAC audio (tag 10)
                    if (*audio_enc_ctx).codec_id == ffi::AVCodecID::AV_CODEC_ID_AAC {
                        (*(*out_stream).codecpar).codec_tag = 10;
                    }
                }
                audio_out_index = Some(unsafe { (*out_stream).index });
            }
        }

        let mut opts: *mut ffi::AVDictionary = ptr::null_mut();
        unsafe {
            ffi::av_dict_set(&mut opts, CString::new("flvflags").unwrap().as_ptr(), CString::new("no_duration_filesize").unwrap().as_ptr(), 0);
            // Add RTMP resilience options for RTMP/RTMPS URLs
            add_rtmp_options(&mut opts, target_url);
        }
        let open_ret = unsafe {
            if (*(*output_ctx).oformat).flags & ffi::AVFMT_NOFILE == 0 {
                ffi::avio_open2(&mut (*output_ctx).pb, url_c.as_ptr(), ffi::AVIO_FLAG_WRITE, ptr::null_mut(), &mut opts)
            } else {
                0
            }
        };
        if open_ret < 0 {
            unsafe { ffi::avformat_free_context(output_ctx) };
            return Err(format!("Failed to open output: {}", ffmpeg_err(open_ret)));
        }

        let header_ret = unsafe { ffi::avformat_write_header(output_ctx, &mut opts) };
        if header_ret < 0 {
            unsafe {
                if (*(*output_ctx).oformat).flags & ffi::AVFMT_NOFILE == 0 {
                    ffi::avio_closep(&mut (*output_ctx).pb);
                }
                ffi::avformat_free_context(output_ctx);
            }
            return Err(format!("Failed to write output header: {}", ffmpeg_err(header_ret)));
        }

        // Create dump_extra BSF for QSV encoders (writes SPS/PPS to each IDR frame)
        let video_bsf_ctx = if is_qsv {
            unsafe {
                match create_dump_extra_bsf((*video_stream).codecpar) {
                    Ok(bsf) => Some(bsf),
                    Err(e) => {
                        log::warn!("Failed to create dump_extra BSF for QSV, continuing without it: {}", e);
                        None
                    }
                }
            }
        } else {
            None
        };

        outputs.push(TranscodeOutput {
            ctx: output_ctx,
            video_out_index: unsafe { (*video_stream).index },
            audio_out_index,
            video_bsf_ctx,
        });
    }

    Ok(outputs)
}

fn transcode_video_packet(
    group: &mut TranscodeGroup,
    in_stream: *mut ffi::AVStream,
    packet: *mut ffi::AVPacket,
) -> Result<(), String> {
    let input_width = unsafe { (*group.video_dec_ctx).width };
    let input_height = unsafe { (*group.video_dec_ctx).height };
    let output_width = unsafe { (*group.video_enc_ctx).width };
    let output_height = unsafe { (*group.video_enc_ctx).height };
    let needs_scale = input_width != output_width || input_height != output_height;

    let send_ret = unsafe { ffi::avcodec_send_packet(group.video_dec_ctx, packet) };
    if send_ret < 0 {
        return Err(format!("Video decoder send failed: {}", ffmpeg_err(send_ret)));
    }

    loop {
        let receive_ret = unsafe { ffi::avcodec_receive_frame(group.video_dec_ctx, group.video_dec_frame) };
        if receive_ret < 0 {
            break;
        }

        let pts = unsafe {
            ffi::av_rescale_q(
                (*group.video_dec_frame).pts,
                (*in_stream).time_base,
                (*group.video_enc_ctx).time_base,
            )
        };

        let mut source_frame = group.video_dec_frame;
        if group.using_hw_decode && unsafe { !(*group.video_dec_frame).hw_frames_ctx.is_null() } {
            let hw_sw_frame = group.video_hw_sw_frame
                .ok_or_else(|| "Missing hardware download frame".to_string())?;
            unsafe {
                let writable_ret = ffi::av_frame_make_writable(hw_sw_frame);
                if writable_ret < 0 {
                    return Err(format!("HW download frame not writable: {}", ffmpeg_err(writable_ret)));
                }
                let transfer_ret = ffi::av_hwframe_transfer_data(hw_sw_frame, group.video_dec_frame, 0);
                if transfer_ret < 0 {
                    return Err(format!("Failed to download hw frame: {}", ffmpeg_err(transfer_ret)));
                }
                (*hw_sw_frame).pts = pts;
            }
            source_frame = hw_sw_frame;
        }

        let mut frame_to_send = source_frame;
        if needs_scale || source_frame == group.video_dec_frame {
            unsafe {
                let writable_ret = ffi::av_frame_make_writable(group.video_sw_frame);
                if writable_ret < 0 {
                    return Err(format!("Video frame not writable: {}", ffmpeg_err(writable_ret)));
                }
                ffi::sws_scale(
                    group.sws_ctx,
                    (*source_frame).data.as_ptr() as *const *const u8,
                    (*source_frame).linesize.as_ptr(),
                    0,
                    input_height,
                    (*group.video_sw_frame).data.as_mut_ptr(),
                    (*group.video_sw_frame).linesize.as_mut_ptr(),
                );
                (*group.video_sw_frame).pts = pts;
            }
            frame_to_send = group.video_sw_frame;
        } else {
            unsafe {
                (*frame_to_send).pts = pts;
            }
        }

        if let (Some(hw_frames_ctx), Some(hw_frame)) = (group.video_hw_frames_ctx, group.video_hw_frame) {
            unsafe {
                ffi::av_frame_unref(hw_frame);
                (*hw_frame).format = (*group.video_enc_ctx).pix_fmt as i32;
                (*hw_frame).width = (*group.video_enc_ctx).width;
                (*hw_frame).height = (*group.video_enc_ctx).height;
                let hw_ret = ffi::av_hwframe_get_buffer(hw_frames_ctx, hw_frame, 0);
                if hw_ret < 0 {
                    return Err(format!("Failed to allocate hw frame: {}", ffmpeg_err(hw_ret)));
                }
                let transfer_ret = ffi::av_hwframe_transfer_data(hw_frame, frame_to_send, 0);
                if transfer_ret < 0 {
                    return Err(format!("Failed to upload hw frame: {}", ffmpeg_err(transfer_ret)));
                }
                (*hw_frame).pts = (*frame_to_send).pts;
                frame_to_send = hw_frame;
            }
        }

        let send_enc_ret = unsafe { ffi::avcodec_send_frame(group.video_enc_ctx, frame_to_send) };
        if send_enc_ret < 0 {
            return Err(format!("Video encoder send failed: {}", ffmpeg_err(send_enc_ret)));
        }

        let mut enc_pkt = unsafe { ffi::av_packet_alloc() };
        if enc_pkt.is_null() {
            return Err("Failed to allocate video packet".to_string());
        }
        loop {
            let recv_ret = unsafe { ffi::avcodec_receive_packet(group.video_enc_ctx, enc_pkt) };
            if recv_ret < 0 {
                break;
            }
            write_encoded_packet(enc_pkt, group, true)?;
            unsafe { ffi::av_packet_unref(enc_pkt) };
        }
        unsafe { ffi::av_packet_free(&mut enc_pkt) };
        unsafe { ffi::av_frame_unref(group.video_dec_frame) };
    }

    Ok(())
}

fn write_encoded_packet(
    enc_pkt: *mut ffi::AVPacket,
    group: &mut TranscodeGroup,
    is_video: bool,
) -> Result<(), String> {
    let output_count = group.outputs.len();
    let is_single_target = output_count == 1;

    for (idx, output) in group.outputs.iter_mut().enumerate() {
        let out_index = if is_video {
            output.video_out_index
        } else {
            match output.audio_out_index {
                Some(idx) => idx,
                None => continue,
            }
        };

        // Optimization: skip cloning for single-target groups or last target in multi-target
        // For single target, use the original packet directly
        // For multi-target, clone for all but the last (use original for last)
        let is_last_target = idx == output_count - 1;
        let (pkt_to_write, needs_free) = if is_single_target || is_last_target {
            // Use original packet directly (caller handles freeing)
            (enc_pkt, false)
        } else {
            // Clone for this target
            let clone = unsafe { ffi::av_packet_clone(enc_pkt) };
            if clone.is_null() {
                continue;
            }
            (clone, true)
        };
        let mut pkt_clone = pkt_to_write;

        // Check if we need to apply video BSF (dump_extra for QSV)
        let needs_video_bsf = is_video && output.video_bsf_ctx.is_some();

        unsafe {
            let out_stream = *(*output.ctx).streams.add(out_index as usize);
            let time_base = if is_video {
                (*group.video_enc_ctx).time_base
            } else {
                (*group.audio_enc_ctx.unwrap()).time_base
            };

            if needs_video_bsf {
                // Apply dump_extra bitstream filter
                let bsf_ctx = output.video_bsf_ctx.unwrap();
                let send_ret = ffi::av_bsf_send_packet(bsf_ctx, pkt_clone);
                if send_ret < 0 {
                    log::warn!("Video BSF send failed: {}", ffmpeg_err(send_ret));
                    if needs_free {
                        ffi::av_packet_free(&mut pkt_clone);
                    }
                    continue;
                }

                // Receive and write filtered packets
                loop {
                    let mut filtered_pkt = ffi::av_packet_alloc();
                    if filtered_pkt.is_null() {
                        break;
                    }
                    let recv_ret = ffi::av_bsf_receive_packet(bsf_ctx, filtered_pkt);
                    if recv_ret < 0 {
                        ffi::av_packet_free(&mut filtered_pkt);
                        break;
                    }

                    ffi::av_packet_rescale_ts(filtered_pkt, time_base, (*out_stream).time_base);
                    (*filtered_pkt).stream_index = out_index;
                    let write_ret = ffi::av_interleaved_write_frame(output.ctx, filtered_pkt);
                    if write_ret < 0 {
                        log::warn!(
                            "FFmpeg libs transcode write failed for group {}: {}",
                            group.group_id,
                            ffmpeg_err(write_ret)
                        );
                    }
                    ffi::av_packet_free(&mut filtered_pkt);
                }
                if needs_free {
                    ffi::av_packet_free(&mut pkt_clone);
                }
            } else {
                // No BSF needed, write directly
                ffi::av_packet_rescale_ts(pkt_clone, time_base, (*out_stream).time_base);
                (*pkt_clone).stream_index = out_index;
                let write_ret = ffi::av_interleaved_write_frame(output.ctx, pkt_clone);
                if write_ret < 0 {
                    log::warn!(
                        "FFmpeg libs transcode write failed for group {}: {}",
                        group.group_id,
                        ffmpeg_err(write_ret)
                    );
                }
                if needs_free {
                    ffi::av_packet_free(&mut pkt_clone);
                }
            }
        }
    }
    Ok(())
}

fn transcode_audio_packet(
    group: &mut TranscodeGroup,
    in_stream: *mut ffi::AVStream,
    packet: *mut ffi::AVPacket,
) -> Result<(), String> {
    if group.audio_stream_index.is_none() {
        return Ok(());
    }

    if group.audio_dec_ctx.is_none() || group.audio_enc_ctx.is_none() {
        // Audio copy path - optimized to avoid unnecessary cloning.
        let output_count = group.outputs.len();

        for (idx, output) in group.outputs.iter().enumerate() {
            let out_index = match output.audio_out_index {
                Some(idx) => idx,
                None => continue,
            };

            // Optimization: skip cloning for single-target or last target
            let is_last_target = idx == output_count - 1;
            let (pkt_to_write, needs_free) = if output_count == 1 || is_last_target {
                // Use original packet directly (caller handles it)
                (packet, false)
            } else {
                // Clone for this target
                let clone = unsafe { ffi::av_packet_clone(packet) };
                if clone.is_null() {
                    continue;
                }
                (clone, true)
            };

            unsafe {
                let out_stream = *(*output.ctx).streams.add(out_index as usize);
                ffi::av_packet_rescale_ts(pkt_to_write, (*in_stream).time_base, (*out_stream).time_base);
                (*pkt_to_write).stream_index = out_index;
                let write_ret = ffi::av_interleaved_write_frame(output.ctx, pkt_to_write);
                if write_ret < 0 {
                    log::warn!(
                        "FFmpeg libs audio copy write failed for group {}: {}",
                        group.group_id,
                        ffmpeg_err(write_ret)
                    );
                }
                if needs_free {
                    ffi::av_packet_free(&mut (pkt_to_write as *mut _));
                }
            }
        }

        return Ok(());
    }

    let audio_dec_ctx = group.audio_dec_ctx.unwrap();
    let audio_enc_ctx = group.audio_enc_ctx.unwrap();
    let send_ret = unsafe { ffi::avcodec_send_packet(audio_dec_ctx, packet) };
    if send_ret < 0 {
        return Err(format!("Audio decoder send failed: {}", ffmpeg_err(send_ret)));
    }

    loop {
        let recv_ret = unsafe { ffi::avcodec_receive_frame(audio_dec_ctx, group.audio_dec_frame) };
        if recv_ret < 0 {
            break;
        }

        let out_samples = unsafe {
            ffi::av_rescale_rnd(
                ffi::swr_get_delay(group.swr_ctx.unwrap(), (*audio_dec_ctx).sample_rate as i64)
                    + (*group.audio_dec_frame).nb_samples as i64,
                (*audio_enc_ctx).sample_rate as i64,
                (*audio_dec_ctx).sample_rate as i64,
                ffi::AVRounding::AV_ROUND_UP,
            ) as i32
        };

        let mut out_frame = unsafe { ffi::av_frame_alloc() };
        if out_frame.is_null() {
            return Err("Failed to allocate audio output frame".to_string());
        }
        unsafe {
            (*out_frame).nb_samples = out_samples;
            (*out_frame).format = (*audio_enc_ctx).sample_fmt as i32;
            (*out_frame).sample_rate = (*audio_enc_ctx).sample_rate;
            ffi::av_channel_layout_copy(&mut (*out_frame).ch_layout, &(*audio_enc_ctx).ch_layout);
            let buffer_ret = ffi::av_frame_get_buffer(out_frame, 0);
            if buffer_ret < 0 {
                ffi::av_frame_free(&mut out_frame);
                return Err(format!("Failed to allocate audio buffer: {}", ffmpeg_err(buffer_ret)));
            }
        }

        let convert_ret = unsafe {
            ffi::swr_convert(
                group.swr_ctx.unwrap(),
                (*out_frame).data.as_mut_ptr(),
                out_samples,
                (*group.audio_dec_frame).data.as_ptr() as *const *const u8,
                (*group.audio_dec_frame).nb_samples,
            )
        };
        if convert_ret < 0 {
            unsafe { ffi::av_frame_free(&mut out_frame) };
            return Err(format!("Audio resample failed: {}", ffmpeg_err(convert_ret)));
        }

        unsafe {
            (*out_frame).pts = ffi::av_rescale_q(
                (*group.audio_dec_frame).pts,
                (*in_stream).time_base,
                (*audio_enc_ctx).time_base,
            );
        }

        let send_enc_ret = unsafe { ffi::avcodec_send_frame(audio_enc_ctx, out_frame) };
        if send_enc_ret < 0 {
            unsafe { ffi::av_frame_free(&mut out_frame) };
            return Err(format!("Audio encoder send failed: {}", ffmpeg_err(send_enc_ret)));
        }

        let mut enc_pkt = unsafe { ffi::av_packet_alloc() };
        if enc_pkt.is_null() {
            unsafe { ffi::av_frame_free(&mut out_frame) };
            return Err("Failed to allocate audio packet".to_string());
        }
        loop {
            let recv_ret = unsafe { ffi::avcodec_receive_packet(audio_enc_ctx, enc_pkt) };
            if recv_ret < 0 {
                break;
            }
            write_encoded_packet(enc_pkt, group, false)?;
            unsafe { ffi::av_packet_unref(enc_pkt) };
        }
        unsafe { ffi::av_packet_free(&mut enc_pkt) };
        unsafe { ffi::av_frame_free(&mut out_frame) };
        unsafe { ffi::av_frame_unref(group.audio_dec_frame) };
    }

    Ok(())
}

fn flush_transcode_group(group: &mut TranscodeGroup) -> Result<(), String> {
    let send_ret = unsafe { ffi::avcodec_send_frame(group.video_enc_ctx, ptr::null()) };
    if send_ret >= 0 {
        let mut enc_pkt = unsafe { ffi::av_packet_alloc() };
        if enc_pkt.is_null() {
            return Err("Failed to allocate flush packet".to_string());
        }
        loop {
            let recv_ret = unsafe { ffi::avcodec_receive_packet(group.video_enc_ctx, enc_pkt) };
            if recv_ret < 0 {
                break;
            }
            write_encoded_packet(enc_pkt, group, true)?;
            unsafe { ffi::av_packet_unref(enc_pkt) };
        }
        unsafe { ffi::av_packet_free(&mut enc_pkt) };
    }

    if let Some(audio_enc_ctx) = group.audio_enc_ctx {
        let send_ret = unsafe { ffi::avcodec_send_frame(audio_enc_ctx, ptr::null()) };
        if send_ret >= 0 {
            let mut enc_pkt = unsafe { ffi::av_packet_alloc() };
            if enc_pkt.is_null() {
                return Err("Failed to allocate audio flush packet".to_string());
            }
            loop {
                let recv_ret = unsafe { ffi::avcodec_receive_packet(audio_enc_ctx, enc_pkt) };
                if recv_ret < 0 {
                    break;
                }
                write_encoded_packet(enc_pkt, group, false)?;
                unsafe { ffi::av_packet_unref(enc_pkt) };
            }
            unsafe { ffi::av_packet_free(&mut enc_pkt) };
        }
    }

    Ok(())
}

/// Clean up a single passthrough group (close RTMP connections, free contexts)
fn cleanup_single_passthrough_group(group: &mut GroupOutputs) {
    if group.cleaned_up {
        return;
    }

    log::debug!("Cleaning up passthrough group: {}", group.group_id);

    for target in &mut group.targets {
        unsafe {
            // Free BSF context if present
            if let Some(mut bsf_ctx) = target.audio_bsf_ctx.take() {
                ffi::av_bsf_free(&mut bsf_ctx);
            }

            if !target.ctx.is_null() {
                let _ = ffi::av_write_trailer(target.ctx);
                if (*(*target.ctx).oformat).flags & ffi::AVFMT_NOFILE == 0 {
                    let _ = ffi::avio_closep(&mut (*target.ctx).pb);
                }
                ffi::avformat_free_context(target.ctx);
                target.ctx = ptr::null_mut();
            }
        }
    }

    group.cleaned_up = true;
}

/// Clean up transcode group output connections only (not encoder contexts)
/// Used when stopping a group mid-stream
fn cleanup_transcode_group_outputs(group: &mut TranscodeGroup) {
    log::debug!("Cleaning up transcode group outputs: {}", group.group_id);

    for output in &mut group.outputs {
        unsafe {
            // Free video BSF context if present
            if let Some(mut bsf_ctx) = output.video_bsf_ctx.take() {
                ffi::av_bsf_free(&mut bsf_ctx);
            }

            if !output.ctx.is_null() {
                let _ = ffi::av_write_trailer(output.ctx);
                if (*(*output.ctx).oformat).flags & ffi::AVFMT_NOFILE == 0 {
                    let _ = ffi::avio_closep(&mut (*output.ctx).pb);
                }
                ffi::avformat_free_context(output.ctx);
                output.ctx = ptr::null_mut();
            }
        }
    }
}

/// Full cleanup of transcode group (all resources including encoder contexts)
fn cleanup_transcode_group(group: TranscodeGroup) {
    if group.cleaned_up {
        // Outputs already cleaned, just free encoder resources
        unsafe {
            ffi::av_frame_free(&mut (group.video_dec_frame as *mut _));
            ffi::av_frame_free(&mut (group.video_sw_frame as *mut _));
            if let Some(mut hw_sw_frame) = group.video_hw_sw_frame {
                ffi::av_frame_free(&mut (hw_sw_frame as *mut _));
            }
            if let Some(mut hw_frame) = group.video_hw_frame {
                ffi::av_frame_free(&mut (hw_frame as *mut _));
            }
            if !group.audio_dec_frame.is_null() {
                ffi::av_frame_free(&mut (group.audio_dec_frame as *mut _));
            }
            if let Some(mut swr_ctx) = group.swr_ctx {
                ffi::swr_free(&mut swr_ctx);
            }
            ffi::sws_freeContext(group.sws_ctx);
            if let Some(mut frames_ref) = group.video_hw_frames_ctx {
                ffi::av_buffer_unref(&mut frames_ref);
            }
            if let Some(mut frames_ref) = group.video_dec_hw_frames_ctx {
                ffi::av_buffer_unref(&mut frames_ref);
            }
            if let Some(mut audio_enc) = group.audio_enc_ctx {
                ffi::avcodec_free_context(&mut audio_enc);
            }
            if let Some(mut audio_dec) = group.audio_dec_ctx {
                ffi::avcodec_free_context(&mut audio_dec);
            }
            if let Some(mut device_ref) = group.video_hw_device {
                ffi::av_buffer_unref(&mut device_ref);
            }
            ffi::avcodec_free_context(&mut (group.video_enc_ctx as *mut _));
            ffi::avcodec_free_context(&mut (group.video_dec_ctx as *mut _));
        }
        return;
    }

    unsafe {
        for mut output in group.outputs {
            // Free video BSF context if present
            if let Some(mut bsf_ctx) = output.video_bsf_ctx.take() {
                ffi::av_bsf_free(&mut bsf_ctx);
            }

            let _ = ffi::av_write_trailer(output.ctx);
            if (*(*output.ctx).oformat).flags & ffi::AVFMT_NOFILE == 0 {
                let _ = ffi::avio_closep(&mut (*output.ctx).pb);
            }
            ffi::avformat_free_context(output.ctx);
        }
        ffi::av_frame_free(&mut (group.video_dec_frame as *mut _));
        ffi::av_frame_free(&mut (group.video_sw_frame as *mut _));
        if let Some(mut hw_sw_frame) = group.video_hw_sw_frame {
            ffi::av_frame_free(&mut (hw_sw_frame as *mut _));
        }
        if let Some(mut hw_frame) = group.video_hw_frame {
            ffi::av_frame_free(&mut (hw_frame as *mut _));
        }
        if !group.audio_dec_frame.is_null() {
            ffi::av_frame_free(&mut (group.audio_dec_frame as *mut _));
        }
        if let Some(mut swr_ctx) = group.swr_ctx {
            ffi::swr_free(&mut swr_ctx);
        }
        ffi::sws_freeContext(group.sws_ctx);
        if let Some(mut frames_ref) = group.video_hw_frames_ctx {
            ffi::av_buffer_unref(&mut frames_ref);
        }
        if let Some(mut frames_ref) = group.video_dec_hw_frames_ctx {
            ffi::av_buffer_unref(&mut frames_ref);
        }
        if let Some(mut audio_enc) = group.audio_enc_ctx {
            ffi::avcodec_free_context(&mut audio_enc);
        }
        if let Some(mut audio_dec) = group.audio_dec_ctx {
            ffi::avcodec_free_context(&mut audio_dec);
        }
        if let Some(mut device_ref) = group.video_hw_device {
            ffi::av_buffer_unref(&mut device_ref);
        }
        ffi::avcodec_free_context(&mut (group.video_enc_ctx as *mut _));
        ffi::avcodec_free_context(&mut (group.video_dec_ctx as *mut _));
    }
}

/// Clean up all passthrough groups
fn cleanup_outputs(groups: &mut Vec<GroupOutputs>) {
    for group in groups.iter_mut() {
        cleanup_single_passthrough_group(group);
    }
}

fn is_hw_encoder(encoder_name: &str) -> bool {
    let name = encoder_name.to_ascii_lowercase();
    name.contains("nvenc") || name.contains("qsv") || name.contains("amf") || name.contains("videotoolbox")
}

/// Check if any target URL appears to be Twitch
fn targets_contain_twitch(targets: &[String]) -> bool {
    targets.iter().any(|url| {
        let lower = url.to_ascii_lowercase();
        lower.contains("twitch.tv") || lower.contains("live-video.net")
    })
}

/// Check if any stream target is Twitch based on service field or URL
fn stream_targets_contain_twitch(targets: &[StreamTarget]) -> bool {
    targets.iter().any(|t| {
        // Check service name (Platform enum serializes to string like "Twitch")
        let service_str = format!("{:?}", t.service);
        if service_str.to_ascii_lowercase().contains("twitch") {
            return true;
        }
        // Fallback: check URL
        let lower = t.url.to_ascii_lowercase();
        lower.contains("twitch.tv") || lower.contains("live-video.net")
    })
}

/// Apply encoder-specific options via av_opt_set
/// This must be called BEFORE avcodec_open2
unsafe fn apply_encoder_options(
    enc_ctx: *mut ffi::AVCodecContext,
    encoder_name: &str,
    preset: Option<&str>,
    profile: Option<&str>,
    is_twitch_target: bool,
) {
    let name_lower = encoder_name.to_ascii_lowercase();
    let is_low_latency_preset = preset
        .map(|value| {
            let lower = value.trim().to_ascii_lowercase();
            matches!(
                lower.as_str(),
                "low_latency"
                    | "low-latency"
                    | "lowlatency"
                    | "ll"
                    | "llhq"
                    | "llhp"
            )
        })
        .unwrap_or(false);

    // Apply preset if provided
    if let Some(preset_val) = preset {
        let preset_c = CString::new(preset_val).unwrap_or_default();

        // Different encoders use different preset option names
        if name_lower.contains("nvenc") {
            let key = CString::new("preset").unwrap();
            // NVENC presets: p1-p7 or names like "fast", "medium", "slow"
            let preset_lower = preset_val.trim().to_ascii_lowercase();
            let nvenc_preset = match preset_lower.as_str() {
                "p1" | "p2" | "p3" | "p4" | "p5" | "p6" | "p7" | "default" | "slow"
                | "medium" | "fast" | "hp" | "hq" | "bd" | "ll" | "llhq" | "llhp"
                | "lossless" | "losslesshp" => preset_lower.as_str(),
                "ultrafast" => "p1",
                "superfast" => "p2",
                "veryfast" => "p3",
                "faster" => "p4",
                "slower" => "p6",
                "veryslow" => "p7",
                "quality" => "p7",
                "balanced" => "p4",
                "performance" => "p2",
                "low_latency" | "low-latency" | "lowlatency" => "p1",
                _ => "p4",
            };
            let val = CString::new(nvenc_preset).unwrap_or_default();
            ffi::av_opt_set(enc_ctx.cast(), key.as_ptr(), val.as_ptr(), 0);
        } else if name_lower.contains("qsv") {
            let key = CString::new("preset").unwrap();
            // Map custom preset names to QSV presets (matches CLI behavior)
            // QSV presets: veryfast, faster, fast, medium, slow, slower, veryslow
            let preset_lower = preset_val.to_lowercase();
            let qsv_preset = match preset_lower.as_str() {
                "quality" => "slow",
                "balanced" => "medium",
                "performance" => "fast",
                "speed" => "veryfast",
                "low_latency" | "low-latency" | "lowlatency" => "veryfast",
                _ => preset_val, // Pass through if already a valid FFmpeg preset
            };
            let val = CString::new(qsv_preset).unwrap_or_default();
            ffi::av_opt_set(enc_ctx.cast(), key.as_ptr(), val.as_ptr(), 0);
        } else if name_lower.contains("amf") {
            let key = CString::new("quality").unwrap();
            // AMF quality: speed, balanced, quality
            let preset_lower = preset_val.to_lowercase();
            let amf_quality = match preset_lower.as_str() {
                "ultrafast" | "superfast" | "veryfast" | "faster" | "fast" => "speed",
                "medium" => "balanced",
                "slow" | "slower" | "veryslow" | "placebo" => "quality",
                _ => preset_val,
            };
            let val = CString::new(amf_quality).unwrap_or_default();
            ffi::av_opt_set(enc_ctx.cast(), key.as_ptr(), val.as_ptr(), 0);

            if is_low_latency_preset {
                let key = CString::new("usage").unwrap();
                let val = CString::new("lowlatency").unwrap();
                ffi::av_opt_set(enc_ctx.cast(), key.as_ptr(), val.as_ptr(), 0);
            }
        } else if name_lower.contains("videotoolbox") {
            // VideoToolbox doesn't have presets, it uses realtime flag
            let preset_lower = preset_val.to_lowercase();
            if preset_lower.contains("fast") {
                let key = CString::new("realtime").unwrap();
                let val = CString::new("1").unwrap();
                ffi::av_opt_set(enc_ctx.cast(), key.as_ptr(), val.as_ptr(), 0);
            }
        } else {
            // Software encoders (libx264, libx265)
            let key = CString::new("preset").unwrap();
            ffi::av_opt_set(enc_ctx.cast(), key.as_ptr(), preset_c.as_ptr(), 0);
        }
    }

    // Apply profile if provided
    if let Some(profile_val) = profile {
        let profile_lower = profile_val.to_lowercase();

        // For AMF encoders, set profile via codec context's profile field
        // AMF's av_opt_set with string doesn't work reliably
        if name_lower.contains("amf") {
            // Map profile names to FF_PROFILE_H264 values
            let profile_id = match profile_lower.as_str() {
                "baseline" | "constrained_baseline" => 66,  // FF_PROFILE_H264_BASELINE
                "main" => 77,                               // FF_PROFILE_H264_MAIN
                "high" => 100,                              // FF_PROFILE_H264_HIGH
                "high10" => 110,                            // FF_PROFILE_H264_HIGH_10
                "high422" => 122,                           // FF_PROFILE_H264_HIGH_422
                "high444" => 244,                           // FF_PROFILE_H264_HIGH_444_PREDICTIVE
                _ => -1,                                    // Use encoder default
            };
            if profile_id > 0 {
                (*enc_ctx).profile = profile_id;
            }
        } else {
            // For other encoders, use av_opt_set
            let key = CString::new("profile").unwrap();
            let profile_c = CString::new(profile_val).unwrap_or_default();
            ffi::av_opt_set(enc_ctx.cast(), key.as_ptr(), profile_c.as_ptr(), 0);
        }
    }

    // QSV H.264 specific settings (profile and level)
    if name_lower.contains("qsv") && name_lower.contains("264") {
        // Default to high profile if not specified (matches CLI behavior)
        if profile.is_none() {
            let key = CString::new("profile").unwrap();
            let val = CString::new("high").unwrap();
            ffi::av_opt_set(enc_ctx.cast(), key.as_ptr(), val.as_ptr(), 0);
        }

        // Set H.264 level based on resolution for streaming platform compatibility
        // Level 4.2 supports 1080p60, level 5.1 supports 1440p60 and 4K30
        let width = (*enc_ctx).width;
        let height = (*enc_ctx).height;
        let fps = if (*enc_ctx).framerate.den > 0 {
            (*enc_ctx).framerate.num / (*enc_ctx).framerate.den
        } else {
            30
        };

        let level = if height > 1080 || (height == 1080 && fps > 60) {
            "5.1"
        } else if height >= 1080 && fps >= 60 {
            "4.2"
        } else {
            "4.1"
        };
        let key = CString::new("level").unwrap();
        let val = CString::new(level).unwrap();
        ffi::av_opt_set(enc_ctx.cast(), key.as_ptr(), val.as_ptr(), 0);
        log::debug!("QSV H.264: set level {} for {}x{}@{}fps", level, width, height, fps);
    }

    // Apply Twitch-safe QSV overrides
    // Twitch has strict requirements for QSV: no B-frames, no lookahead, forced IDR
    if is_twitch_target && name_lower.contains("qsv") {
        log::info!("Applying Twitch-safe QSV overrides");

        // Disable B-frames
        (*enc_ctx).max_b_frames = 0;

        // Disable lookahead (reduces latency, required for Twitch)
        let key = CString::new("look_ahead").unwrap();
        let val = CString::new("0").unwrap();
        ffi::av_opt_set(enc_ctx.cast(), key.as_ptr(), val.as_ptr(), 0);

        // Reduce async depth (helps with latency and compatibility)
        let key = CString::new("async_depth").unwrap();
        let val = CString::new("1").unwrap();
        ffi::av_opt_set(enc_ctx.cast(), key.as_ptr(), val.as_ptr(), 0);

        // Force IDR frames at keyframe boundaries
        let key = CString::new("forced_idr").unwrap();
        let val = CString::new("1").unwrap();
        ffi::av_opt_set(enc_ctx.cast(), key.as_ptr(), val.as_ptr(), 0);

        // Repeat PPS/SPS for each IDR (ensures stream decoders can join mid-stream)
        // Note: repeat_pps may not be supported on older FFmpeg/QSV versions; av_opt_set
        // will return an error but we continue anyway as the stream may still work
        let key = CString::new("repeat_pps").unwrap();
        let val = CString::new("1").unwrap();
        let ret = ffi::av_opt_set(enc_ctx.cast(), key.as_ptr(), val.as_ptr(), 0);
        if ret < 0 {
            log::debug!("QSV repeat_pps not supported (error {}), continuing without it", ret);
        }
    } else if name_lower.contains("qsv") {
        // Non-Twitch QSV defaults aligned with OBS for best quality
        // Enable B-frames for better compression
        (*enc_ctx).max_b_frames = 3;

        // Enable lookahead for better rate control
        let key = CString::new("look_ahead").unwrap();
        let val = CString::new("1").unwrap();
        ffi::av_opt_set(enc_ctx.cast(), key.as_ptr(), val.as_ptr(), 0);

        // Lookahead depth (60 frames = 1 second at 60fps)
        // Use shorter depth for low-latency presets (matches CLI behavior)
        let preset_lower = preset.map(|p| p.to_lowercase()).unwrap_or_default();
        let is_low_latency = matches!(
            preset_lower.as_str(),
            "low_latency" | "low-latency" | "lowlatency"
        );
        let depth = if is_low_latency { "30" } else { "60" };
        let key = CString::new("look_ahead_depth").unwrap();
        let val = CString::new(depth).unwrap();
        ffi::av_opt_set(enc_ctx.cast(), key.as_ptr(), val.as_ptr(), 0);

        // Async depth for better GPU utilization
        let key = CString::new("async_depth").unwrap();
        let val = CString::new("4").unwrap();
        ffi::av_opt_set(enc_ctx.cast(), key.as_ptr(), val.as_ptr(), 0);

        log::debug!("QSV: set B-frames=3, look_ahead=1, look_ahead_depth={}, async_depth=4", depth);
    }

    // Apply common hardware encoder optimizations
    if name_lower.contains("nvenc") {
        if is_low_latency_preset {
            // NVENC tuning for low-latency streaming
            let key = CString::new("tune").unwrap();
            let val = CString::new("ll").unwrap(); // low latency
            ffi::av_opt_set(enc_ctx.cast(), key.as_ptr(), val.as_ptr(), 0);
        }

        // Use CBR rate control for streaming
        let key = CString::new("rc").unwrap();
        let val = CString::new("cbr").unwrap();
        ffi::av_opt_set(enc_ctx.cast(), key.as_ptr(), val.as_ptr(), 0);
    }

    // AMF-specific optimizations
    if name_lower.contains("amf") {
        // Use CBR rate control for streaming
        let key = CString::new("rc").unwrap();
        let val = CString::new("cbr").unwrap();
        ffi::av_opt_set(enc_ctx.cast(), key.as_ptr(), val.as_ptr(), 0);

        if is_low_latency_preset {
            let key = CString::new("usage").unwrap();
            let val = CString::new("lowlatency").unwrap();
            ffi::av_opt_set(enc_ctx.cast(), key.as_ptr(), val.as_ptr(), 0);

            // Disable B-frames for low-latency streaming
            (*enc_ctx).max_b_frames = 0;
        }
    }

    // Software encoder CBR enforcement (libx264, libx265)
    // These require encoder-specific options to enable NAL-HRD signaling for true CBR
    // Also disable scene change detection for consistent keyframe intervals (matches CLI)
    if name_lower == "libx264" {
        // x264-params: nal-hrd=cbr enables NAL HRD signaling, force-cfr=1 ensures constant frame rate
        // scenecut=0 disables scene change detection for consistent keyframe placement
        let key = CString::new("x264-params").unwrap();
        let val = CString::new("nal-hrd=cbr:force-cfr=1:scenecut=0").unwrap();
        ffi::av_opt_set(enc_ctx.cast(), key.as_ptr(), val.as_ptr(), 0);
        log::debug!("libx264: enabled NAL-HRD CBR mode, disabled scenecut");
    } else if name_lower == "libx265" {
        // x265-params: nal-hrd=cbr enables NAL HRD signaling for HEVC
        // scenecut=0 disables scene change detection
        let key = CString::new("x265-params").unwrap();
        let val = CString::new("nal-hrd=cbr:scenecut=0").unwrap();
        ffi::av_opt_set(enc_ctx.cast(), key.as_ptr(), val.as_ptr(), 0);
        log::debug!("libx265: enabled NAL-HRD CBR mode, disabled scenecut");
    }
}

fn hw_device_type_for_encoder(encoder_name: &str) -> Option<ffi::AVHWDeviceType> {
    let name = encoder_name.to_ascii_lowercase();
    if name.contains("nvenc") {
        Some(ffi::AVHWDeviceType::AV_HWDEVICE_TYPE_CUDA)
    } else if name.contains("qsv") {
        Some(ffi::AVHWDeviceType::AV_HWDEVICE_TYPE_QSV)
    } else if name.contains("amf") {
        Some(ffi::AVHWDeviceType::AV_HWDEVICE_TYPE_D3D11VA)
    } else if name.contains("videotoolbox") {
        Some(ffi::AVHWDeviceType::AV_HWDEVICE_TYPE_VIDEOTOOLBOX)
    } else {
        None
    }
}

fn hw_pix_fmt_for_encoder(encoder_name: &str) -> Option<ffi::AVPixelFormat> {
    let name = encoder_name.to_ascii_lowercase();
    if name.contains("nvenc") {
        Some(ffi::AVPixelFormat::AV_PIX_FMT_CUDA)
    } else if name.contains("qsv") {
        Some(ffi::AVPixelFormat::AV_PIX_FMT_QSV)
    } else if name.contains("amf") {
        Some(ffi::AVPixelFormat::AV_PIX_FMT_D3D11)
    } else if name.contains("videotoolbox") {
        // VideoToolbox can accept software frames directly (NV12/YUV420P)
        // or hardware surfaces (AV_PIX_FMT_VIDEOTOOLBOX).
        // For simplicity, we use software frames with device context attached.
        None
    } else {
        None
    }
}

fn attach_hw_device(
    encoder_name: &str,
    enc_ctx: *mut ffi::AVCodecContext,
) -> Option<*mut ffi::AVBufferRef> {
    let device_type = hw_device_type_for_encoder(encoder_name)?;

    let mut device_ctx: *mut ffi::AVBufferRef = ptr::null_mut();
    let ret = unsafe {
        ffi::av_hwdevice_ctx_create(&mut device_ctx, device_type, ptr::null(), ptr::null_mut(), 0)
    };
    if ret < 0 || device_ctx.is_null() {
        log::debug!(
            "FFmpeg libs hw device init failed for {}: {}",
            encoder_name,
            ffmpeg_err(ret)
        );
        return None;
    }

    let device_ref = unsafe { ffi::av_buffer_ref(device_ctx) };
    unsafe { ffi::av_buffer_unref(&mut device_ctx) };
    if device_ref.is_null() {
        return None;
    }

    let enc_ref = unsafe { ffi::av_buffer_ref(device_ref) };
    if enc_ref.is_null() {
        unsafe { ffi::av_buffer_unref(&mut (device_ref as *mut _)) };
        return None;
    }

    unsafe {
        (*enc_ctx).hw_device_ctx = enc_ref;
    }

    Some(device_ref)
}

fn create_hw_frames_ctx(
    device_ref: *mut ffi::AVBufferRef,
    hw_fmt: ffi::AVPixelFormat,
    sw_fmt: ffi::AVPixelFormat,
    width: i32,
    height: i32,
) -> Result<*mut ffi::AVBufferRef, String> {
    let mut frames_ref = unsafe { ffi::av_hwframe_ctx_alloc(device_ref) };
    if frames_ref.is_null() {
        return Err("Failed to allocate hardware frames context".to_string());
    }

    unsafe {
        let frames_ctx = (*frames_ref).data as *mut ffi::AVHWFramesContext;
        if frames_ctx.is_null() {
            ffi::av_buffer_unref(&mut frames_ref);
            return Err("Hardware frames context was null".to_string());
        }
        (*frames_ctx).format = hw_fmt;
        (*frames_ctx).sw_format = sw_fmt;
        (*frames_ctx).width = width;
        (*frames_ctx).height = height;
        (*frames_ctx).initial_pool_size = 20;
    }

    let init_ret = unsafe { ffi::av_hwframe_ctx_init(frames_ref) };
    if init_ret < 0 {
        unsafe { ffi::av_buffer_unref(&mut frames_ref) };
        return Err(format!("Failed to init hardware frames context: {}", ffmpeg_err(init_ret)));
    }

    Ok(frames_ref)
}

/// Get the hardware decoder name that pairs with a given hardware encoder.
/// This enables zero-copy transcoding where decode and encode share the same device.
///
/// # Arguments
/// * `encoder_name` - The name of the hardware encoder (e.g., "h264_nvenc", "hevc_qsv")
/// * `input_codec_id` - The FFmpeg codec ID of the input stream
///
/// # Returns
/// * `Some(decoder_name)` if a matching hardware decoder exists
/// * `None` if no hardware decoder is available or recommended
///
/// # Zero-Copy Path
/// When the hardware decoder and encoder share the same device context, frames
/// can stay on GPU throughout the transcode pipeline:
/// - NVDEC + NVENC: Share CUDA device, frames in CUDA memory
/// - QSV decode + QSV encode: Share QSV device, frames in QSV memory
/// - VideoToolbox decode + VideoToolbox encode: Share VT device
///
/// This eliminates CPU↔GPU transfers, significantly improving performance for
/// same-resolution transcodes or when GPU-accelerated scaling is available.
fn hw_decoder_for_encoder(encoder_name: &str, input_codec_id: ffi::AVCodecID) -> Option<&'static str> {
    let name = encoder_name.to_ascii_lowercase();

    // NVIDIA NVENC pairs with NVDEC (CUVID)
    if name.contains("nvenc") {
        return match input_codec_id {
            ffi::AVCodecID::AV_CODEC_ID_H264 => Some("h264_cuvid"),
            ffi::AVCodecID::AV_CODEC_ID_HEVC => Some("hevc_cuvid"),
            ffi::AVCodecID::AV_CODEC_ID_VP8 => Some("vp8_cuvid"),
            ffi::AVCodecID::AV_CODEC_ID_VP9 => Some("vp9_cuvid"),
            ffi::AVCodecID::AV_CODEC_ID_AV1 => Some("av1_cuvid"),
            ffi::AVCodecID::AV_CODEC_ID_MPEG4 => Some("mpeg4_cuvid"),
            ffi::AVCodecID::AV_CODEC_ID_MPEG2VIDEO => Some("mpeg2_cuvid"),
            _ => None,
        };
    }

    // Intel QSV encode pairs with QSV decode
    if name.contains("qsv") {
        return match input_codec_id {
            ffi::AVCodecID::AV_CODEC_ID_H264 => Some("h264_qsv"),
            ffi::AVCodecID::AV_CODEC_ID_HEVC => Some("hevc_qsv"),
            ffi::AVCodecID::AV_CODEC_ID_VP8 => Some("vp8_qsv"),
            ffi::AVCodecID::AV_CODEC_ID_VP9 => Some("vp9_qsv"),
            ffi::AVCodecID::AV_CODEC_ID_AV1 => Some("av1_qsv"),
            ffi::AVCodecID::AV_CODEC_ID_MPEG2VIDEO => Some("mpeg2_qsv"),
            ffi::AVCodecID::AV_CODEC_ID_MJPEG => Some("mjpeg_qsv"),
            _ => None,
        };
    }

    // AMD AMF doesn't have matching hardware decoders in FFmpeg
    // (AMF is encode-only, decode uses D3D11VA which is different)
    if name.contains("amf") {
        return None;
    }

    // Apple VideoToolbox has unified decode/encode
    if name.contains("videotoolbox") {
        return match input_codec_id {
            ffi::AVCodecID::AV_CODEC_ID_H264 => Some("h264_videotoolbox"),
            ffi::AVCodecID::AV_CODEC_ID_HEVC => Some("hevc_videotoolbox"),
            _ => None,
        };
    }

    None
}

/// Check if zero-copy transcoding is possible between decoder and encoder.
/// Zero-copy requires:
/// 1. Hardware decoder and encoder on same device type
/// 2. Compatible pixel formats (or GPU scaling available)
/// 3. Same resolution (or GPU scaling available)
///
/// # Returns
/// * `true` if frames can stay on GPU throughout
/// * `false` if CPU transfer is required
fn can_use_zero_copy(
    encoder_name: &str,
    input_width: i32,
    input_height: i32,
    output_width: i32,
    output_height: i32,
) -> bool {
    let name = encoder_name.to_ascii_lowercase();

    // NVIDIA NVENC + NVDEC: Zero-copy supported with CUDA scaling
    // GPU can handle resolution changes via CUDA NPP or scale_cuda filter
    if name.contains("nvenc") {
        // NVENC/NVDEC support zero-copy even with resolution changes
        // via scale_cuda filter or NPP scaling
        return true;
    }

    // Intel QSV: Zero-copy requires same resolution or vpp_qsv filter
    if name.contains("qsv") {
        // QSV can do GPU scaling via vpp_qsv filter
        // For now, only enable zero-copy for same resolution
        // TODO: Add vpp_qsv filter support for GPU scaling
        return input_width == output_width && input_height == output_height;
    }

    // VideoToolbox: Zero-copy supported, system handles scaling
    if name.contains("videotoolbox") {
        return true;
    }

    // AMF: No hardware decoder, can't do zero-copy
    false
}

/// Attempt to initialize hardware decoder for zero-copy path.
/// Falls back to software decoder if hardware decoder is unavailable.
///
/// # Arguments
/// * `encoder_name` - The hardware encoder being used
/// * `input_codecpar` - The codec parameters from the input stream
/// * `hw_device_ctx` - The hardware device context (shared with encoder)
///
/// # Returns
/// * `Ok((decoder_ctx, is_hw_decode))` - Decoder context and whether it's hardware
/// * `Err(reason)` - Why hardware decode couldn't be initialized
unsafe fn try_init_hw_decoder(
    encoder_name: &str,
    input_codecpar: *const ffi::AVCodecParameters,
    hw_device_ctx: *mut ffi::AVBufferRef,
) -> Result<(*mut ffi::AVCodecContext, bool), String> {
    let input_codec_id = (*input_codecpar).codec_id;

    // Check if there's a matching hardware decoder
    let hw_decoder_name = match hw_decoder_for_encoder(encoder_name, input_codec_id) {
        Some(name) => name,
        None => {
            return Err(format!(
                "No hardware decoder available for encoder {} and codec {:?}",
                encoder_name, input_codec_id
            ));
        }
    };

    // Find the hardware decoder
    let decoder_name_cstr = match CString::new(hw_decoder_name) {
        Ok(s) => s,
        Err(_) => return Err("Invalid decoder name".to_string()),
    };

    let hw_decoder = ffi::avcodec_find_decoder_by_name(decoder_name_cstr.as_ptr());
    if hw_decoder.is_null() {
        return Err(format!("Hardware decoder {} not found in FFmpeg build", hw_decoder_name));
    }

    // Allocate decoder context
    let dec_ctx = ffi::avcodec_alloc_context3(hw_decoder);
    if dec_ctx.is_null() {
        return Err("Failed to allocate hardware decoder context".to_string());
    }

    // Copy codec parameters
    let params_ret = ffi::avcodec_parameters_to_context(dec_ctx, input_codecpar);
    if params_ret < 0 {
        ffi::avcodec_free_context(&mut (dec_ctx as *mut _));
        return Err(format!("Failed to copy codec params to hw decoder: {}", ffmpeg_err(params_ret)));
    }

    // Attach the hardware device context (shared with encoder)
    if !hw_device_ctx.is_null() {
        let dec_device_ref = ffi::av_buffer_ref(hw_device_ctx);
        if !dec_device_ref.is_null() {
            (*dec_ctx).hw_device_ctx = dec_device_ref;
        }
    }

    // Open the decoder
    let open_ret = ffi::avcodec_open2(dec_ctx, hw_decoder, ptr::null_mut());
    if open_ret < 0 {
        ffi::avcodec_free_context(&mut (dec_ctx as *mut _));
        return Err(format!("Failed to open hardware decoder {}: {}", hw_decoder_name, ffmpeg_err(open_ret)));
    }

    log::info!(
        "Initialized hardware decoder {} for zero-copy transcode with {}",
        hw_decoder_name, encoder_name
    );

    Ok((dec_ctx, true))
}

fn ffmpeg_err(code: i32) -> String {
    let mut buf = [0i8; ffi::AV_ERROR_MAX_STRING_SIZE as usize];
    unsafe {
        ffi::av_strerror(code, buf.as_mut_ptr(), buf.len());
        CStr::from_ptr(buf.as_ptr()).to_string_lossy().into_owned()
    }
}

fn parse_bitrate_to_bits(value: &str) -> Option<i64> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    let split_at = trimmed
        .find(|c: char| !c.is_ascii_digit() && c != '.')
        .unwrap_or(trimmed.len());
    let (num_str, suffix) = trimmed.split_at(split_at);
    let number: f64 = num_str.parse().ok()?;
    let multiplier = match suffix.trim().to_ascii_lowercase().as_str() {
        "k" | "kbps" | "kbit" | "kbits" | "kbit/s" | "kbits/s" => 1_000.0,
        "m" | "mbps" | "mbit" | "mbits" | "mbit/s" | "mbits/s" => 1_000_000.0,
        "g" | "gbps" | "gbit" | "gbits" | "gbit/s" | "gbits/s" => 1_000_000_000.0,
        _ => 1.0,
    };
    Some((number * multiplier) as i64)
}

fn select_pix_fmt(encoder: *const ffi::AVCodec, prefer_nv12: bool) -> ffi::AVPixelFormat {
    if encoder.is_null() {
        return ffi::AVPixelFormat::AV_PIX_FMT_YUV420P;
    }
    unsafe {
        let mut formats = (*encoder).pix_fmts;
        if formats.is_null() {
            return ffi::AVPixelFormat::AV_PIX_FMT_YUV420P;
        }
        let mut fallback = ffi::AVPixelFormat::AV_PIX_FMT_YUV420P;
        while *formats != ffi::AVPixelFormat::AV_PIX_FMT_NONE {
            if prefer_nv12 && *formats == ffi::AVPixelFormat::AV_PIX_FMT_NV12 {
                return *formats;
            }
            if *formats == ffi::AVPixelFormat::AV_PIX_FMT_YUV420P || *formats == ffi::AVPixelFormat::AV_PIX_FMT_NV12 {
                fallback = *formats;
            }
            formats = formats.add(1);
        }
        fallback
    }
}

fn select_sample_fmt(encoder: *const ffi::AVCodec, fallback: ffi::AVSampleFormat) -> ffi::AVSampleFormat {
    if encoder.is_null() {
        return fallback;
    }
    unsafe {
        let mut formats = (*encoder).sample_fmts;
        if formats.is_null() {
            return fallback;
        }
        while *formats != ffi::AVSampleFormat::AV_SAMPLE_FMT_NONE {
            if *formats == ffi::AVSampleFormat::AV_SAMPLE_FMT_FLTP
                || *formats == ffi::AVSampleFormat::AV_SAMPLE_FMT_S16 {
                return *formats;
            }
            formats = formats.add(1);
        }
    }
    fallback
}
