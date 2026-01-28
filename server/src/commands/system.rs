// System Commands
// Handles system-level operations like encoder detection

use std::process::Command;
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
    #[cfg(target_os = "linux")]
    let mut video_encoder_table: Vec<(&str, Option<&str>)> = vec![
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

    #[cfg(target_os = "linux")]
    video_encoder_table.extend([
        ("h264_vaapi", Some("vaapi")),
        ("hevc_vaapi", Some("vaapi")),
        ("av1_vaapi", Some("vaapi")),
    ]);

    #[cfg(not(target_os = "linux"))]
    let video_encoder_table: Vec<(&str, Option<&str>)> = vec![
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
                Some("vaapi") if (has_amd || has_intel) => video.push((*name).to_string()),
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

/// Get just the FFmpeg version number (e.g., "7.1" instead of full version line)
pub fn get_ffmpeg_version() -> Result<String, String> {
    let version_line = test_ffmpeg()?;
    // Use the ffmpeg_downloader's version extraction
    crate::services::FFmpegDownloader::extract_version_from_text(&version_line)
        .ok_or_else(|| format!("Could not parse version from: {version_line}"))
}

/// Result of testing an RTMP target
#[derive(Debug, Clone, serde::Serialize)]
pub struct RtmpTestResult {
    pub success: bool,
    pub message: String,
    /// Time taken in milliseconds
    pub latency_ms: Option<u64>,
}

/// Test RTMP target connectivity by attempting a brief connection
///
/// This performs:
/// 1. TCP connectivity test to the RTMP host:port
/// 2. Brief FFmpeg publish attempt to verify the endpoint accepts streams
pub fn test_rtmp_target(url: String, stream_key: String) -> Result<RtmpTestResult, String> {
    use std::net::TcpStream;
    use std::time::{Duration, Instant};

    let start = Instant::now();

    // Parse the RTMP URL to extract host and port
    let (host, port) = parse_rtmp_url(&url)?;

    // Step 1: TCP connectivity test (fast check)
    let tcp_timeout = Duration::from_secs(5);
    let addr = format!("{host}:{port}");

    match TcpStream::connect_timeout(
        &addr.parse().map_err(|e| format!("Invalid address {addr}: {e}"))?,
        tcp_timeout,
    ) {
        Ok(_) => {
            log::info!("TCP connection to {addr} successful");
        }
        Err(e) => {
            return Ok(RtmpTestResult {
                success: false,
                message: format!("Cannot reach {addr} - {e}"),
                latency_ms: Some(start.elapsed().as_millis() as u64),
            });
        }
    }

    // Step 2: FFmpeg publish test
    // Generate a 2-second test pattern and attempt to publish
    let ffmpeg_path = find_ffmpeg();
    let full_url = if url.ends_with('/') {
        format!("{url}{stream_key}")
    } else {
        format!("{url}/{stream_key}")
    };

    // Build FFmpeg command for a brief test publish
    // Using testsrc for video and anullsrc for audio, 2 seconds duration
    let mut cmd = Command::new(&ffmpeg_path);
    cmd.args([
        "-hide_banner",
        "-loglevel", "error",
        // Test video source
        "-f", "lavfi",
        "-i", "testsrc=duration=2:size=320x240:rate=30",
        // Test audio source
        "-f", "lavfi",
        "-i", "anullsrc=r=44100:cl=stereo",
        "-t", "2",
        // Fast encoding settings
        "-c:v", "libx264",
        "-preset", "ultrafast",
        "-tune", "zerolatency",
        "-b:v", "500k",
        "-c:a", "aac",
        "-b:a", "64k",
        // Output format
        "-f", "flv",
        &full_url,
    ]);

    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);

    // Set a timeout for the FFmpeg process
    let output = cmd.output()
        .map_err(|e| format!("Failed to run FFmpeg: {e}"))?;

    let elapsed = start.elapsed().as_millis() as u64;

    if output.status.success() {
        Ok(RtmpTestResult {
            success: true,
            message: "Connection successful - stream accepted".to_string(),
            latency_ms: Some(elapsed),
        })
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // Parse common RTMP errors for user-friendly messages
        let message = if stderr.contains("Connection refused") {
            "Connection refused - server not accepting connections".to_string()
        } else if stderr.contains("Connection timed out") {
            "Connection timed out - server not responding".to_string()
        } else if stderr.contains("Server returned 404") || stderr.contains("NetStream.Publish.BadName") {
            "Stream key rejected - check your stream key".to_string()
        } else if stderr.contains("Authorization") || stderr.contains("auth") || stderr.contains("401") {
            "Authentication failed - invalid stream key".to_string()
        } else if stderr.contains("NetConnection.Connect.Rejected") {
            "Connection rejected by server - may need authentication".to_string()
        } else if stderr.contains("Already publishing") {
            // This actually means the key is valid, someone is already using it
            "Stream key is valid but already in use".to_string()
        } else if stderr.is_empty() {
            "Connection failed - unknown error".to_string()
        } else {
            // Return a truncated error for other cases
            let truncated: String = stderr.chars().take(200).collect();
            format!("Connection failed: {}", truncated.trim())
        };

        Ok(RtmpTestResult {
            success: false,
            message,
            latency_ms: Some(elapsed),
        })
    }
}

/// Parse an RTMP URL to extract host and port
fn parse_rtmp_url(url: &str) -> Result<(String, u16), String> {
    let url = url.trim();

    // Handle rtmp:// and rtmps:// protocols
    let (is_secure, rest) = if let Some(rest) = url.strip_prefix("rtmps://") {
        (true, rest)
    } else if let Some(rest) = url.strip_prefix("rtmp://") {
        (false, rest)
    } else {
        return Err("Invalid RTMP URL: must start with rtmp:// or rtmps://".to_string());
    };

    // Extract host:port from the path
    // URL format: rtmp://host:port/app/stream or rtmp://host/app/stream
    let host_port = rest.split('/').next().unwrap_or(rest);

    let (host, port) = if host_port.contains(':') {
        let parts: Vec<&str> = host_port.splitn(2, ':').collect();
        let port: u16 = parts[1].parse()
            .map_err(|_| format!("Invalid port in URL: {}", parts[1]))?;
        (parts[0].to_string(), port)
    } else {
        // Default ports
        let default_port = if is_secure { 443 } else { 1935 };
        (host_port.to_string(), default_port)
    };

    if host.is_empty() {
        return Err("Empty host in RTMP URL".to_string());
    }

    Ok((host, port))
}

/// Validate a specific FFmpeg path and return version if valid
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
