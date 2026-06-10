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

#[cfg(test)]
mod tests {
    use super::validate_ffmpeg_path;
    use crate::errors::CoreError;

    fn validation_code(err: CoreError) -> String {
        match err {
            CoreError::ValidationFailed { reasons } => {
                reasons.into_iter().next().expect("a reason").code
            }
            other => panic!("expected ValidationFailed, got {other:?}"),
        }
    }

    #[test]
    fn missing_path_is_rejected_before_exec() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("no-such-ffmpeg");
        let err = validate_ffmpeg_path(missing.to_string_lossy().into_owned()).unwrap_err();
        assert_eq!(validation_code(err), "invalid_ffmpeg_path");
    }

    #[test]
    fn directory_path_is_rejected_before_exec() {
        let dir = tempfile::tempdir().unwrap();
        let err = validate_ffmpeg_path(dir.path().to_string_lossy().into_owned()).unwrap_err();
        assert_eq!(validation_code(err), "invalid_ffmpeg_path");
    }
}
