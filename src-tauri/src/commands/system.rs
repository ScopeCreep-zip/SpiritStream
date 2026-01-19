// System Commands
// Handles system-level operations like encoder detection

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tauri::{AppHandle, Manager};
use crate::models::Encoders;

// Windows: Hide console windows for spawned processes
#[cfg(windows)]
use std::os::windows::process::CommandExt;
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

/// Find FFmpeg path
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
    {
        let mut cmd = Command::new("where");
        cmd.arg("ffmpeg");
        cmd.creation_flags(CREATE_NO_WINDOW);
        if let Ok(output) = cmd.output() {
            if output.status.success() {
                if let Ok(path) = String::from_utf8(output.stdout) {
                    // `where` can return multiple paths, take the first
                    if let Some(first_path) = path.lines().next() {
                        return first_path.trim().to_string();
                    }
                }
            }
        }
    }

    // Fallback to common locations on macOS
    #[cfg(target_os = "macos")]
    {
        if std::path::Path::new("/opt/homebrew/bin/ffmpeg").exists() {
            return "/opt/homebrew/bin/ffmpeg".to_string();
        }
        if std::path::Path::new("/usr/local/bin/ffmpeg").exists() {
            return "/usr/local/bin/ffmpeg".to_string();
        }
    }

    // Fallback to common locations on Linux
    #[cfg(target_os = "linux")]
    {
        if std::path::Path::new("/usr/bin/ffmpeg").exists() {
            return "/usr/bin/ffmpeg".to_string();
        }
        if std::path::Path::new("/usr/local/bin/ffmpeg").exists() {
            return "/usr/local/bin/ffmpeg".to_string();
        }
        // Snap package location
        if std::path::Path::new("/snap/bin/ffmpeg").exists() {
            return "/snap/bin/ffmpeg".to_string();
        }
        // Flatpak location
        if std::path::Path::new("/var/lib/flatpak/exports/bin/ffmpeg").exists() {
            return "/var/lib/flatpak/exports/bin/ffmpeg".to_string();
        }
    }

    // Fallback to common locations on Windows
    #[cfg(windows)]
    {
        let program_files = std::env::var("ProgramFiles").unwrap_or_default();
        let ffmpeg_path = std::path::Path::new(&program_files).join("ffmpeg\\bin\\ffmpeg.exe");
        if ffmpeg_path.exists() {
            return ffmpeg_path.to_string_lossy().to_string();
        }
        // Also check common download location
        let local_app_data = std::env::var("LOCALAPPDATA").unwrap_or_default();
        let ffmpeg_local = std::path::Path::new(&local_app_data).join("ffmpeg\\bin\\ffmpeg.exe");
        if ffmpeg_local.exists() {
            return ffmpeg_local.to_string_lossy().to_string();
        }
    }

    // Default - rely on PATH
    #[cfg(windows)]
    { "ffmpeg.exe".to_string() }

    #[cfg(not(windows))]
    { "ffmpeg".to_string() }
}

