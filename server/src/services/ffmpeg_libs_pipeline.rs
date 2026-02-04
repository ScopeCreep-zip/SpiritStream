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
    atomic::{AtomicBool, Ordering},
    mpsc::{self, Sender, Receiver},
};
use std::thread::{self, JoinHandle};

use ffmpeg_sys_next as ffi;

use crate::models::{OutputGroup, StreamTarget};

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
        }
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

        let handle = thread::spawn(move || {
            run_pipeline_loop(
                &input_url,
                expected_stream_key.as_deref(),
                groups,
                group_controls,
                stop_flag,
                command_rx,
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
}

struct GroupOutputs {
    group_id: String,
    control: GroupControl,
    targets: Vec<TargetOutput>,
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
    sws_ctx: *mut ffi::SwsContext,
    swr_ctx: Option<*mut ffi::SwrContext>,
    video_dec_frame: *mut ffi::AVFrame,
    video_sw_frame: *mut ffi::AVFrame,
    video_hw_frame: Option<*mut ffi::AVFrame>,
    audio_dec_frame: *mut ffi::AVFrame,
    outputs: Vec<TranscodeOutput>,
    /// Track if this group has been cleaned up
    cleaned_up: bool,
}

struct TranscodeOutput {
    ctx: *mut ffi::AVFormatContext,
    video_out_index: i32,
    audio_out_index: Option<i32>,
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
) -> Result<(), String> {
    unsafe {
        ffi::avformat_network_init();
    }

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
            }
        }

        if let Some(ref mut tg) = transcode_group {
            if !tg.cleaned_up && tg.control.should_stop() {
                log::info!("Cleaning up stopped transcode group: {}", tg.group_id);
                flush_transcode_group(tg)?;
                cleanup_transcode_group_outputs(tg);
                tg.cleaned_up = true;
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

        if stream_index == video_stream_index {
            write_passthrough_packet(packet, in_stream, &group_outputs);
            if let Some(group) = transcode_group.as_mut() {
                if !group.cleaned_up && group.control.is_enabled() {
                    transcode_video_packet(group, in_stream, packet)?;
                }
            }
        } else if audio_stream_index == Some(stream_index) {
            write_passthrough_packet(packet, in_stream, &group_outputs);
            if let Some(group) = transcode_group.as_mut() {
                if !group.cleaned_up && group.control.is_enabled() {
                    transcode_audio_packet(group, in_stream, packet)?;
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

        unsafe {
            (*out_stream).time_base = (*in_stream).time_base;
        }
        out_streams[idx] = out_stream;
    }

    let mut opts: *mut ffi::AVDictionary = ptr::null_mut();
    let flvflags = CString::new("no_duration_filesize").unwrap();
    unsafe {
        ffi::av_dict_set(&mut opts, CString::new("flvflags").unwrap().as_ptr(), flvflags.as_ptr(), 0);
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
    })
}

fn write_passthrough_packet(
    packet: *mut ffi::AVPacket,
    in_stream: *mut ffi::AVStream,
    group_outputs: &[GroupOutputs],
) {
    for group in group_outputs {
        // Skip groups that are stopped or disabled
        if group.cleaned_up || !group.control.is_enabled() {
            continue;
        }

        for target in &group.targets {
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

            unsafe {
                ffi::av_packet_rescale_ts(pkt_clone, (*in_stream).time_base, (*out_stream).time_base);
                (*pkt_clone).stream_index = (*out_stream).index;
                let write_ret = ffi::av_interleaved_write_frame(target.ctx, pkt_clone);
                if write_ret < 0 {
                    log::warn!(
                        "FFmpeg libs write failed for group {}: {}",
                        group.group_id,
                        ffmpeg_err(write_ret)
                    );
                }
                ffi::av_packet_free(&mut pkt_clone);
            }
        }
    }
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

    let video_decoder = unsafe { ffi::avcodec_find_decoder((*video_codecpar).codec_id) };
    if video_decoder.is_null() {
        return Err("Failed to find video decoder".to_string());
    }

    let video_dec_ctx = unsafe { ffi::avcodec_alloc_context3(video_decoder) };
    if video_dec_ctx.is_null() {
        return Err("Failed to allocate video decoder context".to_string());
    }

    let dec_ret = unsafe { ffi::avcodec_parameters_to_context(video_dec_ctx, video_codecpar) };
    if dec_ret < 0 {
        unsafe { ffi::avcodec_free_context(&mut (video_dec_ctx as *mut _)) };
        return Err(format!("Failed to copy video decoder parameters: {}", ffmpeg_err(dec_ret)));
    }

    let open_dec_ret = unsafe { ffi::avcodec_open2(video_dec_ctx, video_decoder, ptr::null_mut()) };
    if open_dec_ret < 0 {
        unsafe { ffi::avcodec_free_context(&mut (video_dec_ctx as *mut _)) };
        return Err(format!("Failed to open video decoder: {}", ffmpeg_err(open_dec_ret)));
    }

    let video_encoder_name = CString::new(group.video.codec.clone())
        .map_err(|_| "Video codec contains null byte".to_string())?;
    let video_encoder = unsafe { ffi::avcodec_find_encoder_by_name(video_encoder_name.as_ptr()) };
    if video_encoder.is_null() {
        unsafe { ffi::avcodec_free_context(&mut (video_dec_ctx as *mut _)) };
        return Err(format!("Video encoder not found: {}", group.video.codec));
    }

    let video_enc_ctx = unsafe { ffi::avcodec_alloc_context3(video_encoder) };
    if video_enc_ctx.is_null() {
        unsafe { ffi::avcodec_free_context(&mut (video_dec_ctx as *mut _)) };
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
    let output_width = if group.video.width > 0 { group.video.width as i32 } else { unsafe { (*video_codecpar).width } };
    let output_height = if group.video.height > 0 { group.video.height as i32 } else { unsafe { (*video_codecpar).height } };

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
        }
        if let Some(interval) = group.video.keyframe_interval_seconds {
            if output_fps.num > 0 {
                (*video_enc_ctx).gop_size = (output_fps.num as i32).saturating_mul(interval as i32);
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
            ffi::avcodec_free_context(&mut (video_dec_ctx as *mut _));
        }
        return Err(format!("Failed to open video encoder: {}", ffmpeg_err(open_enc_ret)));
    }

    let sws_ctx = unsafe {
        ffi::sws_getContext(
            (*video_dec_ctx).width,
            (*video_dec_ctx).height,
            (*video_dec_ctx).pix_fmt,
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
        sws_ctx,
        swr_ctx,
        video_dec_frame,
        video_sw_frame,
        video_hw_frame,
        audio_dec_frame,
        outputs,
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
                }
                audio_out_index = Some(unsafe { (*out_stream).index });
            }
        }

        let mut opts: *mut ffi::AVDictionary = ptr::null_mut();
        unsafe {
            ffi::av_dict_set(&mut opts, CString::new("flvflags").unwrap().as_ptr(), CString::new("no_duration_filesize").unwrap().as_ptr(), 0);
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

        outputs.push(TranscodeOutput {
            ctx: output_ctx,
            video_out_index: unsafe { (*video_stream).index },
            audio_out_index,
        });
    }

    Ok(outputs)
}

fn transcode_video_packet(
    group: &mut TranscodeGroup,
    in_stream: *mut ffi::AVStream,
    packet: *mut ffi::AVPacket,
) -> Result<(), String> {
    let send_ret = unsafe { ffi::avcodec_send_packet(group.video_dec_ctx, packet) };
    if send_ret < 0 {
        return Err(format!("Video decoder send failed: {}", ffmpeg_err(send_ret)));
    }

    loop {
        let receive_ret = unsafe { ffi::avcodec_receive_frame(group.video_dec_ctx, group.video_dec_frame) };
        if receive_ret < 0 {
            break;
        }

        unsafe {
            let writable_ret = ffi::av_frame_make_writable(group.video_sw_frame);
            if writable_ret < 0 {
                return Err(format!("Video frame not writable: {}", ffmpeg_err(writable_ret)));
            }
            ffi::sws_scale(
                group.sws_ctx,
                (*group.video_dec_frame).data.as_ptr() as *const *const u8,
                (*group.video_dec_frame).linesize.as_ptr(),
                0,
                (*group.video_dec_ctx).height,
                (*group.video_sw_frame).data.as_mut_ptr(),
                (*group.video_sw_frame).linesize.as_mut_ptr(),
            );
            (*group.video_sw_frame).pts = ffi::av_rescale_q(
                (*group.video_dec_frame).pts,
                (*in_stream).time_base,
                (*group.video_enc_ctx).time_base,
            );
        }

        let mut frame_to_send = group.video_sw_frame;
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
                let transfer_ret = ffi::av_hwframe_transfer_data(hw_frame, group.video_sw_frame, 0);
                if transfer_ret < 0 {
                    return Err(format!("Failed to upload hw frame: {}", ffmpeg_err(transfer_ret)));
                }
                (*hw_frame).pts = (*group.video_sw_frame).pts;
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
    for output in &group.outputs {
        let out_index = if is_video {
            output.video_out_index
        } else {
            match output.audio_out_index {
                Some(idx) => idx,
                None => continue,
            }
        };

        let mut pkt_clone = unsafe { ffi::av_packet_clone(enc_pkt) };
        if pkt_clone.is_null() {
            continue;
        }
        unsafe {
            let out_stream = *(*output.ctx).streams.add(out_index as usize);
            let time_base = if is_video {
                (*group.video_enc_ctx).time_base
            } else {
                (*group.audio_enc_ctx.unwrap()).time_base
            };
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
            ffi::av_packet_free(&mut pkt_clone);
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
        // Audio copy path.
        let mut pkt_clone = unsafe { ffi::av_packet_clone(packet) };
        if pkt_clone.is_null() {
            return Ok(());
        }

        for output in &group.outputs {
            let out_index = match output.audio_out_index {
                Some(idx) => idx,
                None => continue,
            };

            let mut pkt_target = unsafe { ffi::av_packet_clone(pkt_clone) };
            if pkt_target.is_null() {
                continue;
            }
            unsafe {
                let out_stream = *(*output.ctx).streams.add(out_index as usize);
                ffi::av_packet_rescale_ts(pkt_target, (*in_stream).time_base, (*out_stream).time_base);
                (*pkt_target).stream_index = out_index;
                let write_ret = ffi::av_interleaved_write_frame(output.ctx, pkt_target);
                if write_ret < 0 {
                    log::warn!(
                        "FFmpeg libs audio copy write failed for group {}: {}",
                        group.group_id,
                        ffmpeg_err(write_ret)
                    );
                }
                ffi::av_packet_free(&mut pkt_target);
            }
        }

        unsafe { ffi::av_packet_free(&mut pkt_clone) };
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
        for output in group.outputs {
            let _ = ffi::av_write_trailer(output.ctx);
            if (*(*output.ctx).oformat).flags & ffi::AVFMT_NOFILE == 0 {
                let _ = ffi::avio_closep(&mut (*output.ctx).pb);
            }
            ffi::avformat_free_context(output.ctx);
        }
        ffi::av_frame_free(&mut (group.video_dec_frame as *mut _));
        ffi::av_frame_free(&mut (group.video_sw_frame as *mut _));
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

    // Apply preset if provided
    if let Some(preset_val) = preset {
        let preset_c = CString::new(preset_val).unwrap_or_default();

        // Different encoders use different preset option names
        if name_lower.contains("nvenc") {
            let key = CString::new("preset").unwrap();
            // NVENC presets: p1-p7 or names like "fast", "medium", "slow"
            let preset_lower = preset_val.to_lowercase();
            let nvenc_preset = match preset_lower.as_str() {
                "ultrafast" | "superfast" | "veryfast" => "p1",
                "faster" | "fast" => "p2",
                "medium" => "p4",
                "slow" => "p5",
                "slower" | "veryslow" => "p6",
                "placebo" => "p7",
                p if p.starts_with('p') => p, // Already p1-p7 format
                _ => preset_val,
            };
            let val = CString::new(nvenc_preset).unwrap_or_default();
            ffi::av_opt_set(enc_ctx.cast(), key.as_ptr(), val.as_ptr(), 0);
        } else if name_lower.contains("qsv") {
            let key = CString::new("preset").unwrap();
            // QSV presets: veryfast, faster, fast, medium, slow, slower, veryslow
            ffi::av_opt_set(enc_ctx.cast(), key.as_ptr(), preset_c.as_ptr(), 0);
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

    // Apply Twitch-safe QSV overrides
    // Twitch has strict requirements for QSV: no B-frames, no lookahead, forced IDR
    if is_twitch_target && name_lower.contains("qsv") {
        log::info!("Applying Twitch-safe QSV overrides");

        // Disable B-frames
        (*enc_ctx).max_b_frames = 0;

        // Disable lookahead
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

        // Repeat PPS/SPS for each IDR
        let key = CString::new("repeat_pps").unwrap();
        let val = CString::new("1").unwrap();
        ffi::av_opt_set(enc_ctx.cast(), key.as_ptr(), val.as_ptr(), 0);
    }

    // Apply common hardware encoder optimizations
    if name_lower.contains("nvenc") {
        // NVENC tuning for streaming
        let key = CString::new("tune").unwrap();
        let val = CString::new("ll").unwrap(); // low latency
        ffi::av_opt_set(enc_ctx.cast(), key.as_ptr(), val.as_ptr(), 0);

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

        // Low latency mode
        let key = CString::new("usage").unwrap();
        let val = CString::new("lowlatency").unwrap();
        ffi::av_opt_set(enc_ctx.cast(), key.as_ptr(), val.as_ptr(), 0);

        // Disable B-frames for streaming (improves latency and compatibility)
        (*enc_ctx).max_b_frames = 0;
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
