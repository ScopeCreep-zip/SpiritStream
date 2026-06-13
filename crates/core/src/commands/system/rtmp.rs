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
#[ts(export, export_to = "../../../packages/types/src/generated/")]
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
        Ok(RtmpTestResult {
            success: false,
            message: classify_rtmp_error(&stderr),
            latency_ms: Some(elapsed),
        })
    }
}

/// Maps FFmpeg's RTMP-probe stderr to a user-friendly failure message.
/// Pure — the network/spawn I/O lives in the caller.
fn classify_rtmp_error(stderr: &str) -> String {
    if stderr.contains("Connection refused") {
        "Connection refused - server not accepting connections".to_string()
    } else if stderr.contains("Connection timed out") {
        "Connection timed out - server not responding".to_string()
    } else if stderr.contains("Server returned 404") || stderr.contains("NetStream.Publish.BadName")
    {
        "Stream key rejected - check your stream key".to_string()
    } else if stderr.contains("Authorization") || stderr.contains("auth") || stderr.contains("401")
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

#[cfg(test)]
mod tests {
    use super::{classify_rtmp_error, parse_rtmp_url};
    use crate::errors::CoreError;

    fn validation_codes(err: CoreError) -> Vec<String> {
        match err {
            CoreError::ValidationFailed { reasons } => {
                reasons.into_iter().map(|r| r.code).collect()
            }
            other => panic!("expected ValidationFailed, got {other:?}"),
        }
    }

    #[test]
    fn plain_rtmp_defaults_to_port_1935() {
        let (host, port) = parse_rtmp_url("rtmp://live.example.com/app/key").unwrap();
        assert_eq!(host, "live.example.com");
        assert_eq!(port, 1935);
    }

    #[test]
    fn rtmps_defaults_to_port_443() {
        let (host, port) = parse_rtmp_url("rtmps://secure.example.com/app").unwrap();
        assert_eq!(host, "secure.example.com");
        assert_eq!(port, 443);
    }

    #[test]
    fn explicit_port_overrides_the_scheme_default() {
        let (host, port) = parse_rtmp_url("rtmp://host.example.com:1234/app/key").unwrap();
        assert_eq!(host, "host.example.com");
        assert_eq!(port, 1234);
    }

    #[test]
    fn host_without_path_is_accepted() {
        let (host, port) = parse_rtmp_url("rtmp://host.example.com").unwrap();
        assert_eq!(host, "host.example.com");
        assert_eq!(port, 1935);
    }

    #[test]
    fn surrounding_whitespace_is_trimmed() {
        let (host, port) = parse_rtmp_url("  rtmp://host.example.com/app  ").unwrap();
        assert_eq!(host, "host.example.com");
        assert_eq!(port, 1935);
    }

    #[test]
    fn non_rtmp_scheme_is_rejected() {
        let err = parse_rtmp_url("https://host.example.com/app").unwrap_err();
        assert_eq!(validation_codes(err), vec!["invalid_rtmp_url"]);
    }

    #[test]
    fn non_numeric_port_is_rejected() {
        let err = parse_rtmp_url("rtmp://host.example.com:notaport/app").unwrap_err();
        assert_eq!(validation_codes(err), vec!["invalid_rtmp_url"]);
    }

    #[test]
    fn empty_host_is_rejected() {
        let err = parse_rtmp_url("rtmp:///app/key").unwrap_err();
        assert_eq!(validation_codes(err), vec!["invalid_rtmp_url"]);
    }

    #[test]
    fn classify_connection_refused() {
        let msg = classify_rtmp_error("rtmp://x: Connection refused\n");
        assert_eq!(msg, "Connection refused - server not accepting connections");
    }

    #[test]
    fn classify_connection_timed_out() {
        let msg = classify_rtmp_error("Connection timed out");
        assert_eq!(msg, "Connection timed out - server not responding");
    }

    #[test]
    fn classify_bad_stream_key_from_404() {
        assert_eq!(
            classify_rtmp_error("Server returned 404 Not Found"),
            "Stream key rejected - check your stream key"
        );
    }

    #[test]
    fn classify_bad_stream_key_from_publish_badname() {
        assert_eq!(
            classify_rtmp_error("NetStream.Publish.BadName"),
            "Stream key rejected - check your stream key"
        );
    }

    #[test]
    fn classify_auth_failure() {
        assert_eq!(
            classify_rtmp_error("HTTP 401 Authorization required"),
            "Authentication failed - invalid stream key"
        );
    }

    #[test]
    fn classify_connection_rejected() {
        assert_eq!(
            classify_rtmp_error("NetConnection.Connect.Rejected by app"),
            "Connection rejected by server - may need authentication"
        );
    }

    #[test]
    fn classify_already_publishing_is_valid_key() {
        assert_eq!(
            classify_rtmp_error("Already publishing to this stream"),
            "Stream key is valid but already in use"
        );
    }

    #[test]
    fn classify_empty_stderr_is_unknown() {
        assert_eq!(classify_rtmp_error(""), "Connection failed - unknown error");
    }

    #[test]
    fn classify_unknown_error_is_truncated_to_200_chars() {
        let long = "x".repeat(300);
        let msg = classify_rtmp_error(&long);
        assert!(msg.starts_with("Connection failed: "));
        // 200-char cap on the stderr body (the prefix is separate).
        let body = msg.trim_start_matches("Connection failed: ");
        assert_eq!(body.chars().count(), 200);
    }

    #[test]
    fn classify_unknown_error_is_trimmed() {
        let msg = classify_rtmp_error("  weird ffmpeg failure  ");
        assert_eq!(msg, "Connection failed: weird ffmpeg failure");
    }

    #[test]
    fn classify_refused_takes_priority_over_generic() {
        // A stderr that contains "Connection refused" never falls through to
        // the generic truncation branch even if other text is present.
        let msg = classify_rtmp_error("noise noise Connection refused noise");
        assert_eq!(msg, "Connection refused - server not accepting connections");
    }
}
