// Native Capture Support
// FFmpeg stdin pipe handling for raw video/audio frame input

use std::process::{Command, Stdio};
use std::sync::Arc;
use std::thread;
use std::time::Instant;

use crate::models::OutputGroup;
use crate::services::EventSink;

use super::ffmpeg_stats::{self, ProcessInfo};
use super::FFmpegHandler;

use crate::services::process_util::configure_hidden_window;

/// Configuration for native video input
#[derive(Debug, Clone)]
pub struct NativeVideoConfig {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub pixel_format: String, // e.g., "rgb24", "bgra", "nv12"
}

/// Configuration for native audio input
#[derive(Debug, Clone)]
pub struct NativeAudioConfig {
    pub sample_rate: u32,
    pub channels: u16,
    pub sample_format: String, // e.g., "f32le", "s16le"
}

/// Handle for a native capture stream with stdin pipe
pub struct NativeCaptureHandle {
    pub stdin: std::process::ChildStdin,
    pub group_id: String,
}

impl FFmpegHandler {
    /// Spawn an FFmpeg process for native capture and return the stdin handle.
    ///
    /// This is shared logic for `start_from_native_video`, `start_from_native_audio`,
    /// and `start_from_native_av`.
    fn spawn_native_capture(
        &self,
        group: &OutputGroup,
        args: Vec<String>,
        event_sink: Arc<dyn EventSink>,
    ) -> Result<NativeCaptureHandle, String> {
        let mut cmd = Command::new(&self.ffmpeg_path);
        cmd.args(&args)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());
        configure_hidden_window(&mut cmd);

        let mut child = cmd.spawn()
            .map_err(|e| format!("Failed to start FFmpeg: {e}"))?;

        let stdin = child.stdin.take()
            .ok_or_else(|| "Failed to capture FFmpeg stdin".to_string())?;

        let stderr = child.stderr.take()
            .ok_or_else(|| "Failed to capture FFmpeg stderr".to_string())?;

        let group_id = group.id.clone();

        self.processes.insert(group_id.clone(), ProcessInfo {
            child,
            start_time: Instant::now(),
            group_id: group_id.clone(),
        });

        // Start stats reader thread
        let event_sink_clone = Arc::clone(&event_sink);
        let processes_clone = Arc::clone(&self.processes);
        let meter_bytes = ffmpeg_stats::start_bitrate_meter(&group_id, Arc::clone(&processes_clone));
        let relay_clone = Arc::clone(self.relay.relay());
        let stopping_clone = Arc::clone(&self.stopping_groups);
        let relay_refcount_clone = Arc::clone(self.relay.relay_refcount());
        let group_id_clone = group_id.clone();

        thread::spawn(move || {
            ffmpeg_stats::stats_reader(
                stderr,
                group_id_clone,
                meter_bytes,
                event_sink_clone,
                processes_clone,
                stopping_clone,
                relay_clone,
                relay_refcount_clone,
            );
        });

        Ok(NativeCaptureHandle {
            stdin,
            group_id,
        })
    }

    /// Start streaming from raw video frames (native capture)
    ///
    /// This creates an FFmpeg process that reads raw video from stdin,
    /// encodes it according to the group settings, and outputs to targets.
    ///
    /// Returns a handle with the stdin pipe for writing frames.
    pub fn start_from_native_video(
        &self,
        group: &OutputGroup,
        video_config: &NativeVideoConfig,
        event_sink: Arc<dyn EventSink>,
    ) -> Result<NativeCaptureHandle, String> {
        // Check if already streaming this group
        if let Some(pid) = self.get_group_pid(&group.id) {
            return Err(format!("Group {} already streaming (pid {})", group.id, pid));
        }

        let args = self.build_native_video_args(group, video_config);
        let sanitized = self.sanitize_ffmpeg_args(&args, group);
        log::info!(
            "Starting FFmpeg native video capture for group {}: {} {}",
            group.id,
            self.ffmpeg_path,
            sanitized.join(" ")
        );

        self.spawn_native_capture(group, args, event_sink)
    }

    /// Start streaming from raw audio samples (native capture)
    ///
    /// This creates an FFmpeg process that reads raw audio from stdin,
    /// encodes it according to the group settings, and outputs to targets.
    pub fn start_from_native_audio(
        &self,
        group: &OutputGroup,
        audio_config: &NativeAudioConfig,
        event_sink: Arc<dyn EventSink>,
    ) -> Result<NativeCaptureHandle, String> {
        if let Some(pid) = self.get_group_pid(&group.id) {
            return Err(format!("Group {} already streaming (pid {})", group.id, pid));
        }

        let args = self.build_native_audio_args(group, audio_config);
        let sanitized = self.sanitize_ffmpeg_args(&args, group);
        log::info!(
            "Starting FFmpeg native audio capture for group {}: {} {}",
            group.id,
            self.ffmpeg_path,
            sanitized.join(" ")
        );

        self.spawn_native_capture(group, args, event_sink)
    }

    /// Start streaming with both native video and audio
    ///
    /// Creates an FFmpeg process with two stdin pipes (video and audio)
    /// using named pipes or a more complex setup.
    pub fn start_from_native_av(
        &self,
        group: &OutputGroup,
        video_config: &NativeVideoConfig,
        audio_config: &NativeAudioConfig,
        event_sink: Arc<dyn EventSink>,
    ) -> Result<NativeCaptureHandle, String> {
        if let Some(pid) = self.get_group_pid(&group.id) {
            return Err(format!("Group {} already streaming (pid {})", group.id, pid));
        }

        let args = self.build_native_av_args(group, video_config, audio_config);
        let sanitized = self.sanitize_ffmpeg_args(&args, group);
        log::info!(
            "Starting FFmpeg native A/V capture for group {}: {} {}",
            group.id,
            self.ffmpeg_path,
            sanitized.join(" ")
        );

        self.spawn_native_capture(group, args, event_sink)
    }
}
