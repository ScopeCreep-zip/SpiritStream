// Replay Buffer Service
// Maintains a circular buffer of the last N seconds for instant replay saving
//
// Architecture (similar to OBS):
// - Uses FFmpeg segment muxer to create rolling segment files
// - Maintains a list of recent segments covering the buffer duration
// - On save, concatenates segments into a single output file
// - Automatically cleans up old segments beyond buffer duration

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use serde::{Deserialize, Serialize};

#[cfg(windows)]
use std::os::windows::process::CommandExt;
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

/// Segment information for the circular buffer
#[derive(Debug, Clone)]
struct BufferSegment {
    path: PathBuf,
    start_time: Instant,
    duration_secs: f64,
}

/// Replay buffer configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplayBufferConfig {
    /// Duration of replay buffer in seconds (5-300)
    pub duration_secs: u32,
    /// Output directory for saved replays
    pub output_path: String,
    /// Segment duration in seconds (default: 2)
    pub segment_duration: u32,
}

impl Default for ReplayBufferConfig {
    fn default() -> Self {
        Self {
            duration_secs: 30,
            output_path: String::new(),
            segment_duration: 2,
        }
    }
}

/// Replay buffer state for API responses
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplayBufferState {
    pub is_active: bool,
    pub duration_secs: u32,
    pub buffered_secs: f64,
    pub output_path: String,
}

