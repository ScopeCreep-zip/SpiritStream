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
    let (video, audio) = select_encoders(&encoder_list, has_nvidia, has_amd, has_intel);

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

/// Filters FFmpeg's `-encoders` listing into the (video, audio) encoder names
/// SpiritStream offers, gating hardware encoders on the detected GPU vendors.
/// Pure — `get_encoders` owns the FFmpeg + GPU probe I/O. Always yields at
/// least `libx264` / `aac` so the UI never shows an empty encoder picker.
fn select_encoders(
    encoder_list: &str,
    has_nvidia: bool,
    has_amd: bool,
    has_intel: bool,
) -> (Vec<String>, Vec<String>) {
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

    (video, audio)
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

#[cfg(test)]
mod tests {
    use super::{classify, select_encoders, video_encoder_table};
    use crate::models::EncoderKind;

    #[test]
    fn classify_passthrough_has_no_family() {
        let meta = classify("copy");
        assert_eq!(meta.kind, EncoderKind::Passthrough);
        assert!(meta.family.is_empty());
    }

    #[test]
    fn classify_software_x264_and_x265() {
        let x264 = classify("libx264");
        assert_eq!(x264.kind, EncoderKind::Software);
        assert_eq!(x264.family, "libx264");

        let x265 = classify("libx265");
        assert_eq!(x265.kind, EncoderKind::Software);
        assert_eq!(x265.family, "libx265");
    }

    #[test]
    fn classify_maps_hardware_suffixes_to_their_family() {
        for (name, family) in [
            ("h264_nvenc", "nvenc"),
            ("hevc_amf", "amf"),
            ("av1_qsv", "qsv"),
            ("h264_videotoolbox", "videotoolbox"),
            ("hevc_vaapi", "vaapi"),
        ] {
            let meta = classify(name);
            assert_eq!(meta.kind, EncoderKind::Hardware, "{name}");
            assert_eq!(meta.family, family, "{name}");
        }
    }

    #[test]
    fn classify_unknown_falls_back_to_software_no_family() {
        let meta = classify("aac");
        assert_eq!(meta.kind, EncoderKind::Software);
        assert!(meta.family.is_empty());
    }

    #[test]
    fn encoder_table_lists_software_x264_with_no_vendor() {
        let table = video_encoder_table();
        let x264 = table
            .iter()
            .find(|(name, _)| *name == "libx264")
            .expect("libx264 present");
        assert_eq!(x264.1, None);
        // Every hardware entry carries a vendor tag.
        for (name, vendor) in table.iter().filter(|(n, _)| *n != "libx264") {
            assert!(vendor.is_some(), "{name} should have a vendor");
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn encoder_table_includes_vaapi_on_linux() {
        let table = video_encoder_table();
        assert!(table.iter().any(|(name, _)| *name == "h264_vaapi"));
    }

    #[test]
    fn select_software_video_and_all_audio_present() {
        let list = "libx264 aac libopus libmp3lame";
        let (video, audio) = select_encoders(list, false, false, false);
        assert_eq!(video, vec!["libx264"]);
        assert!(audio.contains(&"aac".to_string()));
        assert!(audio.contains(&"libopus".to_string()));
        assert!(audio.contains(&"libmp3lame".to_string()));
    }

    #[test]
    fn select_empty_list_falls_back_to_x264_and_aac() {
        let (video, audio) = select_encoders("", false, false, false);
        assert_eq!(video, vec!["libx264"]);
        assert_eq!(audio, vec!["aac"]);
    }

    #[test]
    fn select_gates_nvenc_on_detected_nvidia() {
        let list = "h264_nvenc hevc_nvenc";
        let (without, _) = select_encoders(list, false, false, false);
        // No vendor match + no libx264 in the list → fallback to libx264.
        assert_eq!(without, vec!["libx264"]);

        let (with, _) = select_encoders(list, true, false, false);
        assert!(with.contains(&"h264_nvenc".to_string()));
        assert!(with.contains(&"hevc_nvenc".to_string()));
        assert!(!with.contains(&"libx264".to_string()));
    }

    #[test]
    fn select_gates_amf_on_detected_amd() {
        let list = "h264_amf";
        assert!(!select_encoders(list, false, false, false)
            .0
            .contains(&"h264_amf".to_string()));
        assert!(select_encoders(list, false, true, false)
            .0
            .contains(&"h264_amf".to_string()));
    }

    #[test]
    fn select_gates_qsv_on_detected_intel() {
        let list = "h264_qsv";
        assert!(!select_encoders(list, false, false, false)
            .0
            .contains(&"h264_qsv".to_string()));
        assert!(select_encoders(list, false, false, true)
            .0
            .contains(&"h264_qsv".to_string()));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn select_gates_vaapi_on_amd_or_intel() {
        let list = "h264_vaapi";
        assert!(!select_encoders(list, false, false, false)
            .0
            .contains(&"h264_vaapi".to_string()));
        assert!(select_encoders(list, false, true, false)
            .0
            .contains(&"h264_vaapi".to_string()));
        assert!(select_encoders(list, false, false, true)
            .0
            .contains(&"h264_vaapi".to_string()));
    }
}
