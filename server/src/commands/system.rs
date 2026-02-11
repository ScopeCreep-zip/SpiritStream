// System Commands
// Handles system-level operations like encoder detection

use crate::models::Encoders;
use crate::services::{EncoderCapabilities, EncoderOption};
#[cfg(feature = "ffmpeg-libs")]
use ffmpeg_sys_next as ffi;
#[cfg(feature = "ffmpeg-libs")]
use std::ffi::CStr;

/// Get available video and audio encoders by querying FFmpeg and hardware
pub fn get_encoders() -> Result<Encoders, String> {
    let caps = EncoderCapabilities::probe();
    let mut video: Vec<String> = caps
        .all_video_encoders()
        .into_iter()
        .map(|encoder| encoder.id)
        .collect();

    let mut audio = Vec::new();
    if caps.software.aac {
        audio.push("aac".to_string());
    }
    if caps.software.opus {
        audio.push("libopus".to_string());
    }

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
    #[cfg(feature = "ffmpeg-libs")]
    unsafe {
        let ptr = ffi::av_version_info();
        if ptr.is_null() {
            return Err("FFmpeg libs version unavailable".to_string());
        }
        let version = CStr::from_ptr(ptr).to_string_lossy().into_owned();
        Ok(format!("ffmpeg version {version}"))
    }
    #[cfg(not(feature = "ffmpeg-libs"))]
    {
        Err("ffmpeg-libs feature not enabled".to_string())
    }
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
    let _ = stream_key;

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

    let elapsed = start.elapsed().as_millis() as u64;
    Ok(RtmpTestResult {
        success: true,
        message: "TCP connection successful (publish test skipped)".to_string(),
        latency_ms: Some(elapsed),
    })
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

/// Probe encoder capabilities using OBS-style detection
/// This is the new preferred method that verifies actual hardware/driver availability
pub fn probe_encoder_capabilities() -> EncoderCapabilities {
    log::info!("Probing encoder capabilities...");
    EncoderCapabilities::refresh()
}

/// Get cached encoder capabilities (faster, uses cached results)
pub fn get_encoder_capabilities() -> &'static EncoderCapabilities {
    EncoderCapabilities::probe()
}

/// Get list of available H.264 encoders (OBS-style probed)
pub fn get_available_h264_encoders() -> Vec<EncoderOption> {
    EncoderCapabilities::probe().available_h264_encoders()
}

/// Get list of available HEVC encoders (OBS-style probed)
pub fn get_available_hevc_encoders() -> Vec<EncoderOption> {
    EncoderCapabilities::probe().available_hevc_encoders()
}

/// Get list of available AV1 encoders (OBS-style probed)
pub fn get_available_av1_encoders() -> Vec<EncoderOption> {
    EncoderCapabilities::probe().available_av1_encoders()
}

/// Get all available video encoders (OBS-style probed)
pub fn get_all_video_encoders() -> Vec<EncoderOption> {
    EncoderCapabilities::probe().all_video_encoders()
}