/// Get available video and audio encoders by querying FFmpeg and hardware
#[tauri::command]
pub fn get_encoders() -> Result<Encoders, String> {
    let ffmpeg_path = find_ffmpeg();
    let mut cmd = Command::new(&ffmpeg_path);
    cmd.args(["-encoders", "-hide_banner"]);
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    let output = cmd.output()
        .map_err(|e| format!("Failed to run FFmpeg: {e}"))?;
    if !output.status.success() {
        return Ok(Encoders {
            video: vec!["libx264".to_string()],
            audio: vec!["aac".to_string()],
        });
    }
    let encoder_list = String::from_utf8_lossy(&output.stdout);

    // Detect hardware
    let mut has_nvidia = false;
    let mut has_amd = false;
    let mut has_intel = false;

    #[cfg(windows)]
    {
        // Use PowerShell for consistent UTF-8 output (WMIC outputs UTF-16 on some systems)
        let mut ps_cmd = Command::new("powershell");
        ps_cmd.args([
            "-NoProfile",
            "-Command",
            "Get-CimInstance -ClassName Win32_VideoController | Select-Object -ExpandProperty Name"
        ]);
        ps_cmd.creation_flags(CREATE_NO_WINDOW);
        let ps_result = ps_cmd.output();

        // Fall back to WMIC if PowerShell fails
        let gpu_output = match ps_result {
            Ok(output) if output.status.success() => Some(output.stdout),
            _ => {
                let mut wmic_cmd = Command::new("wmic");
                wmic_cmd.args(["path", "win32_VideoController", "get", "name"]);
                wmic_cmd.creation_flags(CREATE_NO_WINDOW);
                wmic_cmd.output().ok().map(|o| o.stdout)
            }
        };

        if let Some(stdout) = gpu_output {
            // Use lossy conversion to handle any encoding issues
            let gpu_list = String::from_utf8_lossy(&stdout).to_lowercase();
            log::debug!("GPU detection output: {}", gpu_list.trim());

            if gpu_list.contains("nvidia") || gpu_list.contains("geforce") || gpu_list.contains("quadro") {
                has_nvidia = true;
                log::info!("Detected NVIDIA GPU");
            }
            if gpu_list.contains("amd") || gpu_list.contains("radeon") {
                has_amd = true;
                log::info!("Detected AMD GPU");
            }
            if gpu_list.contains("intel") || gpu_list.contains("arc ") {
                has_intel = true;
                log::info!("Detected Intel GPU");
            }
        }
    }
    #[cfg(target_os = "linux")]
    {
        if let Ok(output) = Command::new("lspci").output() {
            if let Ok(gpu_list) = String::from_utf8(output.stdout) {
                let gpu_list = gpu_list.to_lowercase();
                if gpu_list.contains("nvidia") { has_nvidia = true; }
                if gpu_list.contains("amd") || gpu_list.contains("radeon") { has_amd = true; }
                if gpu_list.contains("intel") { has_intel = true; }
            }
        }
    }
    #[cfg(target_os = "macos")]
    {
        if let Ok(output) = Command::new("system_profiler").args(["SPDisplaysDataType"]).output() {
            if let Ok(gpu_list) = String::from_utf8(output.stdout) {
                let gpu_list = gpu_list.to_lowercase();
                if gpu_list.contains("nvidia") { has_nvidia = true; }
                if gpu_list.contains("amd") || gpu_list.contains("radeon") { has_amd = true; }
                if gpu_list.contains("intel") { has_intel = true; }
            }
        }
    }

    // Capability table for advanced filtering (expand as needed)
    // For now, we only filter by vendor, but you can add model-specific logic here
    let mut video = Vec::new();
    let mut audio = Vec::new();

    // Video encoders: (ffmpeg_name, vendor, codec)
    let video_encoder_table = [
        ("libx264", None),
        ("h264_nvenc", Some("nvidia")),
        ("hevc_nvenc", Some("nvidia")),
        ("av1_nvenc", Some("nvidia")),
        ("h264_amf", Some("amd")),
        ("hevc_amf", Some("amd")),
        ("av1_amf", Some("amd")),
        ("h264_qsv", Some("intel")),
        ("hevc_qsv", Some("intel")),
        ("av1_qsv", Some("intel")),
        ("h264_videotoolbox", Some("apple")),
        ("hevc_videotoolbox", Some("apple")),
        ("av1_videotoolbox", Some("apple")),
    ];

    for (name, vendor) in video_encoder_table.iter() {
        if encoder_list.contains(*name) {
            match *vendor {
                None => video.push((*name).to_string()),
                Some("nvidia") if has_nvidia => video.push((*name).to_string()),
                Some("amd") if has_amd => video.push((*name).to_string()),
                Some("intel") if has_intel => video.push((*name).to_string()),
                Some("apple") if cfg!(target_os = "macos") => video.push((*name).to_string()),
                _ => {},
            }
        }
    }

    // Audio encoders: always available if present in ffmpeg
    let audio_encoder_names = ["aac", "libmp3lame", "libopus"];
    for name in audio_encoder_names.iter() {
        if encoder_list.contains(*name) {
            audio.push((*name).to_string());
        }
    }

    // Ensure at least one encoder is available
    if video.is_empty() {
        video.push("libx264".to_string());
    }
    if audio.is_empty() {
        audio.push("aac".to_string());
    }

    Ok(Encoders { video, audio })
}