/// Information about a saved replay
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedReplayInfo {
    pub file_path: String,
    pub duration_secs: f64,
    pub size_bytes: u64,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Internal state for the replay buffer
struct ReplayBufferInternal {
    config: ReplayBufferConfig,
    ffmpeg_process: Option<Child>,
    segments: VecDeque<BufferSegment>,
    start_time: Option<Instant>,
    segment_counter: u64,
    temp_dir: PathBuf,
}

/// Service for managing the replay buffer
pub struct ReplayBufferService {
    ffmpeg_path: String,
    app_data_dir: PathBuf,
    state: Arc<Mutex<ReplayBufferInternal>>,
}

impl ReplayBufferService {
    /// Create a new replay buffer service
    pub fn new(ffmpeg_path: String, app_data_dir: PathBuf) -> Result<Self, String> {
        let temp_dir = app_data_dir.join("replay_buffer_temp");

        // Ensure temp directory exists
        if !temp_dir.exists() {
            std::fs::create_dir_all(&temp_dir)
                .map_err(|e| format!("Failed to create replay buffer temp dir: {}", e))?;
        }

        let internal = ReplayBufferInternal {
            config: ReplayBufferConfig::default(),
            ffmpeg_process: None,
            segments: VecDeque::new(),
            start_time: None,
            segment_counter: 0,
            temp_dir,
        };

        Ok(Self {
            ffmpeg_path,
            app_data_dir,
            state: Arc::new(Mutex::new(internal)),
        })
    }

    /// Start the replay buffer from a relay URL (composited output)
    pub fn start(&self, relay_url: &str, config: ReplayBufferConfig) -> Result<(), String> {
        let mut state = self.state.lock()
            .map_err(|e| format!("Lock poisoned: {}", e))?;

        if state.ffmpeg_process.is_some() {
            return Err("Replay buffer already active".to_string());
        }

        // Clean up any old segments
        self.cleanup_temp_dir(&state.temp_dir)?;

        // Validate config
        let duration_secs = config.duration_secs.clamp(5, 300);
        let segment_duration = config.segment_duration.clamp(1, 10);

        // Set output path, defaulting to app data replays dir
        let output_path = if config.output_path.is_empty() {
            self.app_data_dir.join("replays").to_string_lossy().to_string()
        } else {
            // Expand ~ to home directory
            if config.output_path.starts_with("~/") {
                dirs_next::home_dir()
                    .map(|h| h.join(&config.output_path[2..]).to_string_lossy().to_string())
                    .unwrap_or_else(|| config.output_path.clone())
            } else {
                config.output_path.clone()
            }
        };

        // Ensure output directory exists
        let output_dir = PathBuf::from(&output_path);
        if !output_dir.exists() {
            std::fs::create_dir_all(&output_dir)
                .map_err(|e| format!("Failed to create replay output dir: {}", e))?;
        }

        state.config = ReplayBufferConfig {
            duration_secs,
            output_path,
            segment_duration,
        };

        // Build FFmpeg command for segment output
        // Using mpegts segments for compatibility and fast seeking
        let segment_pattern = state.temp_dir.join("segment_%05d.ts");

        let args = vec![
            "-i".to_string(), relay_url.to_string(),
            "-c:v".to_string(), "copy".to_string(),
            "-c:a".to_string(), "copy".to_string(),
            "-f".to_string(), "segment".to_string(),
            "-segment_time".to_string(), segment_duration.to_string(),
            "-segment_format".to_string(), "mpegts".to_string(),
            "-reset_timestamps".to_string(), "1".to_string(),
            "-y".to_string(),
            segment_pattern.to_string_lossy().to_string(),
        ];

        log::info!("Starting replay buffer: {} {}", self.ffmpeg_path, args.join(" "));

        let mut cmd = Command::new(&self.ffmpeg_path);
        cmd.args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());

        #[cfg(windows)]
        cmd.creation_flags(CREATE_NO_WINDOW);

        let child = cmd.spawn()
            .map_err(|e| format!("Failed to start replay buffer FFmpeg: {}", e))?;

        state.ffmpeg_process = Some(child);
        state.start_time = Some(Instant::now());
        state.segment_counter = 0;
        state.segments.clear();

        // Start segment watcher thread
        let state_clone = Arc::clone(&self.state);
        let temp_dir = state.temp_dir.clone();
        let segment_dur = segment_duration;
        let buffer_dur = duration_secs;

        std::thread::spawn(move || {
            Self::segment_watcher_loop(state_clone, temp_dir, segment_dur, buffer_dur);
        });

        log::info!("Replay buffer started with {}s buffer, {}s segments",
            duration_secs, segment_duration);

        Ok(())
    }

    /// Stop the replay buffer
    pub fn stop(&self) -> Result<(), String> {
        let mut state = self.state.lock()
            .map_err(|e| format!("Lock poisoned: {}", e))?;

        if let Some(mut process) = state.ffmpeg_process.take() {
            let _ = process.kill();
            let _ = process.wait();
        }

        // Clean up temp segments
        self.cleanup_temp_dir(&state.temp_dir)?;

        state.segments.clear();
        state.start_time = None;

        log::info!("Replay buffer stopped");
        Ok(())
    }

    /// Save the current buffer contents to a file
    pub fn save_replay(&self) -> Result<SavedReplayInfo, String> {
        let state = self.state.lock()
            .map_err(|e| format!("Lock poisoned: {}", e))?;

        if state.ffmpeg_process.is_none() {
            return Err("Replay buffer not active".to_string());
        }

        if state.segments.is_empty() {
            return Err("No segments buffered yet".to_string());
        }

        // Generate output filename
        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
        let output_file = PathBuf::from(&state.config.output_path)
            .join(format!("replay_{}.mp4", timestamp));

        // Create concat list file
        let concat_list_path = state.temp_dir.join("concat_list.txt");
        let concat_content: String = state.segments.iter()
            .map(|seg| format!("file '{}'", seg.path.to_string_lossy()))
            .collect::<Vec<_>>()
            .join("\n");

        std::fs::write(&concat_list_path, &concat_content)
            .map_err(|e| format!("Failed to write concat list: {}", e))?;

        // Calculate total duration
        let total_duration: f64 = state.segments.iter()
            .map(|s| s.duration_secs)
            .sum();

        // Use FFmpeg to concatenate segments
        let concat_args = vec![
            "-f".to_string(), "concat".to_string(),
            "-safe".to_string(), "0".to_string(),
            "-i".to_string(), concat_list_path.to_string_lossy().to_string(),
            "-c".to_string(), "copy".to_string(),
            "-movflags".to_string(), "+faststart".to_string(),
            "-y".to_string(),
            output_file.to_string_lossy().to_string(),
        ];

        log::info!("Saving replay: {} {}", self.ffmpeg_path, concat_args.join(" "));

        let mut cmd = Command::new(&self.ffmpeg_path);
        cmd.args(&concat_args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());

        #[cfg(windows)]
        cmd.creation_flags(CREATE_NO_WINDOW);

        let output = cmd.output()
            .map_err(|e| format!("Failed to run FFmpeg concat: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("FFmpeg concat failed: {}", stderr));
        }

        // Clean up concat list
        let _ = std::fs::remove_file(&concat_list_path);

        // Get file size
        let metadata = std::fs::metadata(&output_file)
            .map_err(|e| format!("Failed to get replay file metadata: {}", e))?;

        let info = SavedReplayInfo {
            file_path: output_file.to_string_lossy().to_string(),
            duration_secs: total_duration,
            size_bytes: metadata.len(),
            created_at: chrono::Utc::now(),
        };

        log::info!("Replay saved: {} ({:.1}s, {} bytes)",
            info.file_path, info.duration_secs, info.size_bytes);

        Ok(info)
    }

    /// Get the current replay buffer state
    pub fn get_state(&self) -> Result<ReplayBufferState, String> {
        let state = self.state.lock()
            .map_err(|e| format!("Lock poisoned: {}", e))?;

        let buffered_secs = if state.ffmpeg_process.is_some() {
            state.segments.iter().map(|s| s.duration_secs).sum()
        } else {
            0.0
        };

        Ok(ReplayBufferState {
            is_active: state.ffmpeg_process.is_some(),
            duration_secs: state.config.duration_secs,
            buffered_secs,
            output_path: state.config.output_path.clone(),
        })
    }

    /// Check if the replay buffer is active
    pub fn is_active(&self) -> bool {
        self.state.lock()
            .map(|s| s.ffmpeg_process.is_some())
            .unwrap_or(false)
    }

    /// Update the buffer duration (requires restart to take effect)
    pub fn set_duration(&self, duration_secs: u32) -> Result<(), String> {
        let mut state = self.state.lock()
            .map_err(|e| format!("Lock poisoned: {}", e))?;

        state.config.duration_secs = duration_secs.clamp(5, 300);
        Ok(())
    }

    /// Update the output path
    pub fn set_output_path(&self, path: String) -> Result<(), String> {
        let mut state = self.state.lock()
            .map_err(|e| format!("Lock poisoned: {}", e))?;

        state.config.output_path = path;
        Ok(())
    }

    /// Background thread that watches for new segments and manages the buffer
    fn segment_watcher_loop(
        state: Arc<Mutex<ReplayBufferInternal>>,
        temp_dir: PathBuf,
        segment_duration: u32,
        buffer_duration: u32,
    ) {
        let max_segments = (buffer_duration / segment_duration) as usize + 2; // +2 for safety margin
        let check_interval = Duration::from_millis(500);
        let mut last_segment_count: u64 = 0;

        loop {
            std::thread::sleep(check_interval);

            let mut guard = match state.lock() {
                Ok(g) => g,
                Err(_) => break,
            };

            // Check if we should stop
            if guard.ffmpeg_process.is_none() {
                break;
            }

            // Check for new segments
            let entries: Vec<_> = match std::fs::read_dir(&temp_dir) {
                Ok(e) => e.filter_map(|e| e.ok()).collect(),
                Err(_) => continue,
            };

            let mut segment_files: Vec<_> = entries.iter()
                .filter(|e| {
                    e.path().extension()
                        .map(|ext| ext == "ts")
                        .unwrap_or(false)
                })
                .collect();

            // Sort by name (which includes the counter)
            segment_files.sort_by_key(|e| e.path());

            let current_count = segment_files.len() as u64;

            // Add new segments to the buffer
            if current_count > last_segment_count {
                for entry in segment_files.iter().skip(last_segment_count as usize) {
                    let path = entry.path();
                    guard.segments.push_back(BufferSegment {
                        path: path.clone(),
                        start_time: Instant::now(),
                        duration_secs: segment_duration as f64,
                    });
                    guard.segment_counter += 1;
                    log::debug!("New segment added: {:?}", path);
                }
                last_segment_count = current_count;
            }

            // Remove old segments beyond buffer duration
            while guard.segments.len() > max_segments {
                if let Some(old_segment) = guard.segments.pop_front() {
                    // Delete the old segment file
                    let _ = std::fs::remove_file(&old_segment.path);
                    log::debug!("Old segment removed: {:?}", old_segment.path);
                }
            }
        }

        log::debug!("Segment watcher loop ended");
    }

    /// Clean up the temp directory
    fn cleanup_temp_dir(&self, temp_dir: &Path) -> Result<(), String> {
        if temp_dir.exists() {
            for entry in std::fs::read_dir(temp_dir)
                .map_err(|e| format!("Failed to read temp dir: {}", e))?
            {
                if let Ok(entry) = entry {
                    let _ = std::fs::remove_file(entry.path());
                }
            }
        }
        Ok(())
    }
}

impl Drop for ReplayBufferService {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}
