// FFmpegHandler Service
// Manages FFmpeg processes for streaming with real-time stats

use std::collections::HashMap;
use std::io::{BufRead, BufReader};
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
    /// Create a new FFmpegHandler
    pub fn new() -> Self {
        Self {
            ffmpeg_path: Self::find_ffmpeg(),
            processes: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Find FFmpeg in PATH or common locations
    fn find_ffmpeg() -> String {
        // Try to find ffmpeg in PATH first
        if let Ok(output) = Command::new("which").arg("ffmpeg").output() {
            if output.status.success() {
                if let Ok(path) = String::from_utf8(output.stdout) {
                    return path.trim().to_string();
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
            let mut processes = self.processes.lock().unwrap();
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

        for line_result in reader.lines() {
            // Check if process is still running
            {
                let procs = processes.lock().unwrap();
                if !procs.contains_key(&group_id) {
                    break;
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

                // Also emit as log entry for debugging
                if line.starts_with("frame=") || line.contains("[error]") {
                    log::debug!("[FFmpeg:{}] {}", group_id, line);
                }
            }
        }

        // Process ended, emit final status
        let _ = app_handle.emit("stream_ended", &group_id);
    }

    /// Start streaming without stats (for use without AppHandle)
    pub fn start_simple(&self, group: &OutputGroup, incoming_url: &str) -> Result<u32, String> {
        let args = self.build_args(group, incoming_url);

        let child = Command::new(&self.ffmpeg_path)
            .args(&args)
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("Failed to start FFmpeg: {}", e))?;

        let pid = child.id();
        self.processes.lock().unwrap().insert(group.id.clone(), ProcessInfo {
            child,
            start_time: Instant::now(),
            group_id: group.id.clone(),
        });

        Ok(pid)
    }

    /// Stop streaming for an output group
    pub fn stop(&self, group_id: &str) -> Result<(), String> {
        if let Some(mut info) = self.processes.lock().unwrap().remove(group_id) {
            info.child.kill().map_err(|e| format!("Failed to stop FFmpeg: {}", e))?;
        }
        Ok(())
    }

    /// Stop all active streams
    pub fn stop_all(&self) -> Result<(), String> {
        let mut processes = self.processes.lock().unwrap();
        for (_, mut info) in processes.drain() {
            info.child.kill().ok();
        }
        Ok(())
    }

    /// Get active stream count
    pub fn active_count(&self) -> usize {
        self.processes.lock().unwrap().len()
    }

    /// Check if a group is streaming
    pub fn is_streaming(&self, group_id: &str) -> bool {
        self.processes.lock().unwrap().contains_key(group_id)
    }

    /// Get list of active stream group IDs
    pub fn get_active_group_ids(&self) -> Vec<String> {
        self.processes.lock().unwrap()
            .values()
            .map(|info| info.group_id.clone())
            .collect()
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

        if group.generate_pts {
            args.extend(["-fflags".to_string(), "+genpts".to_string()]);
        }

        // Add output targets
        for target in &group.stream_targets {
            args.extend([
                "-f".to_string(), "flv".to_string(),
                format!("{}/{}", target.url, target.stream_key),
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
