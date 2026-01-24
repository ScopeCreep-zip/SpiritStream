// FFmpeg libs pipeline (in-process).
// This module is feature-gated so we can build the new pipeline without
// touching the existing FFmpeg CLI flow.

#![cfg(feature = "ffmpeg-libs")]

use std::ffi::{CStr, CString};
use std::ptr;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::thread::{self, JoinHandle};

use ffmpeg_sys_next as ffi;

use crate::models::OutputGroup;

#[derive(Debug, Clone)]
pub struct InputPipelineConfig {
    pub input_id: String,
    pub input_url: String,
}

#[derive(Debug, Clone)]
pub struct OutputGroupConfig {
    pub group_id: String,
    pub mode: OutputGroupMode,
    pub targets: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputGroupMode {
    Passthrough,
    Transcode,
}

pub struct InputPipeline {
    input_id: String,
    input_url: String,
    groups: Vec<OutputGroupConfig>,
    stop_flag: Arc<AtomicBool>,
    thread: Option<JoinHandle<Result<(), String>>>,
}

impl InputPipeline {
    pub fn new(config: InputPipelineConfig) -> Self {
        Self {
            input_id: config.input_id,
            input_url: config.input_url,
            groups: Vec::new(),
            stop_flag: Arc::new(AtomicBool::new(false)),
            thread: None,
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
            group_id: group.id,
            mode,
            targets,
        });

        Ok(())
    }

    pub fn start(&mut self) -> Result<(), String> {
        if self.thread.is_some() {
            return Err("FFmpeg libs pipeline already started".to_string());
        }

        if self.groups.iter().any(|group| group.mode == OutputGroupMode::Transcode) {
            return Err("Transcode groups are not implemented in ffmpeg-libs pipeline yet".to_string());
        }

        let input_url = self.input_url.clone();
        let stop_flag = Arc::clone(&self.stop_flag);
        let groups = self.groups.clone();

        let handle = thread::spawn(move || run_passthrough_loop(&input_url, groups, stop_flag));
        self.thread = Some(handle);
        Ok(())
    }

    pub fn stop(&mut self) {
        self.stop_flag.store(true, Ordering::SeqCst);
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
    targets: Vec<TargetOutput>,
}

fn run_passthrough_loop(
    input_url: &str,
    groups: Vec<OutputGroupConfig>,
    stop_flag: Arc<AtomicBool>,
) -> Result<(), String> {
    unsafe {
        ffi::avformat_network_init();
    }

    let mut input_ctx: *mut ffi::AVFormatContext = ptr::null_mut();
    let input_url_c = CString::new(input_url)
        .map_err(|_| "Input URL contains null byte".to_string())?;

    let open_ret = unsafe {
        ffi::avformat_open_input(
            &mut input_ctx,
            input_url_c.as_ptr(),
            ptr::null_mut(),
            ptr::null_mut(),
        )
    };
    if open_ret < 0 {
        return Err(format!("Failed to open input: {}", ffmpeg_err(open_ret)));
    }

    let info_ret = unsafe { ffi::avformat_find_stream_info(input_ctx, ptr::null_mut()) };
    if info_ret < 0 {
        unsafe { ffi::avformat_close_input(&mut input_ctx) };
        return Err(format!("Failed to read stream info: {}", ffmpeg_err(info_ret)));
    }

    let group_outputs = create_group_outputs(input_ctx, &groups)?;

    let mut packet = unsafe { ffi::av_packet_alloc() };
    if packet.is_null() {
        cleanup_outputs(group_outputs);
        unsafe { ffi::avformat_close_input(&mut input_ctx) };
        return Err("Failed to allocate AVPacket".to_string());
    }

    loop {
        if stop_flag.load(Ordering::SeqCst) {
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
        if codec_type != ffi::AVMEDIA_TYPE_VIDEO && codec_type != ffi::AVMEDIA_TYPE_AUDIO {
            unsafe { ffi::av_packet_unref(packet) };
            continue;
        }

        for group in &group_outputs {
            for target in &group.targets {
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

        unsafe { ffi::av_packet_unref(packet) };
    }

    unsafe { ffi::av_packet_free(&mut packet) };
    cleanup_outputs(group_outputs);
    unsafe { ffi::avformat_close_input(&mut input_ctx) };

    Ok(())
}

fn create_group_outputs(
    input_ctx: *mut ffi::AVFormatContext,
    groups: &[OutputGroupConfig],
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

        outputs.push(GroupOutputs {
            group_id: group.group_id.clone(),
            targets,
        });
    }

    Ok(outputs)
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
        if codec_type != ffi::AVMEDIA_TYPE_VIDEO && codec_type != ffi::AVMEDIA_TYPE_AUDIO {
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

fn cleanup_outputs(groups: Vec<GroupOutputs>) {
    for group in groups {
        for mut target in group.targets {
            unsafe {
                let _ = ffi::av_write_trailer(target.ctx);
                if (*(*target.ctx).oformat).flags & ffi::AVFMT_NOFILE == 0 {
                    let _ = ffi::avio_closep(&mut (*target.ctx).pb);
                }
                ffi::avformat_free_context(target.ctx);
            }
        }
    }
}

fn ffmpeg_err(code: i32) -> String {
    let mut buf = [0i8; ffi::AV_ERROR_MAX_STRING_SIZE as usize];
    unsafe {
        ffi::av_strerror(code, buf.as_mut_ptr(), buf.len());
        CStr::from_ptr(buf.as_ptr()).to_string_lossy().into_owned()
    }
}
