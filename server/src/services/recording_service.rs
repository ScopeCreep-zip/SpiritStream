// Recording Service
// Manages local video recordings with optional encryption

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::Instant;
use serde::{Deserialize, Serialize};

use crate::services::Encryption;
use crate::models::OutputGroup;

// Windows: Hide console windows
#[cfg(windows)]
use std::os::windows::process::CommandExt;
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

/// Recording format options
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RecordingFormat {
    Mp4,
    Mkv,
    Mov,
    Webm,
    Ts,
    Flv,
}

impl RecordingFormat {
    fn extension(&self) -> &'static str {
        match self {
            RecordingFormat::Mp4 => "mp4",
            RecordingFormat::Mkv => "mkv",
            RecordingFormat::Mov => "mov",
            RecordingFormat::Webm => "webm",
            RecordingFormat::Ts => "ts",
            RecordingFormat::Flv => "flv",
        }
    }

    fn ffmpeg_format(&self) -> &'static str {
        match self {
            RecordingFormat::Mp4 => "mp4",
            RecordingFormat::Mkv => "matroska",
            RecordingFormat::Mov => "mov",
            RecordingFormat::Webm => "webm",
            RecordingFormat::Ts => "mpegts",
            RecordingFormat::Flv => "flv",
        }
    }
}

impl Default for RecordingFormat {
    fn default() -> Self {
        RecordingFormat::Mp4
    }
}

/// Recording configuration
#[derive(Debug, Clone)]
pub struct RecordingConfig {
    pub name: String,
    pub format: RecordingFormat,
    pub encrypt: bool,
    pub password: Option<String>,
}

/// Information about a recording
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordingInfo {
    pub id: String,
    pub name: String,
    pub file_path: String,
    pub format: RecordingFormat,
    pub encrypted: bool,
    pub size_bytes: u64,
    pub duration_secs: Option<f64>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub completed: bool,
}

/// Active recording handle
struct ActiveRecording {
    id: String,
    process: Child,
    temp_path: PathBuf,
    final_path: PathBuf,
    config: RecordingConfig,
    start_time: Instant,
}

/// Service for managing local recordings
pub struct RecordingService {
    ffmpeg_path: String,
    recordings_dir: PathBuf,
    app_data_dir: PathBuf,
    active_recordings: Mutex<HashMap<String, ActiveRecording>>,
}

impl RecordingService {
    /// Create a new recording service
    pub fn new(ffmpeg_path: String, app_data_dir: PathBuf) -> Result<Self, String> {
        let recordings_dir = app_data_dir.join("recordings");

        // Ensure recordings directory exists with secure permissions
        Self::ensure_secure_directory(&recordings_dir)?;

        Ok(Self {
            ffmpeg_path,
            recordings_dir,
            app_data_dir,
            active_recordings: Mutex::new(HashMap::new()),
        })
    }

    /// Ensure a directory exists with secure permissions (owner-only)
    fn ensure_secure_directory(dir: &Path) -> Result<(), String> {
        if !dir.exists() {
            std::fs::create_dir_all(dir)
                .map_err(|e| format!("Failed to create directory: {}", e))?;
        }

        // Set restrictive permissions
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(0o700);
            std::fs::set_permissions(dir, perms)
                .map_err(|e| format!("Failed to set directory permissions: {}", e))?;
        }

        // On Windows, ACLs are set by default to owner-only for user data dirs
        // Additional ACL configuration would require winapi calls

