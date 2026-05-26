//! FFmpeg probes — `test_ffmpeg` (existence + version) and
//! `validate_ffmpeg_path` (operator-supplied override).

use std::path::Path;
use std::process::Command;

use crate::errors::{CoreError, ValidationIssue};

use super::{find_ffmpeg, hide_console};

/// Test FFmpeg installation and return version string
pub fn test_ffmpeg() -> Result<String, CoreError> {
    let ffmpeg_path = find_ffmpeg();

    let mut cmd = Command::new(&ffmpeg_path);
    cmd.args(["-version"]);
    hide_console(&mut cmd);
    let output = cmd.output().map_err(|_| CoreError::FfmpegNotFound)?;

    if !output.status.success() {
        return Err(CoreError::FfmpegNotFound);
    }

    let version_output = String::from_utf8_lossy(&output.stdout);

    let version_line = version_output
        .lines()
        .next()
        .unwrap_or("Unknown version")
        .to_string();

    Ok(version_line)
}

/// Validate a specific FFmpeg path and return version if valid
pub fn validate_ffmpeg_path(path: String) -> Result<String, CoreError> {
    fn invalid(msg: impl Into<String>) -> CoreError {
        CoreError::ValidationFailed {
            reasons: vec![ValidationIssue {
                code: "invalid_ffmpeg_path".into(),
                message: msg.into(),
                path: Some("/path".into()),
            }],
        }
    }

    let path_obj = Path::new(&path);

    if !path_obj.exists() {
        return Err(invalid("Path does not exist"));
    }

    if !path_obj.is_file() {
        return Err(invalid("Path is not a file"));
    }

    // Try to run it with -version to verify it's actually FFmpeg
    let mut cmd = Command::new(&path);
    cmd.args(["-version"]);
    hide_console(&mut cmd);
    let output = cmd
        .output()
        .map_err(|_| invalid("Failed to execute as FFmpeg"))?;

    if !output.status.success() {
        return Err(invalid("File is not a valid FFmpeg executable"));
    }

    let version_output = String::from_utf8_lossy(&output.stdout);

    // Verify this is actually FFmpeg by checking for "ffmpeg" in output
    if !version_output.to_lowercase().contains("ffmpeg") {
        return Err(invalid("File is not FFmpeg"));
    }

    let version_line = version_output
        .lines()
        .next()
        .unwrap_or("Unknown version")
        .to_string();

    Ok(version_line)
}
