// FFmpegHandler Service
// Manages FFmpeg processes for streaming with real-time stats

use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};
use crate::models::{OutputGroup, StreamStats};

/// Process info for tracking active streams
struct ProcessInfo {
    child: Child,
    start_time: Instant,
    group_id: String,
}

/// Manages FFmpeg streaming processes
pub struct FFmpegHandler {
    ffmpeg_path: String,
    processes: Arc<Mutex<HashMap<String, ProcessInfo>>>,
}

impl FFmpegHandler {
    /// Create FFmpegHandler with optional custom FFmpeg path from settings
    /// Falls back to auto-discovery if custom path is empty or invalid
    pub fn new_with_custom_path(app_data_dir: PathBuf, custom_path: Option<String>) -> Self {
        let ffmpeg_path = match custom_path {
            Some(ref path) if !path.is_empty() && std::path::Path::new(path).exists() => {
                log::info!("Using custom FFmpeg path from settings: {}", path);
                path.clone()
            }
            _ => {
                log::info!("Using auto-detected FFmpeg path");
                Self::find_ffmpeg_with_bundled(app_data_dir)
            }
        };

        Self {
            ffmpeg_path,
            processes: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Create a new FFmpegHandler (legacy, without bundled FFmpeg support)
    pub fn new() -> Self {
        Self {
            ffmpeg_path: Self::find_ffmpeg(),
            processes: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Normalize an RTMP URL for consistency
    fn normalize_rtmp_url(url: &str) -> String {
        let mut url = url.trim().to_string();

        // Remove trailing slashes
        while url.ends_with('/') {
            url.pop();
        }

        // Ensure rtmp:// or rtmps:// prefix if missing
        if !url.starts_with("rtmp://") && !url.starts_with("rtmps://") {
            // Check if it looks like it should be rtmps (common rtmps ports or hosts)
            if url.contains(":443") || url.contains("facebook.com") {
                url = format!("rtmps://{}", url);
            } else {
                url = format!("rtmp://{}", url);
            }
        }

        url
    }

    /// Find FFmpeg, checking bundled path first, then PATH/common locations
    fn find_ffmpeg_with_bundled(app_data_dir: PathBuf) -> String {
        // 1. Check for bundled/downloaded FFmpeg first
        let ffmpeg_dir = app_data_dir.join("ffmpeg");
        let binary_name = if cfg!(windows) { "ffmpeg.exe" } else { "ffmpeg" };
        let bundled_path = ffmpeg_dir.join(binary_name);

        if bundled_path.exists() {
            log::info!("Using bundled FFmpeg: {:?}", bundled_path);
            return bundled_path.to_string_lossy().to_string();
        }

        // 2. Fall back to regular discovery
        Self::find_ffmpeg()
    }

    /// Find FFmpeg in PATH or common locations
    fn find_ffmpeg() -> String {
        // Try to find ffmpeg in PATH first
        #[cfg(unix)]
        if let Ok(output) = Command::new("which").arg("ffmpeg").output() {
            if output.status.success() {
                if let Ok(path) = String::from_utf8(output.stdout) {
                    return path.trim().to_string();
                }
            }
        }

        #[cfg(windows)]
        if let Ok(output) = Command::new("where").arg("ffmpeg").output() {
            if output.status.success() {
                if let Ok(path) = String::from_utf8(output.stdout) {
                    // `where` can return multiple paths, take the first
                    if let Some(first_path) = path.lines().next() {
                        return first_path.trim().to_string();
                    }
                }
            }
        }

        // Fallback to common locations
        #[cfg(target_os = "macos")]
        {
            if std::path::Path::new("/opt/homebrew/bin/ffmpeg").exists() {
                return "/opt/homebrew/bin/ffmpeg".to_string();
            }
            if std::path::Path::new("/usr/local/bin/ffmpeg").exists() {
                return "/usr/local/bin/ffmpeg".to_string();
            }
        }

        #[cfg(windows)]
        {
            // Check common Windows FFmpeg locations
            let program_files = std::env::var("ProgramFiles").unwrap_or_default();
            let ffmpeg_path = std::path::Path::new(&program_files).join("ffmpeg\\bin\\ffmpeg.exe");
            if ffmpeg_path.exists() {
                return ffmpeg_path.to_string_lossy().to_string();
            }
        }

        // Default
        "ffmpeg".to_string()
    }

    /// Start streaming for an output group with stats monitoring
    pub fn start<R: tauri::Runtime>(
        &self,
        group: &OutputGroup,
        incoming_url: &str,
        app_handle: &AppHandle<R>,
    ) -> Result<u32, String> {
        let args = self.build_args(group, incoming_url);

        let mut child = Command::new(&self.ffmpeg_path)
            .args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("Failed to start FFmpeg: {}", e))?;

        let pid = child.id();
        let group_id = group.id.clone();

        // Take stderr for stats parsing
        let stderr = child.stderr.take()
            .ok_or_else(|| "Failed to capture FFmpeg stderr".to_string())?;

        // Store process info
        {
            let mut processes = self.processes.lock()
                .map_err(|e| format!("Lock poisoned: {}", e))?;
            processes.insert(group_id.clone(), ProcessInfo {
                child,
                start_time: Instant::now(),
                group_id: group_id.clone(),
            });
        }

        // Spawn background thread to read stderr and emit stats
        let app_handle_clone = app_handle.clone();
        let processes_clone = Arc::clone(&self.processes);
        let group_id_clone = group_id.clone();

        thread::spawn(move || {
            Self::stats_reader(
                stderr,
                group_id_clone,
                app_handle_clone,
                processes_clone,
            );
        });

        Ok(pid)
    }

    /// Background thread that reads FFmpeg stderr and emits stats events
    fn stats_reader<R: tauri::Runtime>(
        stderr: std::process::ChildStderr,
        group_id: String,
        app_handle: AppHandle<R>,
        processes: Arc<Mutex<HashMap<String, ProcessInfo>>>,
    ) {
        let reader = BufReader::new(stderr);
        let mut stats = StreamStats::new(group_id.clone());
        let mut last_emit = Instant::now();
        let emit_interval = Duration::from_millis(1000); // Emit every second
        let mut was_intentionally_stopped = false;

        for line_result in reader.lines() {
            // Check if process is still running (was it intentionally stopped?)
            {
                if let Ok(procs) = processes.lock() {
                    if !procs.contains_key(&group_id) {
                        // Process was removed by stop() - intentional stop
                        was_intentionally_stopped = true;
                        break;
                    }
                }
            }

            if let Ok(line) = line_result {
                // Parse stats from FFmpeg output
                if stats.parse_line(&line) {
                    // Emit stats at most every second
                    if last_emit.elapsed() >= emit_interval {
                        // Add uptime from process start
                        if let Ok(procs) = processes.lock() {
                            if let Some(info) = procs.get(&group_id) {
                                stats.time = info.start_time.elapsed().as_secs_f64();
                            }
                        }

                        // Emit event
                        let _ = app_handle.emit("stream_stats", stats.clone());
                        last_emit = Instant::now();
                    }
                }

                // Only log errors and warnings (not frame stats which are too verbose)
                if line.contains("[error]") || line.contains("[warning]") {
                    log::warn!("[FFmpeg:{}] {}", group_id, line);
                }
            }
        }

        // Process ended - check if it was intentional or a crash
        if was_intentionally_stopped {
            // Intentional stop via stop() - process already removed
            let _ = app_handle.emit("stream_ended", &group_id);
        } else {
            // Process ended unexpectedly (crash, connection loss, etc.)
            // Remove from HashMap and check exit status
            let exit_status = {
                if let Ok(mut procs) = processes.lock() {
                    if let Some(mut info) = procs.remove(&group_id) {
                        // Try to get exit status
                        match info.child.try_wait() {
                            Ok(Some(status)) => Some(status),
                            Ok(None) => info.child.wait().ok(),  // Process still running, wait for it
                            Err(_) => info.child.wait().ok(),    // Error checking, try to wait anyway
                        }
                    } else {
                        None
                    }
                } else {
                    None
                }
            };

            // Determine if this was a crash or normal exit
            let error_message = match exit_status {
                Some(status) if status.success() => {
                    // FFmpeg exited cleanly (exit code 0)
                    // This might happen if the input stream ended
                    None
                }
                Some(status) => {
                    // FFmpeg exited with error
                    let code = status.code().unwrap_or(-1);
                    Some(format!("FFmpeg exited with code {}", code))
                }
                None => {
                    // Couldn't get exit status
                    Some("FFmpeg process terminated unexpectedly".to_string())
                }
            };

            if let Some(error) = error_message {
                log::warn!("[FFmpeg:{}] Stream error: {}", group_id, error);
                // Emit stream_error event with group_id and error message
                let _ = app_handle.emit("stream_error", serde_json::json!({
                    "groupId": group_id,
                    "error": error
                }));
            } else {
                // Clean exit (input ended)
                let _ = app_handle.emit("stream_ended", &group_id);
            }
        }
    }

    /// Stop streaming for an output group
    pub fn stop(&self, group_id: &str) -> Result<(), String> {
        if let Some(mut info) = self.processes.lock()
            .map_err(|e| format!("Lock poisoned: {}", e))?
            .remove(group_id)
        {
            info.child.kill().map_err(|e| format!("Failed to kill FFmpeg: {}", e))?;
            info.child.wait().map_err(|e| format!("Failed to wait for FFmpeg: {}", e))?;
        }
        Ok(())
    }

    /// Stop all active streams
    pub fn stop_all(&self) -> Result<(), String> {
        let mut processes = self.processes.lock()
            .map_err(|e| format!("Lock poisoned: {}", e))?;
        for (_, mut info) in processes.drain() {
            let _ = info.child.kill();
            let _ = info.child.wait();
        }
        Ok(())
    }

    /// Get active stream count
    pub fn active_count(&self) -> usize {
        self.processes.lock()
            .map(|procs| procs.len())
            .unwrap_or(0)
    }

    /// Check if a group is streaming
    pub fn is_streaming(&self, group_id: &str) -> bool {
        self.processes.lock()
            .map(|procs| procs.contains_key(group_id))
            .unwrap_or(false)
    }

    /// Get list of active stream group IDs
    pub fn get_active_group_ids(&self) -> Vec<String> {
        self.processes.lock()
            .map(|procs| procs.values().map(|info| info.group_id.clone()).collect())
            .unwrap_or_default()
    }

    /// Build FFmpeg arguments for an output group
    fn build_args(&self, group: &OutputGroup, incoming_url: &str) -> Vec<String> {
        let mut args = vec![
            "-i".to_string(), incoming_url.to_string(),
            "-c:v".to_string(), group.video_encoder.clone(),
            "-s".to_string(), group.resolution.clone(),
            "-b:v".to_string(), format!("{}k", group.video_bitrate),
            "-r".to_string(), group.fps.to_string(),
            "-c:a".to_string(), group.audio_codec.clone(),
            "-b:a".to_string(), format!("{}k", group.audio_bitrate),
            // Progress output for stats parsing
            "-progress".to_string(), "pipe:2".to_string(),
            "-stats".to_string(),
        ];

        // Add encoder preset if specified
        if let Some(preset) = &group.preset {
            // Map user-friendly preset names to FFmpeg preset values
            let ffmpeg_preset = match preset.as_str() {
                "quality" => "slow",
                "balanced" => "medium",
                "performance" => "fast",
                "low_latency" => "ultrafast",
                _ => preset.as_str(), // Use as-is if already a valid FFmpeg preset
            };
            args.extend(["-preset".to_string(), ffmpeg_preset.to_string()]);
        }

        // Add rate control if specified
        if let Some(rate_control) = &group.rate_control {
            match rate_control.as_str() {
                "cbr" => {
                    // Constant bitrate: set max and min equal to target
                    args.extend([
                        "-maxrate".to_string(), format!("{}k", group.video_bitrate),
                        "-minrate".to_string(), format!("{}k", group.video_bitrate),
                        "-bufsize".to_string(), format!("{}k", group.video_bitrate * 2),
                    ]);
                }
                "vbr" => {
                    // Variable bitrate: use CRF quality mode with bitrate hint
                    // Allow 50% overshoot, buffer at 2x bitrate
                    args.extend([
                        "-maxrate".to_string(), format!("{}k", (group.video_bitrate as f32 * 1.5) as u32),
                        "-bufsize".to_string(), format!("{}k", group.video_bitrate * 2),
                    ]);
                }
                "cqp" => {
                    // Constant quality (QP mode) - only for hardware encoders
                    // Default to QP 23 which is roughly equivalent to CRF 23
                    args.extend(["-qp".to_string(), "23".to_string()]);
                }
                _ => {} // Unknown rate control, skip
            }
        }

        if group.generate_pts {
            args.extend(["-fflags".to_string(), "+genpts".to_string()]);
        }

        // Add output targets
        for target in &group.stream_targets {
            let normalized_url = Self::normalize_rtmp_url(&target.url);
            args.extend([
                "-f".to_string(), "flv".to_string(),
                format!("{}/{}", normalized_url, target.stream_key),
            ]);
        }

        args
    }
}

impl Default for FFmpegHandler {
    fn default() -> Self {
        Self::new()
    }
}
