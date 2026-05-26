//! RTMP target connectivity test — TCP-only probe followed by a 2-second
//! FFmpeg publish to verify the endpoint accepts streams.

use std::net::TcpStream;
use std::process::Command;
use std::time::{Duration, Instant};

use crate::errors::{CoreError, ValidationIssue};

use super::{find_ffmpeg, hide_console};

/// Result of testing an RTMP target
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../../packages/types/src/generated/")]
pub struct RtmpTestResult {
    pub success: bool,
    pub message: String,
    /// Time taken in milliseconds. JSON `number` — handshake latency
    /// is bounded by single-digit seconds; precision is moot.
    #[ts(type = "number | null")]
    pub latency_ms: Option<u64>,
}

/// Test RTMP target connectivity by attempting a brief connection.
///
/// 1. TCP connectivity test to the RTMP host:port
/// 2. Brief FFmpeg publish attempt to verify the endpoint accepts streams
pub fn test_rtmp_target(url: String, stream_key: String) -> Result<RtmpTestResult, CoreError> {
    let start = Instant::now();

    let (host, port) = parse_rtmp_url(&url)?;

    // TCP connectivity test (fast check)
    let tcp_timeout = Duration::from_secs(5);
    let addr = format!("{host}:{port}");

    match TcpStream::connect_timeout(
        &addr.parse().map_err(|e| CoreError::Internal {
            context: format!("Invalid address {addr}: {e}"),
        })?,
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

    // FFmpeg publish test: generate a 2-second test pattern and attempt
    // to publish.
    let ffmpeg_path = find_ffmpeg();
    let full_url = if url.ends_with('/') {
        format!("{url}{stream_key}")
    } else {
        format!("{url}/{stream_key}")
    };

    let mut cmd = Command::new(&ffmpeg_path);
    cmd.args([
        "-hide_banner",
        "-loglevel",
        "error",
        "-f",
        "lavfi",
        "-i",
        "testsrc=duration=2:size=320x240:rate=30",
        "-f",
        "lavfi",
        "-i",
        "anullsrc=r=44100:cl=stereo",
        "-t",
        "2",
        "-c:v",
        "libx264",
        "-preset",
        "ultrafast",
        "-tune",
        "zerolatency",
        "-b:v",
        "500k",
        "-c:a",
        "aac",
        "-b:a",
        "64k",
        "-f",
        "flv",
        &full_url,
    ]);

    hide_console(&mut cmd);

    let output = cmd.output().map_err(|_| CoreError::FfmpegNotFound)?;

    let elapsed = start.elapsed().as_millis() as u64;

    if output.status.success() {
        Ok(RtmpTestResult {
            success: true,
            message: "Connection successful - stream accepted".to_string(),
            latency_ms: Some(elapsed),
        })
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // Parse common RTMP errors for user-friendly messages.
        let message = if stderr.contains("Connection refused") {
            "Connection refused - server not accepting connections".to_string()
        } else if stderr.contains("Connection timed out") {
            "Connection timed out - server not responding".to_string()
        } else if stderr.contains("Server returned 404")
            || stderr.contains("NetStream.Publish.BadName")
        {
            "Stream key rejected - check your stream key".to_string()
        } else if stderr.contains("Authorization")
            || stderr.contains("auth")
            || stderr.contains("401")
        {
            "Authentication failed - invalid stream key".to_string()
        } else if stderr.contains("NetConnection.Connect.Rejected") {
            "Connection rejected by server - may need authentication".to_string()
        } else if stderr.contains("Already publishing") {
            // This actually means the key is valid; someone is already using it.
            "Stream key is valid but already in use".to_string()
        } else if stderr.is_empty() {
            "Connection failed - unknown error".to_string()
        } else {
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

/// Parse an RTMP URL to extract host and port.
fn parse_rtmp_url(url: &str) -> Result<(String, u16), CoreError> {
    fn bad(msg: impl Into<String>) -> CoreError {
        CoreError::ValidationFailed {
            reasons: vec![ValidationIssue {
                code: "invalid_rtmp_url".into(),
                message: msg.into(),
                path: Some("/url".into()),
            }],
        }
    }

    let url = url.trim();

    let (is_secure, rest) = if let Some(rest) = url.strip_prefix("rtmps://") {
        (true, rest)
    } else if let Some(rest) = url.strip_prefix("rtmp://") {
        (false, rest)
    } else {
        return Err(bad("Invalid RTMP URL: must start with rtmp:// or rtmps://"));
    };

    // URL format: rtmp://host:port/app/stream or rtmp://host/app/stream
    let host_port = rest.split('/').next().unwrap_or(rest);

    let (host, port) = if host_port.contains(':') {
        let parts: Vec<&str> = host_port.splitn(2, ':').collect();
        let port: u16 = parts[1]
            .parse()
            .map_err(|_| bad(format!("Invalid port in URL: {}", parts[1])))?;
        (parts[0].to_string(), port)
    } else {
        let default_port = if is_secure { 443 } else { 1935 };
        (host_port.to_string(), default_port)
    };

    if host.is_empty() {
        return Err(bad("Empty host in RTMP URL"));
    }

    Ok((host, port))
}