        Ok(())
    }

    /// Generate a unique recording ID
    fn generate_id() -> String {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let random: u64 = rng.gen();
        format!("rec_{:016x}", random)
    }

    /// Sanitize filename for filesystem use
    fn sanitize_filename(name: &str) -> String {
        name.chars()
            .map(|c| match c {
                '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
                c if c.is_control() => '_',
                c => c,
            })
            .collect()
    }

    /// Start recording from a native video input (via stdin pipe)
    pub fn start_recording_native(
        &self,
        config: RecordingConfig,
        width: u32,
        height: u32,
        fps: u32,
        pixel_format: &str,
    ) -> Result<(String, std::process::ChildStdin), String> {
        let id = Self::generate_id();
        let sanitized_name = Self::sanitize_filename(&config.name);
        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");

        // Determine file paths
        let filename = if config.encrypt {
            format!("{}_{}.{}.tmp", sanitized_name, timestamp, config.format.extension())
        } else {
            format!("{}_{}.{}", sanitized_name, timestamp, config.format.extension())
        };

        let temp_path = self.recordings_dir.join(&filename);
        let final_path = if config.encrypt {
            self.recordings_dir.join(format!(
                "{}_{}.{}.enc",
                sanitized_name,
                timestamp,
                config.format.extension()
            ))
        } else {
            temp_path.clone()
        };

        // Build FFmpeg command
        let args = vec![
            "-f".to_string(), "rawvideo".to_string(),
            "-pix_fmt".to_string(), pixel_format.to_string(),
            "-s".to_string(), format!("{}x{}", width, height),
            "-r".to_string(), fps.to_string(),
            "-i".to_string(), "pipe:0".to_string(),
            "-c:v".to_string(), "libx264".to_string(),
            "-preset".to_string(), "fast".to_string(),
            "-crf".to_string(), "23".to_string(),
            "-f".to_string(), config.format.ffmpeg_format().to_string(),
            "-y".to_string(),
            temp_path.to_string_lossy().to_string(),
        ];

        log::info!("Starting recording {}: {} {}", id, self.ffmpeg_path, args.join(" "));

        let mut cmd = Command::new(&self.ffmpeg_path);
        cmd.args(&args)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());

        #[cfg(windows)]
        cmd.creation_flags(CREATE_NO_WINDOW);

        let mut child = cmd.spawn()
            .map_err(|e| format!("Failed to start FFmpeg recording: {}", e))?;

        let stdin = child.stdin.take()
            .ok_or_else(|| "Failed to capture FFmpeg stdin".to_string())?;

        // Store active recording
        {
            let mut recordings = self.active_recordings.lock()
                .map_err(|e| format!("Lock poisoned: {}", e))?;

            recordings.insert(id.clone(), ActiveRecording {
                id: id.clone(),
                process: child,
                temp_path,
                final_path,
                config,
                start_time: Instant::now(),
            });
        }

        log::info!("Recording {} started", id);
        Ok((id, stdin))
    }

    /// Start recording from an output group (RTMP relay)
    pub fn start_recording_from_relay(
        &self,
        config: RecordingConfig,
        _group: &OutputGroup,
        relay_url: &str,
    ) -> Result<String, String> {
        let id = Self::generate_id();
        let sanitized_name = Self::sanitize_filename(&config.name);
        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");

        // Determine file paths
        let filename = if config.encrypt {
            format!("{}_{}.{}.tmp", sanitized_name, timestamp, config.format.extension())
        } else {
            format!("{}_{}.{}", sanitized_name, timestamp, config.format.extension())
        };

        let temp_path = self.recordings_dir.join(&filename);
        let final_path = if config.encrypt {
            self.recordings_dir.join(format!(
                "{}_{}.{}.enc",
                sanitized_name,
                timestamp,
                config.format.extension()
            ))
        } else {
            temp_path.clone()
        };

        // Build FFmpeg command
        let args = vec![
            "-i".to_string(), relay_url.to_string(),
            "-c:v".to_string(), "copy".to_string(),
            "-c:a".to_string(), "copy".to_string(),
            "-f".to_string(), config.format.ffmpeg_format().to_string(),
            "-y".to_string(),
            temp_path.to_string_lossy().to_string(),
        ];

        log::info!("Starting recording {} from relay: {} {}",
            id, self.ffmpeg_path, args.join(" "));

        let mut cmd = Command::new(&self.ffmpeg_path);
        cmd.args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());

        #[cfg(windows)]
        cmd.creation_flags(CREATE_NO_WINDOW);

        let child = cmd.spawn()
            .map_err(|e| format!("Failed to start FFmpeg recording: {}", e))?;

        // Store active recording
        {
            let mut recordings = self.active_recordings.lock()
                .map_err(|e| format!("Lock poisoned: {}", e))?;

            recordings.insert(id.clone(), ActiveRecording {
                id: id.clone(),
                process: child,
                temp_path,
                final_path,
                config,
                start_time: Instant::now(),
            });
        }

        log::info!("Recording {} started from relay", id);
        Ok(id)
    }

    /// Stop a recording and finalize the file
    pub fn stop_recording(&self, id: &str) -> Result<RecordingInfo, String> {
        let recording = {
            let mut recordings = self.active_recordings.lock()
                .map_err(|e| format!("Lock poisoned: {}", e))?;

            recordings.remove(id)
                .ok_or_else(|| format!("Recording {} not found", id))?
        };

        // Stop FFmpeg process
        let mut process = recording.process;
        let _ = process.kill();
        let _ = process.wait();

        let duration = recording.start_time.elapsed().as_secs_f64();

        // Encrypt if configured
        if recording.config.encrypt {
            self.encrypt_recording(&recording.temp_path, &recording.final_path, &recording.config)?;
            // Delete unencrypted temp file
            let _ = std::fs::remove_file(&recording.temp_path);
        }

        // Get file size
        let metadata = std::fs::metadata(&recording.final_path)
            .map_err(|e| format!("Failed to get recording metadata: {}", e))?;

        let info = RecordingInfo {
            id: recording.id,
            name: recording.config.name,
            file_path: recording.final_path.to_string_lossy().to_string(),
            format: recording.config.format,
            encrypted: recording.config.encrypt,
            size_bytes: metadata.len(),
            duration_secs: Some(duration),
            created_at: chrono::Utc::now(),
            completed: true,
        };

        log::info!("Recording {} stopped: {} bytes, {:.1}s",
            info.id, info.size_bytes, duration);

        Ok(info)
    }

    /// Stop all active recordings
    pub fn stop_all(&self) -> Vec<Result<RecordingInfo, String>> {
        let ids: Vec<String> = {
            let recordings = self.active_recordings.lock().ok();
            recordings.map(|r| r.keys().cloned().collect()).unwrap_or_default()
        };

        ids.iter().map(|id| self.stop_recording(id)).collect()
    }

    /// Encrypt a recording file
    fn encrypt_recording(
        &self,
        source: &Path,
        dest: &Path,
        config: &RecordingConfig,
    ) -> Result<(), String> {
        let password = config.password.as_ref()
            .ok_or_else(|| "Password required for encrypted recording".to_string())?;

        // Read source file
        let data = std::fs::read(source)
            .map_err(|e| format!("Failed to read recording for encryption: {}", e))?;

        // Encrypt
        let encrypted = Encryption::encrypt(&data, password)?;

        // Write encrypted file
        std::fs::write(dest, encrypted)
            .map_err(|e| format!("Failed to write encrypted recording: {}", e))?;

        log::info!("Recording encrypted: {} -> {}", source.display(), dest.display());
        Ok(())
    }

    /// List all recordings in the recordings directory
    pub fn list_recordings(&self) -> Result<Vec<RecordingInfo>, String> {
        let entries = std::fs::read_dir(&self.recordings_dir)
            .map_err(|e| format!("Failed to read recordings directory: {}", e))?;

        let mut recordings = Vec::new();

        for entry in entries.flatten() {
            let path = entry.path();

            if !path.is_file() {
                continue;
            }

            let filename = path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");

            // Skip temp files
            if filename.ends_with(".tmp") {
                continue;
            }

            // Determine format and encryption status
            let (format, encrypted) = if filename.ends_with(".enc") {
                // Remove .enc and check format
                let inner = filename.trim_end_matches(".enc");
                let format = if inner.ends_with(".mp4") {
                    RecordingFormat::Mp4
                } else if inner.ends_with(".mkv") {
                    RecordingFormat::Mkv
                } else if inner.ends_with(".mov") {
                    RecordingFormat::Mov
                } else if inner.ends_with(".webm") {
                    RecordingFormat::Webm
                } else {
                    continue;
                };
                (format, true)
            } else if filename.ends_with(".mp4") {
                (RecordingFormat::Mp4, false)
            } else if filename.ends_with(".mkv") {
                (RecordingFormat::Mkv, false)
            } else if filename.ends_with(".mov") {
                (RecordingFormat::Mov, false)
            } else if filename.ends_with(".webm") {
                (RecordingFormat::Webm, false)
            } else {
                continue;
            };

            let metadata = std::fs::metadata(&path).ok();

            let info = RecordingInfo {
                id: filename.to_string(),
                name: filename.to_string(),
                file_path: path.to_string_lossy().to_string(),
                format,
                encrypted,
                size_bytes: metadata.as_ref().map(|m| m.len()).unwrap_or(0),
                duration_secs: None,
                created_at: metadata
                    .and_then(|m| m.created().ok())
                    .map(|t| chrono::DateTime::<chrono::Utc>::from(t))
                    .unwrap_or_else(chrono::Utc::now),
                completed: true,
            };

            recordings.push(info);
        }

        // Sort by creation time (newest first)
        recordings.sort_by(|a, b| b.created_at.cmp(&a.created_at));

        Ok(recordings)
    }

    /// Export (decrypt) a recording to a destination path
    pub fn export_recording(
        &self,
        recording_path: &str,
        password: Option<&str>,
        dest_path: &Path,
    ) -> Result<(), String> {
        let source = PathBuf::from(recording_path);

        if !source.exists() {
            return Err("Recording file not found".to_string());
        }

        // Check if encrypted
        let filename = source.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");

        if filename.ends_with(".enc") {
            let password = password
                .ok_or_else(|| "Password required for encrypted recording".to_string())?;

            // Read and decrypt
            let encrypted = std::fs::read(&source)
                .map_err(|e| format!("Failed to read recording: {}", e))?;

            let decrypted = Encryption::decrypt(&encrypted, password)?;

            // Write decrypted file
            std::fs::write(dest_path, decrypted)
                .map_err(|e| format!("Failed to write exported recording: {}", e))?;
        } else {
            // Just copy
            std::fs::copy(&source, dest_path)
                .map_err(|e| format!("Failed to copy recording: {}", e))?;
        }

        log::info!("Recording exported: {} -> {}", source.display(), dest_path.display());
        Ok(())
    }

    /// Delete a recording
    pub fn delete_recording(&self, recording_path: &str) -> Result<(), String> {
        let path = PathBuf::from(recording_path);

        if !path.exists() {
            return Err("Recording file not found".to_string());
        }

        // Security check: ensure path is within recordings directory
        let canonical_path = path.canonicalize()
            .map_err(|e| format!("Invalid path: {}", e))?;
        let canonical_dir = self.recordings_dir.canonicalize()
            .map_err(|e| format!("Invalid recordings directory: {}", e))?;

        if !canonical_path.starts_with(&canonical_dir) {
            return Err("Access denied: path outside recordings directory".to_string());
        }

        std::fs::remove_file(&path)
            .map_err(|e| format!("Failed to delete recording: {}", e))?;

        log::info!("Recording deleted: {}", path.display());
        Ok(())
    }

    /// Get active recording count
    pub fn active_count(&self) -> usize {
        self.active_recordings.lock()
            .map(|r| r.len())
            .unwrap_or(0)
    }

    /// Get active recording IDs
    pub fn active_ids(&self) -> Vec<String> {
        self.active_recordings.lock()
            .map(|r| r.keys().cloned().collect())
            .unwrap_or_default()
    }

    /// Check if a recording is active
    pub fn is_recording(&self, id: &str) -> bool {
        self.active_recordings.lock()
            .map(|r| r.contains_key(id))
            .unwrap_or(false)
    }

    /// Get the app data directory
    pub fn app_data_dir(&self) -> &Path {
        &self.app_data_dir
    }

    /// Get the recordings directory
    pub fn recordings_dir(&self) -> &Path {
        &self.recordings_dir
    }
}

impl Drop for RecordingService {
    fn drop(&mut self) {
        // Stop all active recordings on drop
        let _ = self.stop_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_filename() {
        assert_eq!(RecordingService::sanitize_filename("test"), "test");
        assert_eq!(RecordingService::sanitize_filename("test/file"), "test_file");
        assert_eq!(RecordingService::sanitize_filename("test:file"), "test_file");
        assert_eq!(RecordingService::sanitize_filename("test*file?"), "test_file_");
    }

    #[test]
    fn test_recording_format() {
        assert_eq!(RecordingFormat::Mp4.extension(), "mp4");
        assert_eq!(RecordingFormat::Mkv.extension(), "mkv");
        assert_eq!(RecordingFormat::Mp4.ffmpeg_format(), "mp4");
    }
}