/// Test FFmpeg installation and return version string
#[tauri::command]
pub fn test_ffmpeg() -> Result<String, String> {
    let ffmpeg_path = find_ffmpeg();

    let mut cmd = Command::new(&ffmpeg_path);
    cmd.args(["-version"]);
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    let output = cmd.output()
        .map_err(|e| format!("Failed to run FFmpeg: {e}"))?;

    if !output.status.success() {
        return Err("FFmpeg returned an error".to_string());
    }

    let version_output = String::from_utf8_lossy(&output.stdout);

    // Extract the first line which contains the version
    let version_line = version_output
        .lines()
        .next()
        .unwrap_or("Unknown version")
        .to_string();

    Ok(version_line)
}

/// Validate a specific FFmpeg path and return version if valid
#[tauri::command]
pub fn validate_ffmpeg_path(path: String) -> Result<String, String> {
    use std::path::Path;

    let path_obj = Path::new(&path);

    // Check if path exists
    if !path_obj.exists() {
        return Err("Path does not exist".to_string());
    }

    // Check if it's a file (not a directory)
    if !path_obj.is_file() {
        return Err("Path is not a file".to_string());
    }

    // Try to run it with -version to verify it's actually FFmpeg
    let mut cmd = Command::new(&path);
    cmd.args(["-version"]);
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    let output = cmd.output()
        .map_err(|e| format!("Failed to execute: {e}"))?;

    if !output.status.success() {
        return Err("File is not a valid FFmpeg executable".to_string());
    }

    let version_output = String::from_utf8_lossy(&output.stdout);

    // Verify this is actually FFmpeg by checking for "ffmpeg" in output
    if !version_output.to_lowercase().contains("ffmpeg") {
        return Err("File is not FFmpeg".to_string());
    }

    // Extract the first line which contains the version
    let version_line = version_output
        .lines()
        .next()
        .unwrap_or("Unknown version")
        .to_string();

    Ok(version_line)
}

fn find_latest_log_file(log_dir: &Path) -> Option<PathBuf> {
    let entries = fs::read_dir(log_dir).ok()?;
    let mut latest: Option<(PathBuf, std::time::SystemTime)> = None;

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("log") {
            continue;
        }
        let modified = entry
            .metadata()
            .and_then(|metadata| metadata.modified())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);

        match latest {
            Some((_, latest_time)) if modified <= latest_time => {}
            _ => latest = Some((path, modified)),
        }
    }

    latest.map(|(path, _)| path)
}

fn read_log_lines(path: &Path, max_lines: usize) -> Result<Vec<String>, String> {
    let bytes = fs::read(path).map_err(|e| format!("Failed to read log file: {e}"))?;
    let content = String::from_utf8_lossy(&bytes);
    let lines: Vec<String> = content
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| line.to_string())
        .collect();

    let start = lines.len().saturating_sub(max_lines);
    Ok(lines[start..].to_vec())
}

/// Load recent log lines from the latest log file.
#[tauri::command]
pub fn get_recent_logs(app_handle: AppHandle, max_lines: Option<usize>) -> Result<Vec<String>, String> {
    let log_dir = app_handle
        .path()
        .app_log_dir()
        .map_err(|e| format!("Failed to resolve log directory: {e}"))?;
    let log_file = match find_latest_log_file(&log_dir) {
        Some(path) => path,
        None => return Ok(Vec::new()),
    };

    read_log_lines(&log_file, max_lines.unwrap_or(500))
}

/// Export logs to a user-selected path.
#[tauri::command]
pub fn export_logs(path: String, content: String) -> Result<(), String> {
    fs::write(&path, content).map_err(|e| format!("Failed to write log file: {e}"))
}
