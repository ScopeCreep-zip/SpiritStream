//! System commands — encoder discovery, FFmpeg probes, RTMP test.
//!
//! Submodules:
//! - [`encoder`] — `get_encoders` (FFmpeg `-encoders` + per-OS GPU probe).
//! - [`ffmpeg`] — `test_ffmpeg`, `validate_ffmpeg_path`.
//! - [`rtmp`] — `test_rtmp_target`, `RtmpTestResult`, RTMP URL parser.
//!
//! `find_ffmpeg` is the shared discovery entry point; all submodules
//! call into it instead of duplicating the lookup chain. Windows
//! console-hide flags live here too because every submodule needs them.

use std::process::Command;

mod encoder;
mod ffmpeg;
mod rtmp;

pub use encoder::get_encoders;
pub use ffmpeg::{test_ffmpeg, validate_ffmpeg_path};
pub use rtmp::{test_rtmp_target, RtmpTestResult};

#[cfg(windows)]
pub(super) use std::os::windows::process::CommandExt;
#[cfg(windows)]
pub(super) const CREATE_NO_WINDOW: u32 = 0x08000000;

/// Find FFmpeg path. Delegates to `FFmpegLocator::discover` so this
/// honors the single discovery chain rule documented in
/// `crates/core/src/services/ffmpeg_locator.rs`:
///   1. settings.ffmpeg_path (operator override)
///   2. SPIRITSTREAM_FFMPEG_PATH env (Tauri shell bundled sidecar)
///   3. $PATH lookup (Linux distro / Docker / brew)
///   4. None → caller refuses
///
/// Returns a literal `"ffmpeg"` string when discovery finds nothing, so
/// `Command::new(path).spawn()` produces a clean ENOENT the caller maps
/// to `CoreError::FfmpegNotFound` without losing the diagnostic.
pub(super) fn find_ffmpeg() -> String {
    use crate::services::FFmpegLocator;
    use crate::services::SettingsManager;
    match FFmpegLocator::discover(None::<&SettingsManager>) {
        Some(path) => path.to_string_lossy().to_string(),
        None => {
            #[cfg(windows)]
            {
                "ffmpeg.exe".to_string()
            }
            #[cfg(not(windows))]
            {
                "ffmpeg".to_string()
            }
        }
    }
}

/// Apply per-OS hardening to a spawned `Command`. On Windows that
/// means hiding the console window for child processes; on Unix the
/// helper is a no-op so submodules call it unconditionally without
/// `cfg!` noise.
pub(super) fn hide_console(_cmd: &mut Command) {
    #[cfg(windows)]
    {
        _cmd.creation_flags(CREATE_NO_WINDOW);
    }
}
