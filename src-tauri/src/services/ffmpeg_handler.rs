// FFmpegHandler Service
// Manages FFmpeg processes for streaming

use std::collections::HashMap;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use crate::models::OutputGroup;

/// Manages FFmpeg streaming processes
pub struct FFmpegHandler {
    ffmpeg_path: String,
    processes: Mutex<HashMap<String, Child>>,
}

impl FFmpegHandler {
    /// Create a new FFmpegHandler
    pub fn new() -> Self {
        Self {
            ffmpeg_path: Self::find_ffmpeg(),
            processes: Mutex::new(HashMap::new()),
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

    /// Start streaming for an output group
    pub fn start(&self, group: &OutputGroup, incoming_url: &str) -> Result<u32, String> {
        let args = self.build_args(group, incoming_url);

        let child = Command::new(&self.ffmpeg_path)
            .args(&args)
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("Failed to start FFmpeg: {}", e))?;

        let pid = child.id();
        self.processes.lock().unwrap().insert(group.id.clone(), child);

        Ok(pid)
    }

    /// Stop streaming for an output group
    pub fn stop(&self, group_id: &str) -> Result<(), String> {
        if let Some(mut child) = self.processes.lock().unwrap().remove(group_id) {
            child.kill().map_err(|e| format!("Failed to stop FFmpeg: {}", e))?;
        }
        Ok(())
    }

    /// Stop all active streams
    pub fn stop_all(&self) -> Result<(), String> {
        let mut processes = self.processes.lock().unwrap();
        for (_, mut child) in processes.drain() {
            child.kill().ok();
        }
        Ok(())
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
