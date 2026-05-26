//! Encoder discovery — query FFmpeg's `-encoders` list, probe the host
//! GPU, and emit an `Encoders` snapshot. Empty list when FFmpeg isn't
//! installed (discovery, not execution: `CoreError::FfmpegNotFound`
//! lives on the start-stream path).

use std::process::Command;

use crate::errors::CoreError;
use crate::models::{EncoderKind, EncoderMeta, Encoders};

use super::{find_ffmpeg, hide_console};

/// Get available video and audio encoders by querying FFmpeg and hardware.
///
/// This is a *discovery* endpoint. When FFmpeg isn't installed the honest
/// answer is "no encoders available" — an empty result, not an error.
/// Distinguishes from *execution* paths (start stream, test encoder) where
/// FFmpeg-missing is a hard failure that surfaces `CoreError::FfmpegNotFound`.
/// Frontend uses `/system/ffmpeg/test` to detect installation status; this
/// endpoint just enumerates what's available given the current install.
pub fn get_encoders() -> Result<Encoders, CoreError> {
    let ffmpeg_path = find_ffmpeg();
    let mut cmd = Command::new(&ffmpeg_path);
    cmd.args(["-encoders", "-hide_banner"]);
    hide_console(&mut cmd);
    let output = match cmd.output() {
        Ok(out) => out,
        Err(err) => {
            log::warn!("FFmpeg not available for encoder discovery: {err}");
            return Ok(Encoders::default());
        }
    };
    if !output.status.success() {
        return Ok(Encoders::default());
    }
    let encoder_list = String::from_utf8_lossy(&output.stdout);

    let (has_nvidia, has_amd, has_intel) = detect_gpus();

    let mut video = Vec::new();
    let mut audio = Vec::new();

    for (name, vendor) in video_encoder_table().iter() {
        if encoder_list.contains(*name) {
            match *vendor {
                None => video.push((*name).to_string()),
                Some("nvidia") if has_nvidia => video.push((*name).to_string()),
                Some("amd") if has_amd => video.push((*name).to_string()),
                Some("intel") if has_intel => video.push((*name).to_string()),
                Some("vaapi") if has_amd || has_intel => video.push((*name).to_string()),
                Some("apple") if cfg!(target_os = "macos") => video.push((*name).to_string()),
                _ => {}
            }
        }
    }

    let audio_encoder_names = ["aac", "libmp3lame", "libopus"];
    for name in audio_encoder_names.iter() {
        if encoder_list.contains(*name) {
            audio.push((*name).to_string());
        }
    }

    if video.is_empty() {
        video.push("libx264".to_string());
    }
    if audio.is_empty() {
        audio.push("aac".to_string());
    }

    let mut metadata = std::collections::HashMap::new();
    for name in video.iter().chain(audio.iter()) {
        metadata.insert(name.clone(), classify(name));
    }

    Ok(Encoders {
        video,
        audio,
        metadata,
    })
}

/// Per-OS GPU probe. Returns `(has_nvidia, has_amd, has_intel)`.
fn detect_gpus() -> (bool, bool, bool) {
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
            "Get-CimInstance -ClassName Win32_VideoController | Select-Object -ExpandProperty Name",
        ]);
        hide_console(&mut ps_cmd);
        let ps_result = ps_cmd.output();

        // Fall back to WMIC if PowerShell fails
        let gpu_output = match ps_result {
            Ok(output) if output.status.success() => Some(output.stdout),
            _ => {
                let mut wmic_cmd = Command::new("wmic");
                wmic_cmd.args(["path", "win32_VideoController", "get", "name"]);
                hide_console(&mut wmic_cmd);
                wmic_cmd.output().ok().map(|o| o.stdout)
            }
        };

        if let Some(stdout) = gpu_output {
            let gpu_list = String::from_utf8_lossy(&stdout).to_lowercase();
            log::debug!("GPU detection output: {}", gpu_list.trim());

            if gpu_list.contains("nvidia")
                || gpu_list.contains("geforce")
                || gpu_list.contains("quadro")
            {
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
                if gpu_list.contains("nvidia") {
                    has_nvidia = true;
                }
                if gpu_list.contains("amd") || gpu_list.contains("radeon") {
                    has_amd = true;
                }
                if gpu_list.contains("intel") {
                    has_intel = true;
                }
            }
        }
    }
    #[cfg(target_os = "macos")]
    {
        if let Ok(output) = Command::new("system_profiler")
            .args(["SPDisplaysDataType"])
            .output()
        {
            if let Ok(gpu_list) = String::from_utf8(output.stdout) {
                let gpu_list = gpu_list.to_lowercase();
                if gpu_list.contains("nvidia") {
                    has_nvidia = true;
                }
                if gpu_list.contains("amd") || gpu_list.contains("radeon") {
                    has_amd = true;
                }
                if gpu_list.contains("intel") {
                    has_intel = true;
                }
            }
        }
    }

    (has_nvidia, has_amd, has_intel)
}

/// Build the video-encoder capability table, with vaapi entries only on
/// Linux (where Mesa exposes the VAAPI runtime by default).
fn video_encoder_table() -> Vec<(&'static str, Option<&'static str>)> {
    #[cfg_attr(not(target_os = "linux"), allow(unused_mut))]
    let mut table: Vec<(&str, Option<&str>)> = vec![
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
    table.extend([
        ("h264_vaapi", Some("vaapi")),
        ("hevc_vaapi", Some("vaapi")),
        ("av1_vaapi", Some("vaapi")),
    ]);
    table
}

/// Classify an encoder name into kind + preset family. Frontend uses
/// this to render the hardware/software badge and to pick the preset
/// list for the codec the user selected, without local substring
/// matching.
fn classify(name: &str) -> EncoderMeta {
    if name == "copy" {
        return EncoderMeta {
            kind: EncoderKind::Passthrough,
            family: String::new(),
        };
    }
    if name == "libx264" {
        return EncoderMeta {
            kind: EncoderKind::Software,
            family: "libx264".to_string(),
        };
    }
    if name == "libx265" {
        return EncoderMeta {
            kind: EncoderKind::Software,
            family: "libx265".to_string(),
        };
    }
    if name.ends_with("_nvenc") {
        return EncoderMeta {
            kind: EncoderKind::Hardware,
            family: "nvenc".to_string(),
        };
    }
    if name.ends_with("_amf") {
        return EncoderMeta {
            kind: EncoderKind::Hardware,
            family: "amf".to_string(),
        };
    }
    if name.ends_with("_qsv") {
        return EncoderMeta {
            kind: EncoderKind::Hardware,
            family: "qsv".to_string(),
        };
    }
    if name.ends_with("_videotoolbox") {
        return EncoderMeta {
            kind: EncoderKind::Hardware,
            family: "videotoolbox".to_string(),
        };
    }
    if name.ends_with("_vaapi") {
        return EncoderMeta {
            kind: EncoderKind::Hardware,
            family: "vaapi".to_string(),
        };
    }
    // Audio encoders and anything unclassified.
    EncoderMeta {
        kind: EncoderKind::Software,
        family: String::new(),
    }
}
