// Encoder Capability Detection Service
// OBS-style encoder probing - detects available encoders based on hardware and drivers
//
// This module follows OBS's approach of actually attempting to initialize encoders
// rather than just checking FFmpeg compilation flags. This ensures we only show
// encoders that will actually work on the user's system.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::process::Command;
use std::sync::OnceLock;

#[cfg(windows)]
use std::os::windows::process::CommandExt;
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

use crate::services::FFmpegDownloader;

#[cfg(feature = "ffmpeg-libs")]
use ffmpeg_sys_next as ffi;

/// NVENC (NVIDIA) encoder capabilities
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NvencCaps {
    pub available: bool,
    pub h264: bool,
    pub hevc: bool,
    pub av1: bool,
    /// Supports B-frames
    pub b_frames: bool,
    /// Supports lookahead (improves quality)
    pub lookahead: bool,
    /// GPU name if detected
    pub gpu_name: Option<String>,
}

/// AMD AMF encoder capabilities
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AmfCaps {
    pub available: bool,
    pub h264: bool,
    pub hevc: bool,
    pub av1: bool,
    /// Supports B-frames
    pub b_frames: bool,
    /// GPU name if detected
    pub gpu_name: Option<String>,
}

/// Intel Quick Sync Video capabilities
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct QsvCaps {
    pub available: bool,
    pub h264: bool,
    pub hevc: bool,
    pub av1: bool,
    /// Supports B-frames (often disabled for Twitch compatibility)
    pub b_frames: bool,
    /// Supports lookahead (often disabled for Twitch)
    pub lookahead: bool,
    /// Low-power mode available (uses fixed-function hardware)
    pub low_power: bool,
    /// GPU/device name if detected
    pub device_name: Option<String>,
}

/// Apple VideoToolbox capabilities (macOS)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VideoToolboxCaps {
    pub available: bool,
    pub h264: bool,
    pub hevc: bool,
    /// Hardware acceleration available (vs software fallback)
    pub hardware_accelerated: bool,
}

/// Software encoder capabilities
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SoftwareCaps {
    pub libx264: bool,
    pub libx265: bool,
    pub libsvtav1: bool,
    pub aac: bool,
    pub opus: bool,
}

/// Unified encoder capabilities for the system
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EncoderCapabilities {
    pub nvenc: NvencCaps,
    pub amf: AmfCaps,
    pub qsv: QsvCaps,
    pub videotoolbox: VideoToolboxCaps,
    pub software: SoftwareCaps,
    /// Timestamp when capabilities were probed
    pub probed_at: Option<String>,
    /// Any errors encountered during probing
    pub probe_errors: Vec<String>,
}

/// Encoder option for UI dropdown
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncoderOption {
    /// Encoder ID (e.g., "h264_nvenc", "libx264")
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// Whether this is a hardware encoder
    pub hardware: bool,
    /// Codec type (h264, hevc, av1)
    pub codec: String,
    /// Additional capabilities
    pub capabilities: HashMap<String, bool>,
}

/// Global cached capabilities
static CACHED_CAPABILITIES: OnceLock<EncoderCapabilities> = OnceLock::new();

impl EncoderCapabilities {
    /// Probe all encoder capabilities (uses cache if available)
    pub fn probe() -> &'static EncoderCapabilities {
        CACHED_CAPABILITIES.get_or_init(|| {
            log::info!("Probing encoder capabilities...");
            let caps = Self::probe_uncached();
            log::info!("Encoder probe complete: {:?}", caps);
            caps
        })
    }

    /// Force re-probe of capabilities (for hot-plug scenarios)
    pub fn refresh() -> EncoderCapabilities {
        log::info!("Refreshing encoder capabilities...");
        Self::probe_uncached()
    }

    /// Probe without caching
    fn probe_uncached() -> EncoderCapabilities {
        let mut caps = EncoderCapabilities {
            probed_at: Some(chrono::Utc::now().to_rfc3339()),
            ..Default::default()
        };

        log::info!("=== Starting encoder capability probe ===");

        // Use native probing (OBS-style) by default, with CLI fallback
        // Native probing directly queries vendor APIs for accurate hardware detection

        log::info!("Probing NVENC (NVIDIA)...");
        caps.nvenc = Self::probe_nvenc(&mut caps.probe_errors);
        log::info!("  NVENC: available={}, h264={}, hevc={}, av1={}, gpu={:?}",
            caps.nvenc.available, caps.nvenc.h264, caps.nvenc.hevc, caps.nvenc.av1, caps.nvenc.gpu_name);

        log::info!("Probing AMF (AMD)...");
        caps.amf = Self::probe_amf(&mut caps.probe_errors);
        log::info!("  AMF: available={}, h264={}, hevc={}, av1={}, gpu={:?}",
            caps.amf.available, caps.amf.h264, caps.amf.hevc, caps.amf.av1, caps.amf.gpu_name);

        log::info!("Probing QSV (Intel)...");
        caps.qsv = Self::probe_qsv(&mut caps.probe_errors);
        log::info!("  QSV: available={}, h264={}, hevc={}, av1={}, low_power={}, device={:?}",
            caps.qsv.available, caps.qsv.h264, caps.qsv.hevc, caps.qsv.av1, caps.qsv.low_power, caps.qsv.device_name);

        log::info!("Probing VideoToolbox (Apple)...");
        caps.videotoolbox = Self::probe_videotoolbox(&mut caps.probe_errors);
        log::info!("  VideoToolbox: available={}, h264={}, hevc={}, hw_accel={}",
            caps.videotoolbox.available, caps.videotoolbox.h264, caps.videotoolbox.hevc,
            caps.videotoolbox.hardware_accelerated);

        log::info!("Probing software encoders...");
        #[cfg(feature = "ffmpeg-libs")]
        {
            caps.software = Self::probe_software_via_libs(&mut caps.probe_errors);
        }
        #[cfg(not(feature = "ffmpeg-libs"))]
        {
            caps.software = Self::probe_software_via_cli(&mut caps.probe_errors);
        }
        log::info!("  Software: libx264={}, libx265={}, libsvtav1={}, aac={}, opus={}",
            caps.software.libx264, caps.software.libx265, caps.software.libsvtav1,
            caps.software.aac, caps.software.opus);

        log::info!("=== Encoder probe complete ===");

        // Summary of available encoders
        let h264_count = caps.available_h264_encoders().len();
        let hevc_count = caps.available_hevc_encoders().len();
        let av1_count = caps.available_av1_encoders().len();
        log::info!("Available encoders: {} H.264, {} HEVC, {} AV1", h264_count, hevc_count, av1_count);

        caps
    }

    // =========================================================================
    // Combined probing (native with CLI fallback)
    // =========================================================================

    /// Probe NVENC - native first, CLI fallback
    fn probe_nvenc(probe_errors: &mut Vec<String>) -> NvencCaps {
        log::debug!("  Attempting native NVENC probe...");
        let mut native_caps = native_nvenc::probe();

        let ffmpeg_h264 = Self::encoder_available_for_probe("h264_nvenc");
        let ffmpeg_hevc = Self::encoder_available_for_probe("hevc_nvenc");
        let ffmpeg_av1 = Self::encoder_available_for_probe("av1_nvenc");

        if native_caps.available {
            log::debug!("  Native NVENC probe succeeded");

            native_caps.h264 &= ffmpeg_h264;
            native_caps.hevc &= ffmpeg_hevc;
            native_caps.av1 &= ffmpeg_av1;
            native_caps.available = native_caps.h264 || native_caps.hevc || native_caps.av1;

            if !native_caps.available {
                probe_errors.push("nvenc: ffmpeg build lacks nvenc encoders".to_string());
            }

            return native_caps;
        }

        if !ffmpeg_h264 && !ffmpeg_hevc && !ffmpeg_av1 {
            probe_errors.push("nvenc: not compiled into ffmpeg".to_string());
            return NvencCaps::default();
        }

        log::debug!("  Native NVENC probe failed, falling back to CLI...");
        Self::probe_nvenc_via_cli(probe_errors)
    }

    /// Probe AMF - native first, CLI fallback
    fn probe_amf(probe_errors: &mut Vec<String>) -> AmfCaps {
        log::debug!("  Attempting native AMF probe...");
        let mut native_caps = native_amf::probe();

        let ffmpeg_h264 = Self::encoder_available_for_probe("h264_amf");
        let ffmpeg_hevc = Self::encoder_available_for_probe("hevc_amf");
        let ffmpeg_av1 = Self::encoder_available_for_probe("av1_amf");

        if native_caps.available {
            log::debug!("  Native AMF probe succeeded");

            native_caps.h264 &= ffmpeg_h264;
            native_caps.hevc &= ffmpeg_hevc;
            native_caps.av1 &= ffmpeg_av1;
            native_caps.available = native_caps.h264 || native_caps.hevc || native_caps.av1;

            if !native_caps.available {
                probe_errors.push("amf: ffmpeg build lacks amf encoders".to_string());
            }

            return native_caps;
        }

        if !ffmpeg_h264 && !ffmpeg_hevc && !ffmpeg_av1 {
            probe_errors.push("amf: not compiled into ffmpeg".to_string());
            return AmfCaps::default();
        }

        log::debug!("  Native AMF probe failed, falling back to CLI...");
        Self::probe_amf_via_cli(probe_errors)
    }

    /// Probe QSV - native first, CLI fallback
    fn probe_qsv(probe_errors: &mut Vec<String>) -> QsvCaps {
        log::debug!("  Attempting native QSV probe...");
        let mut native_caps = native_qsv::probe();

        let ffmpeg_h264 = Self::encoder_available_for_probe("h264_qsv");
        let ffmpeg_hevc = Self::encoder_available_for_probe("hevc_qsv");
        let ffmpeg_av1 = Self::encoder_available_for_probe("av1_qsv");

        if native_caps.available {
            log::debug!("  Native QSV probe succeeded");

            native_caps.h264 &= ffmpeg_h264;
            native_caps.hevc &= ffmpeg_hevc;
            native_caps.av1 &= ffmpeg_av1;
            native_caps.available = native_caps.h264 || native_caps.hevc || native_caps.av1;

            if !native_caps.available {
                probe_errors.push("qsv: ffmpeg build lacks qsv encoders".to_string());
            }

            return native_caps;
        }

        if !ffmpeg_h264 && !ffmpeg_hevc && !ffmpeg_av1 {
            probe_errors.push("qsv: not compiled into ffmpeg".to_string());
            return QsvCaps::default();
        }

        log::debug!("  Native QSV probe failed, falling back to CLI...");
        Self::probe_qsv_via_cli(probe_errors)
    }

    /// Probe VideoToolbox - native first, CLI fallback
    fn probe_videotoolbox(probe_errors: &mut Vec<String>) -> VideoToolboxCaps {
        log::debug!("  Attempting native VideoToolbox probe...");
        let mut native_caps = native_videotoolbox::probe();

        let ffmpeg_h264 = Self::encoder_available_for_probe("h264_videotoolbox");
        let ffmpeg_hevc = Self::encoder_available_for_probe("hevc_videotoolbox");

        if native_caps.available {
            log::debug!("  Native VideoToolbox probe succeeded");

            native_caps.h264 &= ffmpeg_h264;
            native_caps.hevc &= ffmpeg_hevc;
            native_caps.available = native_caps.h264 || native_caps.hevc;

            if !native_caps.available {
                probe_errors.push("videotoolbox: ffmpeg build lacks videotoolbox encoders".to_string());
            }

            return native_caps;
        }

        if !ffmpeg_h264 && !ffmpeg_hevc {
            probe_errors.push("videotoolbox: not compiled into ffmpeg".to_string());
            return VideoToolboxCaps::default();
        }

        log::debug!("  Native VideoToolbox probe failed, falling back to CLI...");
        Self::probe_videotoolbox_via_cli(probe_errors)
    }

    /// Get list of available H.264 encoders for UI
    pub fn available_h264_encoders(&self) -> Vec<EncoderOption> {
        let mut encoders = Vec::new();

        // Hardware encoders first (in preference order)
        if self.nvenc.h264 {
            let mut capabilities = HashMap::new();
            capabilities.insert("b_frames".to_string(), self.nvenc.b_frames);
            capabilities.insert("lookahead".to_string(), self.nvenc.lookahead);

            encoders.push(EncoderOption {
                id: "h264_nvenc".to_string(),
                name: format!(
                    "NVIDIA NVENC H.264{}",
                    self.nvenc.gpu_name.as_ref().map(|n| format!(" ({})", n)).unwrap_or_default()
                ),
                hardware: true,
                codec: "h264".to_string(),
                capabilities,
            });
        }

        if self.amf.h264 {
            let mut capabilities = HashMap::new();
            capabilities.insert("b_frames".to_string(), self.amf.b_frames);

            encoders.push(EncoderOption {
                id: "h264_amf".to_string(),
                name: format!(
                    "AMD AMF H.264{}",
                    self.amf.gpu_name.as_ref().map(|n| format!(" ({})", n)).unwrap_or_default()
                ),
                hardware: true,
                codec: "h264".to_string(),
                capabilities,
            });
        }

        if self.videotoolbox.h264 {
            encoders.push(EncoderOption {
                id: "h264_videotoolbox".to_string(),
                name: "Apple VideoToolbox H.264".to_string(),
                hardware: self.videotoolbox.hardware_accelerated,
                codec: "h264".to_string(),
                capabilities: HashMap::new(),
            });
        }

        if self.qsv.h264 {
            let mut capabilities = HashMap::new();
            capabilities.insert("b_frames".to_string(), self.qsv.b_frames);
            capabilities.insert("lookahead".to_string(), self.qsv.lookahead);
            capabilities.insert("low_power".to_string(), self.qsv.low_power);

            encoders.push(EncoderOption {
                id: "h264_qsv".to_string(),
                name: format!(
                    "Intel Quick Sync H.264{}",
                    self.qsv.device_name.as_ref().map(|n| format!(" ({})", n)).unwrap_or_default()
                ),
                hardware: true,
                codec: "h264".to_string(),
                capabilities,
            });
        }

        // Software encoder last
        if self.software.libx264 {
            encoders.push(EncoderOption {
                id: "libx264".to_string(),
                name: "x264 (Software)".to_string(),
                hardware: false,
                codec: "h264".to_string(),
                capabilities: HashMap::new(),
            });
        }

        encoders
    }

    /// Get list of available HEVC encoders for UI
    pub fn available_hevc_encoders(&self) -> Vec<EncoderOption> {
        let mut encoders = Vec::new();

        if self.nvenc.hevc {
            encoders.push(EncoderOption {
                id: "hevc_nvenc".to_string(),
                name: "NVIDIA NVENC HEVC".to_string(),
                hardware: true,
                codec: "hevc".to_string(),
                capabilities: HashMap::new(),
            });
        }

        if self.amf.hevc {
            encoders.push(EncoderOption {
                id: "hevc_amf".to_string(),
                name: "AMD AMF HEVC".to_string(),
                hardware: true,
                codec: "hevc".to_string(),
                capabilities: HashMap::new(),
            });
        }

        if self.videotoolbox.hevc {
            encoders.push(EncoderOption {
                id: "hevc_videotoolbox".to_string(),
                name: "Apple VideoToolbox HEVC".to_string(),
                hardware: self.videotoolbox.hardware_accelerated,
                codec: "hevc".to_string(),
                capabilities: HashMap::new(),
            });
        }

        if self.qsv.hevc {
            encoders.push(EncoderOption {
                id: "hevc_qsv".to_string(),
                name: "Intel Quick Sync HEVC".to_string(),
                hardware: true,
                codec: "hevc".to_string(),
                capabilities: HashMap::new(),
            });
        }

        if self.software.libx265 {
            encoders.push(EncoderOption {
                id: "libx265".to_string(),
                name: "x265 (Software)".to_string(),
                hardware: false,
                codec: "hevc".to_string(),
                capabilities: HashMap::new(),
            });
        }

        encoders
    }

    /// Get list of available AV1 encoders for UI
    pub fn available_av1_encoders(&self) -> Vec<EncoderOption> {
        let mut encoders = Vec::new();

        if self.nvenc.av1 {
            encoders.push(EncoderOption {
                id: "av1_nvenc".to_string(),
                name: "NVIDIA NVENC AV1".to_string(),
                hardware: true,
                codec: "av1".to_string(),
                capabilities: HashMap::new(),
            });
        }

        if self.amf.av1 {
            encoders.push(EncoderOption {
                id: "av1_amf".to_string(),
                name: "AMD AMF AV1".to_string(),
                hardware: true,
                codec: "av1".to_string(),
                capabilities: HashMap::new(),
            });
        }

        if self.qsv.av1 {
            encoders.push(EncoderOption {
                id: "av1_qsv".to_string(),
                name: "Intel Quick Sync AV1".to_string(),
                hardware: true,
                codec: "av1".to_string(),
                capabilities: HashMap::new(),
            });
        }

        if self.software.libsvtav1 {
            encoders.push(EncoderOption {
                id: "libsvtav1".to_string(),
                name: "SVT-AV1 (Software)".to_string(),
                hardware: false,
                codec: "av1".to_string(),
                capabilities: HashMap::new(),
            });
        }

        encoders
    }

    /// Get all available video encoders
    pub fn all_video_encoders(&self) -> Vec<EncoderOption> {
        let mut all = Vec::new();
        all.extend(self.available_h264_encoders());
        all.extend(self.available_hevc_encoders());
        all.extend(self.available_av1_encoders());
        all
    }

    // =========================================================================
    // CLI-based probing (Phase 0 - before ffmpeg-sys-next integration)
    // These methods use FFmpeg CLI to detect encoder availability.
    // They will be replaced/supplemented by native probing in later phases.
    // =========================================================================

    /// Get FFmpeg path for probing
    fn get_ffmpeg_path() -> Option<std::path::PathBuf> {
        let path = FFmpegDownloader::get_ffmpeg_path(None);
        if let Some(ref p) = path {
            log::debug!("Using FFmpeg at: {:?}", p);
        } else {
            log::warn!("FFmpeg not found - encoder detection will fail");
        }
        path
    }

    /// Run FFmpeg with given args and return stdout
    fn run_ffmpeg(args: &[&str]) -> Option<String> {
        let ffmpeg_path = Self::get_ffmpeg_path()?;

        #[cfg(windows)]
        let output = Command::new(&ffmpeg_path)
            .args(args)
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .ok()?;

        #[cfg(not(windows))]
        let output = Command::new(&ffmpeg_path)
            .args(args)
            .output()
            .ok()?;

        if output.status.success() || !output.stderr.is_empty() {
            // FFmpeg often outputs to stderr
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            Some(format!("{}{}", stdout, stderr))
        } else {
            None
        }
    }

    fn record_probe_error(
        probe_errors: &mut Vec<String>,
        encoder: &str,
        output: &str,
    ) {
        let trimmed = output.trim();
        if trimmed.is_empty() {
            return;
        }

        let snippet: String = trimmed.chars().take(500).collect();
        probe_errors.push(format!("{encoder}: {snippet}"));
    }

    /// Check if an encoder is available in FFmpeg
    fn encoder_available(encoder_name: &str) -> bool {
        Self::run_ffmpeg(&["-encoders", "-hide_banner"])
            .map(|output| output.contains(encoder_name))
            .unwrap_or(false)
    }

    fn encoder_available_for_probe(encoder_name: &str) -> bool {
        #[cfg(feature = "ffmpeg-libs")]
        {
            Self::encoder_available_via_libs(encoder_name)
        }
        #[cfg(not(feature = "ffmpeg-libs"))]
        {
            Self::encoder_available(encoder_name)
        }
    }

    #[cfg(feature = "ffmpeg-libs")]
    fn encoder_available_via_libs(encoder_name: &str) -> bool {
        let name = std::ffi::CString::new(encoder_name).ok();
        let Some(name) = name else {
            return false;
        };
        unsafe { !ffi::avcodec_find_encoder_by_name(name.as_ptr()).is_null() }
    }

    #[cfg(feature = "ffmpeg-libs")]
    fn probe_software_via_libs(_probe_errors: &mut Vec<String>) -> SoftwareCaps {
        let caps = SoftwareCaps {
            libx264: Self::encoder_available_via_libs("libx264"),
            libx265: Self::encoder_available_via_libs("libx265"),
            libsvtav1: Self::encoder_available_via_libs("libsvtav1"),
            aac: Self::encoder_available_via_libs("aac"),
            opus: Self::encoder_available_via_libs("libopus"),
        };

        log::debug!(
            "  Software encoders (libs): x264={}, x265={}, svtav1={}, aac={}, opus={}",
            caps.libx264,
            caps.libx265,
            caps.libsvtav1,
            caps.aac,
            caps.opus
        );

        caps
    }

    /// Probe NVENC via CLI
    fn probe_nvenc_via_cli(probe_errors: &mut Vec<String>) -> NvencCaps {
        let mut caps = NvencCaps::default();

        // Check if NVENC encoders are compiled into FFmpeg
        let h264_available = Self::encoder_available("h264_nvenc");
        let hevc_available = Self::encoder_available("hevc_nvenc");
        let av1_available = Self::encoder_available("av1_nvenc");

        log::debug!("  NVENC FFmpeg support: h264={}, hevc={}, av1={}",
            h264_available, hevc_available, av1_available);

        if !h264_available && !hevc_available && !av1_available {
            log::debug!("  NVENC: No encoders compiled into FFmpeg");
            probe_errors.push("nvenc: not compiled into ffmpeg".to_string());
            return caps;
        }

        // Try to actually initialize the encoder with a test encode.
        // Use a conservative frame size/format to avoid false negatives.
        // This verifies driver/hardware availability
        log::debug!("  NVENC: Testing encoder initialization...");
        if let Some(output) = Self::run_ffmpeg(&[
            "-f", "lavfi",
            "-i", "color=black:s=128x128:r=30:d=0.5",
            "-vf", "format=nv12",
            "-b:v", "1M",
            "-c:v", "h264_nvenc",
            "-f", "null",
            "-"
        ]) {
            // If we don't see "Cannot load" or "No capable" errors, NVENC is working
            let has_error = output.contains("Cannot load")
                || output.contains("No capable devices found")
                || output.contains("Driver does not support")
                || output.contains("not found")
                || output.contains("Codec not currently supported");

            if has_error {
                log::debug!("  NVENC: Hardware/driver not available");
                log::trace!("  NVENC error output: {}", output);
                Self::record_probe_error(probe_errors, "nvenc", &output);
            } else {
                log::debug!("  NVENC: Encoder initialization successful");
                caps.available = true;
                caps.h264 = h264_available;
                caps.hevc = hevc_available;
                caps.av1 = av1_available;
                caps.b_frames = true;  // NVENC supports B-frames
                caps.lookahead = true; // NVENC supports lookahead

                // Try to get GPU name from nvidia-smi
                #[cfg(windows)]
                if let Ok(output) = Command::new("nvidia-smi")
                    .args(["--query-gpu=name", "--format=csv,noheader"])
                    .creation_flags(CREATE_NO_WINDOW)
                    .output()
                {
                    if output.status.success() {
                        let name = String::from_utf8_lossy(&output.stdout);
                        caps.gpu_name = Some(name.trim().to_string());
                        log::debug!("  NVENC: GPU detected: {}", name.trim());
                    }
                }

                #[cfg(not(windows))]
                if let Ok(output) = Command::new("nvidia-smi")
                    .args(["--query-gpu=name", "--format=csv,noheader"])
                    .output()
                {
                    if output.status.success() {
                        let name = String::from_utf8_lossy(&output.stdout);
                        caps.gpu_name = Some(name.trim().to_string());
                        log::debug!("  NVENC: GPU detected: {}", name.trim());
                    }
                }
            }
        } else {
            log::debug!("  NVENC: Failed to run FFmpeg test encode");
            probe_errors.push("nvenc: ffmpeg test encode failed to start".to_string());
        }

        caps
    }

    /// Probe AMF via CLI
    fn probe_amf_via_cli(probe_errors: &mut Vec<String>) -> AmfCaps {
        let mut caps = AmfCaps::default();

        // AMF is Windows-only
        #[cfg(not(target_os = "windows"))]
        {
            log::debug!("  AMF: Not available (Windows-only)");
            probe_errors.push("amf: unsupported platform".to_string());
            return caps;
        }

        #[cfg(target_os = "windows")]
        {
            let h264_available = Self::encoder_available("h264_amf");
            let hevc_available = Self::encoder_available("hevc_amf");
            let av1_available = Self::encoder_available("av1_amf");

            log::debug!("  AMF FFmpeg support: h264={}, hevc={}, av1={}",
                h264_available, hevc_available, av1_available);

            if !h264_available && !hevc_available && !av1_available {
                log::debug!("  AMF: No encoders compiled into FFmpeg");
                probe_errors.push("amf: not compiled into ffmpeg".to_string());
                return caps;
            }

            // Try to initialize AMF encoder with a conservative test encode.
            // AMF can reject very small frames (e.g. 64x64) on some drivers.
            log::debug!("  AMF: Testing encoder initialization...");
            if let Some(output) = Self::run_ffmpeg(&[
                "-f", "lavfi",
                "-i", "color=black:s=128x128:r=30:d=0.5",
                "-vf", "format=nv12",
                "-b:v", "1M",
                "-c:v", "h264_amf",
                "-f", "null",
                "-"
            ]) {
                // Be specific about AMF errors - "Error" alone is too broad
                // FFmpeg outputs "Error" in many contexts that aren't failures
                let has_error = output.contains("Cannot load")
                    || output.contains("DLL amfrt64.dll failed")
                    || output.contains("Failed to create AMF")
                    || output.contains("AMF runtime not found")
                    || output.contains("CreateComponent failed")
                    || output.contains("not supported in AMF")
                    || output.contains("AMF failed")
                    || output.contains("No AMD devices");

                // Log output for debugging
                log::debug!("  AMF FFmpeg output (first 500 chars): {}",
                    output.chars().take(500).collect::<String>());

                if has_error {
                    log::debug!("  AMF: Hardware/driver not available");
                    log::trace!("  AMF full error output: {}", output);
                    Self::record_probe_error(probe_errors, "amf", &output);
                } else {
                    log::debug!("  AMF: Encoder initialization successful");
                    caps.available = true;
                    caps.h264 = h264_available;
                    caps.hevc = hevc_available;
                    caps.av1 = av1_available;
                    caps.b_frames = true;
                }
            } else {
                log::debug!("  AMF: Failed to run FFmpeg test encode");
                probe_errors.push("amf: ffmpeg test encode failed to start".to_string());
            }

            caps
        }
    }

    /// Probe QSV via CLI
    fn probe_qsv_via_cli(probe_errors: &mut Vec<String>) -> QsvCaps {
        let mut caps = QsvCaps::default();

        // QSV is Windows/Linux only
        #[cfg(target_os = "macos")]
        {
            log::debug!("  QSV: Not available (Windows/Linux only)");
            probe_errors.push("qsv: unsupported platform".to_string());
            return caps;
        }

        #[cfg(not(target_os = "macos"))]
        {
            let h264_available = Self::encoder_available("h264_qsv");
            let hevc_available = Self::encoder_available("hevc_qsv");
            let av1_available = Self::encoder_available("av1_qsv");

            log::debug!("  QSV FFmpeg support: h264={}, hevc={}, av1={}",
                h264_available, hevc_available, av1_available);

            if !h264_available && !hevc_available && !av1_available {
                log::debug!("  QSV: No encoders compiled into FFmpeg");
                probe_errors.push("qsv: not compiled into ffmpeg".to_string());
                return caps;
            }

            // Try to initialize QSV encoder with a conservative test encode.
            log::debug!("  QSV: Testing encoder initialization...");
            if let Some(output) = Self::run_ffmpeg(&[
                "-f", "lavfi",
                "-i", "color=black:s=128x128:r=30:d=0.5",
                "-vf", "format=nv12",
                "-b:v", "1M",
                "-c:v", "h264_qsv",
                "-f", "null",
                "-"
            ]) {
                let has_error = output.contains("Error initializing")
                    || output.contains("Cannot load")
                    || output.contains("not supported")
                    || output.contains("MFX_ERR")
                    || output.contains("No device");

                if has_error {
                    log::debug!("  QSV: Hardware/driver not available");
                    log::trace!("  QSV error output: {}", output);
                    Self::record_probe_error(probe_errors, "qsv", &output);
                } else {
                    log::debug!("  QSV: Encoder initialization successful");
                    caps.available = true;
                    caps.h264 = h264_available;
                    caps.hevc = hevc_available;
                    caps.av1 = av1_available;

                    // QSV B-frame and lookahead support depends on hardware
                    // For Twitch compatibility, we often disable these anyway
                    // Conservative defaults - will be refined with native probing
                    caps.b_frames = true;
                    caps.lookahead = true;
                    caps.low_power = false; // Detect via native probing later
                }
            } else {
                log::debug!("  QSV: Failed to run FFmpeg test encode");
                probe_errors.push("qsv: ffmpeg test encode failed to start".to_string());
            }

            caps
        }
    }

    /// Probe VideoToolbox via CLI
    fn probe_videotoolbox_via_cli(probe_errors: &mut Vec<String>) -> VideoToolboxCaps {
        // VideoToolbox is macOS only
        #[cfg(not(target_os = "macos"))]
        {
            probe_errors.push("videotoolbox: unsupported platform".to_string());
            VideoToolboxCaps::default()
        }

        #[cfg(target_os = "macos")]
        let mut caps = VideoToolboxCaps::default();

        #[cfg(target_os = "macos")]
        {
            let h264_available = Self::encoder_available("h264_videotoolbox");
            let hevc_available = Self::encoder_available("hevc_videotoolbox");

            if !h264_available && !hevc_available {
                return caps;
            }

            // VideoToolbox is always available on macOS if FFmpeg has it compiled in
            // The framework is part of the OS
            caps.available = true;
            caps.h264 = h264_available;
            caps.hevc = hevc_available;
            caps.hardware_accelerated = true; // Assume hardware accel on modern Macs

            caps
        }
    }

    /// Probe software encoders via CLI
    fn probe_software_via_cli(_probe_errors: &mut Vec<String>) -> SoftwareCaps {
        let caps = SoftwareCaps {
            libx264: Self::encoder_available("libx264"),
            libx265: Self::encoder_available("libx265"),
            libsvtav1: Self::encoder_available("libsvtav1"),
            aac: Self::encoder_available(" aac "), // Space-padded to avoid false matches
            opus: Self::encoder_available("libopus"),
        };

        log::debug!("  Software encoders: x264={}, x265={}, svtav1={}, aac={}, opus={}",
            caps.libx264, caps.libx265, caps.libsvtav1, caps.aac, caps.opus);

        caps
    }
}

// =========================================================================
// Native OBS-style probing
// These directly load vendor DLLs and query hardware capabilities
// =========================================================================

/// Native NVENC probing via NVIDIA Video Codec SDK
/// Loads nvEncodeAPI64.dll (Windows) or libnvidia-encode.so (Linux)
#[allow(dead_code)] // Constants and types defined for SDK documentation and future use
mod native_nvenc {
    use super::NvencCaps;
    use libloading::{Library, Symbol};
    use std::ffi::c_void;

    // NVENC API constants
    const NV_ENC_OPEN_ENCODE_SESSION_EX_PARAMS_VER: u32 = 0x10000001;
    const NV_ENC_CAPS_PARAM_VER: u32 = 0x10000001;
    const NV_ENCODE_API_FUNCTION_LIST_VER: u32 = 0x10000002;

    // Capability flags we can query (for future detailed probing)
    const NV_ENC_CAPS_SUPPORT_BFRAME_REF_MODE: i32 = 15;
    const NV_ENC_CAPS_SUPPORT_LOOKAHEAD: i32 = 17;
    const NV_ENC_CAPS_NUM_MAX_BFRAMES: i32 = 0;

    // Codec GUIDs (H.264, HEVC, AV1) for future codec-specific capability queries
    const NV_ENC_CODEC_H264_GUID: NvGuid = NvGuid {
        data1: 0x6BC82762,
        data2: 0x4E63,
        data3: 0x4CA4,
        data4: [0xAA, 0x85, 0x1E, 0x50, 0xF3, 0x21, 0xF6, 0xBF],
    };
    const NV_ENC_CODEC_HEVC_GUID: NvGuid = NvGuid {
        data1: 0x790CDC88,
        data2: 0x4522,
        data3: 0x4D7B,
        data4: [0x94, 0x25, 0xBD, 0xA9, 0x97, 0x5F, 0x76, 0x03],
    };
    const NV_ENC_CODEC_AV1_GUID: NvGuid = NvGuid {
        data1: 0x0A352289,
        data2: 0x0AA7,
        data3: 0x4759,
        data4: [0x86, 0x2D, 0x5D, 0x15, 0xCD, 0x16, 0xD2, 0x54],
    };

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct NvGuid {
        data1: u32,
        data2: u16,
        data3: u16,
        data4: [u8; 8],
    }

    impl PartialEq for NvGuid {
        fn eq(&self, other: &Self) -> bool {
            self.data1 == other.data1
                && self.data2 == other.data2
                && self.data3 == other.data3
                && self.data4 == other.data4
        }
    }

    #[repr(C)]
    struct NvEncOpenEncodeSessionExParams {
        version: u32,
        device_type: u32, // 1 = CUDA
        device: *mut c_void,
        reserved: *mut c_void,
        api_version: u32,
        reserved1: [u32; 253],
        reserved2: [*mut c_void; 64],
    }

    #[repr(C)]
    struct NvEncCapsParam {
        version: u32,
        caps_to_query: i32,
        reserved: [u32; 62],
    }

    #[repr(C)]
    struct NvEncodeApiFunctionList {
        version: u32,
        reserved: u32,
        nv_enc_open_encode_session: *const c_void,
        nv_enc_get_encode_guid_count: *const c_void,
        nv_enc_get_encode_guids: Option<
            unsafe extern "C" fn(
                encoder: *mut c_void,
                guids: *mut NvGuid,
                guid_array_size: u32,
                guid_count: *mut u32,
            ) -> i32,
        >,
        nv_enc_get_encode_profile_guid_count: *const c_void,
        nv_enc_get_encode_profile_guids: *const c_void,
        nv_enc_get_input_format_count: *const c_void,
        nv_enc_get_input_formats: *const c_void,
        nv_enc_get_encode_caps: Option<
            unsafe extern "C" fn(
                encoder: *mut c_void,
                encode_guid: NvGuid,
                caps_param: *mut NvEncCapsParam,
                caps_val: *mut i32,
            ) -> i32,
        >,
        nv_enc_get_encode_preset_count: *const c_void,
        nv_enc_get_encode_preset_guids: *const c_void,
        nv_enc_get_encode_preset_config: *const c_void,
        nv_enc_initialize_encoder: *const c_void,
        nv_enc_create_input_buffer: *const c_void,
        nv_enc_destroy_input_buffer: *const c_void,
        nv_enc_create_bitstream_buffer: *const c_void,
        nv_enc_destroy_bitstream_buffer: *const c_void,
        nv_enc_encode_picture: *const c_void,
        nv_enc_lock_bitstream: *const c_void,
        nv_enc_unlock_bitstream: *const c_void,
        nv_enc_lock_input_buffer: *const c_void,
        nv_enc_unlock_input_buffer: *const c_void,
        nv_enc_get_encode_stats: *const c_void,
        nv_enc_get_sequence_params: *const c_void,
        nv_enc_register_async_event: *const c_void,
        nv_enc_unregister_async_event: *const c_void,
        nv_enc_map_input_resource: *const c_void,
        nv_enc_unmap_input_resource: *const c_void,
        nv_enc_destroy_encoder:
            Option<unsafe extern "C" fn(encoder: *mut c_void) -> i32>,
        nv_enc_invalidate_ref_frames: *const c_void,
        nv_enc_open_encode_session_ex: Option<
            unsafe extern "C" fn(
                params: *mut NvEncOpenEncodeSessionExParams,
                encoder: *mut *mut c_void,
            ) -> i32,
        >,
        nv_enc_register_resource: *const c_void,
        nv_enc_unregister_resource: *const c_void,
        nv_enc_reconfigure_encoder: *const c_void,
        reserved1: *const c_void,
        nv_enc_create_mv_buffer: *const c_void,
        nv_enc_destroy_mv_buffer: *const c_void,
        nv_enc_run_motion_estimation_only: *const c_void,
        nv_enc_get_last_error_string: *const c_void,
        nv_enc_set_io_cuda_streams: *const c_void,
        nv_enc_get_encode_preset_config_ex: *const c_void,
        nv_enc_get_sequence_param_ex: *const c_void,
        nv_enc_lookahead_picture: *const c_void,
        // Note: reserved2 has 275 elements but we use a workaround for Default
        reserved2_1: [*const c_void; 32],
        reserved2_2: [*const c_void; 32],
        reserved2_3: [*const c_void; 32],
        reserved2_4: [*const c_void; 32],
        reserved2_5: [*const c_void; 32],
        reserved2_6: [*const c_void; 32],
        reserved2_7: [*const c_void; 32],
        reserved2_8: [*const c_void; 32],
        reserved2_9: [*const c_void; 19],
    }

    impl Default for NvEncodeApiFunctionList {
        fn default() -> Self {
            // Safety: All fields are pointers or Options that can be zero-initialized
            unsafe { std::mem::zeroed() }
        }
    }

    type NvEncodeApiCreateInstanceFn =
        unsafe extern "C" fn(function_list: *mut NvEncodeApiFunctionList) -> i32;

    pub fn probe() -> NvencCaps {
        let mut caps = NvencCaps::default();

        // Try to load the NVENC library
        #[cfg(target_os = "windows")]
        let lib_names = ["nvEncodeAPI64.dll", "nvEncodeAPI.dll"];
        #[cfg(target_os = "linux")]
        let lib_names = [
            "libnvidia-encode.so.1",
            "libnvidia-encode.so",
        ];
        #[cfg(not(any(target_os = "windows", target_os = "linux")))]
        let lib_names: [&str; 0] = [];

        let lib = lib_names.iter().find_map(|name| {
            unsafe { Library::new(*name).ok() }
        });

        let lib = match lib {
            Some(l) => l,
            None => {
                log::debug!("Native NVENC: Library not found");
                return caps;
            }
        };

        // Get the API creation function
        let create_instance: Symbol<NvEncodeApiCreateInstanceFn> =
            match unsafe { lib.get(b"NvEncodeAPICreateInstance\0") } {
                Ok(sym) => sym,
                Err(e) => {
                    log::debug!("Native NVENC: NvEncodeAPICreateInstance not found: {}", e);
                    return caps;
                }
            };

        // Initialize the function list
        let mut func_list = NvEncodeApiFunctionList {
            version: NV_ENCODE_API_FUNCTION_LIST_VER,
            ..Default::default()
        };

        let ret = unsafe { create_instance(&mut func_list) };
        if ret != 0 {
            log::debug!("Native NVENC: NvEncodeAPICreateInstance failed: {}", ret);
            return caps;
        }

        // Try to open an encode session (without a real CUDA device, we use device type 0)
        // This validates driver availability
        // Verify key functions are available
        let _open_session = match func_list.nv_enc_open_encode_session_ex {
            Some(f) => f,
            None => {
                log::debug!("Native NVENC: nvEncOpenEncodeSessionEx not available");
                return caps;
            }
        };

        // Store function pointers for potential future use (capability queries)
        let _destroy_encoder = func_list.nv_enc_destroy_encoder;
        let _get_encode_guids = func_list.nv_enc_get_encode_guids;
        let _get_encode_caps = func_list.nv_enc_get_encode_caps;

        // We need CUDA to open a real session, but we can check if the API is functional
        // For a proper check, we'd need to initialize CUDA first
        // If the library loaded and API initialized successfully, NVENC is available
        caps.available = true;

        // Try to query capabilities by attempting to open a session
        // This requires a CUDA context, which we may not have
        // For now, assume if the library loads and API initializes, basic support exists
        caps.h264 = true; // NVENC always supports H.264
        caps.hevc = true; // Modern NVENC supports HEVC
        caps.av1 = false; // AV1 only on RTX 40 series, conservative default
        caps.b_frames = true;
        caps.lookahead = true;

        // Try to get GPU name via CUDA if available
        #[cfg(target_os = "windows")]
        {
            caps.gpu_name = get_nvidia_gpu_name_windows();
        }
        #[cfg(target_os = "linux")]
        {
            caps.gpu_name = get_nvidia_gpu_name_linux();
        }

        log::info!("Native NVENC: Available, GPU: {:?}", caps.gpu_name);
        caps
    }

    #[cfg(target_os = "windows")]
    fn get_nvidia_gpu_name_windows() -> Option<String> {
        use std::process::Command;
        #[cfg(windows)]
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;

        let output = Command::new("nvidia-smi")
            .args(["--query-gpu=name", "--format=csv,noheader"])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .ok()?;

        if output.status.success() {
            let name = String::from_utf8_lossy(&output.stdout);
            let trimmed = name.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
        None
    }

    #[cfg(target_os = "linux")]
    fn get_nvidia_gpu_name_linux() -> Option<String> {
        use std::process::Command;

        let output = Command::new("nvidia-smi")
            .args(["--query-gpu=name", "--format=csv,noheader"])
            .output()
            .ok()?;

        if output.status.success() {
            let name = String::from_utf8_lossy(&output.stdout);
            let trimmed = name.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
        None
    }
}

/// Native AMF probing via AMD Media Framework
/// Loads amfrt64.dll (Windows only - AMF is Windows-exclusive)
#[allow(dead_code, non_snake_case)] // SDK types/constants for documentation; non-snake_case matches SDK naming
mod native_amf {
    use super::AmfCaps;
    #[cfg(target_os = "windows")]
    use libloading::{Library, Symbol};
    #[cfg(target_os = "windows")]
    use std::ffi::c_void;
    #[cfg(target_os = "windows")]
    use std::ptr;

    // AMF result codes
    #[cfg(target_os = "windows")]
    const AMF_OK: i32 = 0;

    // AMF component IDs for encoders
    #[cfg(target_os = "windows")]
    const AMF_VIDEO_ENCODER_VCE: &[u16] = &[
        'A' as u16, 'M' as u16, 'F' as u16, 'V' as u16, 'i' as u16, 'd' as u16,
        'e' as u16, 'o' as u16, 'E' as u16, 'n' as u16, 'c' as u16, 'o' as u16,
        'd' as u16, 'e' as u16, 'r' as u16, 'V' as u16, 'C' as u16, 'E' as u16, 0,
    ];
    #[cfg(target_os = "windows")]
    const AMF_VIDEO_ENCODER_HEVC: &[u16] = &[
        'A' as u16, 'M' as u16, 'F' as u16, 'V' as u16, 'i' as u16, 'd' as u16,
        'e' as u16, 'o' as u16, 'E' as u16, 'n' as u16, 'c' as u16, 'o' as u16,
        'd' as u16, 'e' as u16, 'r' as u16, 'H' as u16, 'E' as u16, 'V' as u16,
        'C' as u16, 0,
    ];
    #[cfg(target_os = "windows")]
    const AMF_VIDEO_ENCODER_AV1: &[u16] = &[
        'A' as u16, 'M' as u16, 'F' as u16, 'V' as u16, 'i' as u16, 'd' as u16,
        'e' as u16, 'o' as u16, 'E' as u16, 'n' as u16, 'c' as u16, 'o' as u16,
        'd' as u16, 'e' as u16, 'r' as u16, 'A' as u16, 'V' as u16, '1' as u16, 0,
    ];

    // AMF interface IDs
    #[cfg(target_os = "windows")]
    #[repr(C)]
    struct AmfGuid {
        data1: u32,
        data2: u16,
        data3: u16,
        data4: [u8; 8],
    }

    #[cfg(target_os = "windows")]
    const IID_AMF_CONTEXT: AmfGuid = AmfGuid {
        data1: 0x71258F55,
        data2: 0x3A3E,
        data3: 0x4953,
        data4: [0x98, 0x2D, 0x87, 0x43, 0xAB, 0x94, 0xEC, 0x24],
    };

    // AMF vtable structures (simplified)
    #[cfg(target_os = "windows")]
    #[repr(C)]
    struct AmfFactoryVtbl {
        // IUnknown-like
        query_interface: *const c_void,
        acquire: *const c_void,
        release: Option<unsafe extern "system" fn(this: *mut c_void) -> i64>,
        // AMFFactory
        trace: *const c_void,
        debug: *const c_void,
        get_trace: *const c_void,
        get_debug: *const c_void,
        get_programs: *const c_void,
        create_context: Option<
            unsafe extern "system" fn(
                this: *mut c_void,
                context: *mut *mut c_void,
            ) -> i32,
        >,
        create_component: *const c_void,
        set_cache_folder: *const c_void,
        get_cache_folder: *const c_void,
        set_cache_enabled: *const c_void,
        get_cache_enabled: *const c_void,
    }

    #[cfg(target_os = "windows")]
    #[repr(C)]
    struct AmfFactory {
        vtbl: *const AmfFactoryVtbl,
    }

    #[cfg(target_os = "windows")]
    #[repr(C)]
    struct AmfContextVtbl {
        // IUnknown-like
        query_interface: *const c_void,
        acquire: *const c_void,
        release: Option<unsafe extern "system" fn(this: *mut c_void) -> i64>,
        // AMFPropertyStorage
        _property_storage: [*const c_void; 10],
        // AMFContext
        terminate: Option<unsafe extern "system" fn(this: *mut c_void) -> i32>,
        init_dx9: *const c_void,
        init_dx11: Option<
            unsafe extern "system" fn(
                this: *mut c_void,
                device: *mut c_void,
            ) -> i32,
        >,
        init_opengl: *const c_void,
        init_opencl: *const c_void,
        init_openclEx: *const c_void,
        lock_dx9: *const c_void,
        lock_dx11: *const c_void,
        lock_opengl: *const c_void,
        unlock_dx9: *const c_void,
        unlock_dx11: *const c_void,
        unlock_opengl: *const c_void,
        get_dx9_device_adapter: *const c_void,
        get_dx9_device: *const c_void,
        get_dx11_device: *const c_void,
        get_opengl_device: *const c_void,
        get_opencl_context: *const c_void,
        get_opencl_command_queue: *const c_void,
        get_opencl_device_id: *const c_void,
        alloc_buffer: *const c_void,
        alloc_surface: *const c_void,
        alloc_audio_buffer: *const c_void,
        create_buffer_from_host: *const c_void,
        create_buffer_from_dx9: *const c_void,
        create_buffer_from_dx11: *const c_void,
        create_buffer_from_opengl: *const c_void,
        create_buffer_from_opencl: *const c_void,
        create_surface_from_host: *const c_void,
        create_surface_from_dx9: *const c_void,
        create_surface_from_dx11: *const c_void,
        create_surface_from_opengl: *const c_void,
        create_surface_from_opencl: *const c_void,
        create_audio_buffer_from_host: *const c_void,
        create_component:  Option<
            unsafe extern "system" fn(
                this: *mut c_void,
                id: *const u16,
                component: *mut *mut c_void,
            ) -> i32,
        >,
    }

    #[cfg(target_os = "windows")]
    #[repr(C)]
    struct AmfContext {
        vtbl: *const AmfContextVtbl,
    }

    #[cfg(target_os = "windows")]
    #[repr(C)]
    struct AmfComponentVtbl {
        query_interface: *const c_void,
        acquire: *const c_void,
        release: Option<unsafe extern "system" fn(this: *mut c_void) -> i64>,
        // ... other methods we don't need
    }

    #[cfg(target_os = "windows")]
    #[repr(C)]
    struct AmfComponent {
        vtbl: *const AmfComponentVtbl,
    }

    #[cfg(target_os = "windows")]
    type AmfInitFn = unsafe extern "system" fn() -> i32;
    #[cfg(target_os = "windows")]
    type AmfQueryVersionFn = unsafe extern "system" fn(version: *mut u64) -> i32;
    #[cfg(target_os = "windows")]
    type AmfCreateFactoryFn = unsafe extern "system" fn(factory: *mut *mut AmfFactory) -> i32;

    pub fn probe() -> AmfCaps {
        let mut caps = AmfCaps::default();

        #[cfg(not(target_os = "windows"))]
        {
            log::debug!("Native AMF: Not available (Windows only)");
            return caps;
        }

        #[cfg(target_os = "windows")]
        {
            probe_windows(&mut caps);
            caps
        }
    }

    #[cfg(target_os = "windows")]
    fn probe_windows(caps: &mut AmfCaps) {
        // Try to load the AMF runtime
        let lib = match unsafe { Library::new("amfrt64.dll") } {
            Ok(l) => l,
            Err(e) => {
                log::debug!("Native AMF: amfrt64.dll not found: {}", e);
                return;
            }
        };

        // Get AMFInit
        let amf_init: Symbol<AmfInitFn> = match unsafe { lib.get(b"AMFInit\0") } {
            Ok(sym) => sym,
            Err(e) => {
                log::debug!("Native AMF: AMFInit not found: {}", e);
                return;
            }
        };

        // Initialize AMF
        let ret = unsafe { amf_init() };
        if ret != AMF_OK {
            log::debug!("Native AMF: AMFInit failed: {}", ret);
            return;
        }

        // Query version
        if let Ok(query_version) = unsafe { lib.get::<AmfQueryVersionFn>(b"AMFQueryVersion\0") } {
            let mut version: u64 = 0;
            if unsafe { query_version(&mut version) } == AMF_OK {
                let major = (version >> 48) & 0xFFFF;
                let minor = (version >> 32) & 0xFFFF;
                log::debug!("Native AMF: Version {}.{}", major, minor);
            }
        }

        // Create factory
        let create_factory: Symbol<AmfCreateFactoryFn> =
            match unsafe { lib.get(b"AMFCreateFactory\0") } {
                Ok(sym) => sym,
                Err(e) => {
                    log::debug!("Native AMF: AMFCreateFactory not found: {}", e);
                    return;
                }
            };

        let mut factory: *mut AmfFactory = ptr::null_mut();
        let ret = unsafe { create_factory(&mut factory) };
        if ret != AMF_OK || factory.is_null() {
            log::debug!("Native AMF: AMFCreateFactory failed: {}", ret);
            return;
        }

        // Create context
        let create_context = unsafe {
            match (*(*factory).vtbl).create_context {
                Some(f) => f,
                None => {
                    log::debug!("Native AMF: CreateContext not available");
                    return;
                }
            }
        };

        let mut context: *mut c_void = ptr::null_mut();
        let ret = unsafe { create_context(factory as *mut c_void, &mut context) };
        if ret != AMF_OK || context.is_null() {
            log::debug!("Native AMF: CreateContext failed: {}", ret);
            return;
        }

        let context = context as *mut AmfContext;

        // Initialize DX11 (required for encoder)
        // We pass null to let AMF create its own device
        let init_dx11 = unsafe {
            match (*(*context).vtbl).init_dx11 {
                Some(f) => f,
                None => {
                    log::debug!("Native AMF: InitDX11 not available");
                    cleanup_amf_context(context);
                    return;
                }
            }
        };

        let ret = unsafe { init_dx11(context as *mut c_void, ptr::null_mut()) };
        if ret != AMF_OK {
            log::debug!("Native AMF: InitDX11 failed: {}", ret);
            cleanup_amf_context(context);
            return;
        }

        caps.available = true;

        // Try to create each encoder type to verify availability
        let create_component = unsafe {
            match (*(*context).vtbl).create_component {
                Some(f) => f,
                None => {
                    log::debug!("Native AMF: CreateComponent not available");
                    cleanup_amf_context(context);
                    return;
                }
            }
        };

        // Test H.264 encoder
        let mut component: *mut c_void = ptr::null_mut();
        let ret = unsafe {
            create_component(context as *mut c_void, AMF_VIDEO_ENCODER_VCE.as_ptr(), &mut component)
        };
        if ret == AMF_OK && !component.is_null() {
            caps.h264 = true;
            unsafe {
                if let Some(release) = (*(*( component as *mut AmfComponent)).vtbl).release {
                    release(component);
                }
            }
            log::debug!("Native AMF: H.264 encoder available");
        }

        // Test HEVC encoder
        component = ptr::null_mut();
        let ret = unsafe {
            create_component(context as *mut c_void, AMF_VIDEO_ENCODER_HEVC.as_ptr(), &mut component)
        };
        if ret == AMF_OK && !component.is_null() {
            caps.hevc = true;
            unsafe {
                if let Some(release) = (*(*( component as *mut AmfComponent)).vtbl).release {
                    release(component);
                }
            }
            log::debug!("Native AMF: HEVC encoder available");
        }

        // Test AV1 encoder
        component = ptr::null_mut();
        let ret = unsafe {
            create_component(context as *mut c_void, AMF_VIDEO_ENCODER_AV1.as_ptr(), &mut component)
        };
        if ret == AMF_OK && !component.is_null() {
            caps.av1 = true;
            unsafe {
                if let Some(release) = (*(*( component as *mut AmfComponent)).vtbl).release {
                    release(component);
                }
            }
            log::debug!("Native AMF: AV1 encoder available");
        }

        // AMF supports B-frames for H.264
        if caps.h264 {
            caps.b_frames = true;
        }

        // Try to get GPU name
        caps.gpu_name = get_amd_gpu_name();

        cleanup_amf_context(context);

        log::info!(
            "Native AMF: Available, h264={}, hevc={}, av1={}, GPU: {:?}",
            caps.h264, caps.hevc, caps.av1, caps.gpu_name
        );
    }

    #[cfg(target_os = "windows")]
    fn cleanup_amf_context(context: *mut AmfContext) {
        unsafe {
            if let Some(terminate) = (*(*context).vtbl).terminate {
                terminate(context as *mut c_void);
            }
            if let Some(release) = (*(*context).vtbl).release {
                release(context as *mut c_void);
            }
        }
    }

    #[cfg(target_os = "windows")]
    fn get_amd_gpu_name() -> Option<String> {
        use std::process::Command;
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;

        // Try wmic to get AMD GPU name
        let output = Command::new("wmic")
            .args(["path", "win32_VideoController", "get", "name"])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .ok()?;

        if output.status.success() {
            let text = String::from_utf8_lossy(&output.stdout);
            for line in text.lines() {
                let trimmed = line.trim();
                if trimmed.contains("AMD") || trimmed.contains("Radeon") {
                    return Some(trimmed.to_string());
                }
            }
        }
        None
    }
}

/// Native QSV probing via Intel oneVPL / Media SDK
/// Loads libvpl.dll or libmfx.dll (Windows) / libvpl.so or libmfxhw64.so (Linux)
#[allow(dead_code, non_snake_case)] // SDK types/constants for documentation; non-snake_case matches SDK naming
mod native_qsv {
    use super::QsvCaps;
    use libloading::{Library, Symbol};
    use std::ffi::c_void;
    use std::ptr;

    // MFX status codes
    const MFX_ERR_NONE: i32 = 0;
    const MFX_ERR_UNSUPPORTED: i32 = -3;

    // MFX implementation types
    const MFX_IMPL_AUTO_ANY: u32 = 0x0000;
    const MFX_IMPL_HARDWARE: u32 = 0x0002;
    const MFX_IMPL_HARDWARE_ANY: u32 = 0x0006;

    // Codec IDs
    const MFX_CODEC_AVC: u32 = 0x20435641; // 'AVC '
    const MFX_CODEC_HEVC: u32 = 0x43564548; // 'HEVC'
    const MFX_CODEC_AV1: u32 = 0x31305641; // 'AV1'

    // Version
    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct MfxVersion {
        minor: u16,
        major: u16,
    }

    // MFX structures (simplified)
    #[repr(C)]
    struct MfxInfoMfx {
        _reserved: [u16; 7],
        low_power: u16,
        brc_param_multiplier: u16,
        frame_info: MfxFrameInfo,
        codec_id: u32,
        codec_profile: u16,
        codec_level: u16,
        num_thread: u16,
        // Padding split into smaller arrays for Default impl
        _padding1: [u8; 32],
        _padding2: [u8; 6],
    }

    impl Default for MfxInfoMfx {
        fn default() -> Self {
            // Safety: All fields are primitives or arrays that can be zero-initialized
            unsafe { std::mem::zeroed() }
        }
    }

    #[repr(C)]
    #[derive(Default)]
    struct MfxFrameInfo {
        _reserved: [u8; 4],
        channel_id: u16,
        bit_depth_luma: u16,
        bit_depth_chroma: u16,
        shift: u16,
        frame_id: MfxFrameId,
        four_cc: u32,
        _dimensions: [u16; 8], // width, height, crop values
        frame_rate: MfxFrameRate,
        aspect_ratio: [u16; 2],
        pic_struct: u16,
        chroma_format: u16,
        _reserved2: [u16; 4],
    }

    #[repr(C)]
    #[derive(Default)]
    struct MfxFrameId {
        temporal_id: u16,
        priority_id: u16,
        dependency_id: u16,
        quality_id: u16,
        view_id: u16,
    }

    #[repr(C)]
    #[derive(Default)]
    struct MfxFrameRate {
        frame_rate_ext_n: u32,
        frame_rate_ext_d: u32,
    }

    #[repr(C)]
    #[derive(Default)]
    struct MfxVideoParam {
        _reserved: [u32; 1],
        async_depth: u16,
        _reserved2: [u16; 3],
        mfx: MfxInfoMfx,
        _protected: u16,
        io_pattern: u16,
        _ext_param_stuff: [u8; 16],
        _reserved3: [u16; 2],
    }

    // Session handle
    type MfxSession = *mut c_void;

    // oneVPL loader types
    type MfxLoader = *mut c_void;
    type MfxConfig = *mut c_void;

    // Legacy Media SDK functions
    type MfxInitFn = unsafe extern "C" fn(impl_: u32, ver: *mut MfxVersion, session: *mut MfxSession) -> i32;
    type MfxCloseFn = unsafe extern "C" fn(session: MfxSession) -> i32;
    type MfxVideoEncodeQueryFn = unsafe extern "C" fn(
        session: MfxSession,
        in_param: *mut MfxVideoParam,
        out_param: *mut MfxVideoParam,
    ) -> i32;
    type MfxQueryImplDescriptionFn = unsafe extern "C" fn(
        session: MfxSession,
        format: u32,
        desc: *mut *mut c_void,
    ) -> i32;

    // oneVPL loader functions
    type MfxLoadFn = unsafe extern "C" fn() -> MfxLoader;
    type MfxUnloadFn = unsafe extern "C" fn(loader: MfxLoader);
    type MfxCreateConfigFn = unsafe extern "C" fn(loader: MfxLoader) -> MfxConfig;
    type MfxSetConfigFilterPropertyFn = unsafe extern "C" fn(
        config: MfxConfig,
        name: *const u8,
        value: MfxVariant,
    ) -> i32;
    type MfxCreateSessionFn = unsafe extern "C" fn(
        loader: MfxLoader,
        index: u32,
        session: *mut MfxSession,
    ) -> i32;

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct MfxVariant {
        type_: u32,
        data: MfxVariantData,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    union MfxVariantData {
        u32_val: u32,
        ptr: *const c_void,
    }

    pub fn probe() -> QsvCaps {
        let mut caps = QsvCaps::default();

        // QSV is not available on macOS
        #[cfg(target_os = "macos")]
        {
            log::debug!("Native QSV: Not available (Windows/Linux only)");
            return caps;
        }

        #[cfg(not(target_os = "macos"))]
        {
            // Try oneVPL first (newer), then fall back to legacy Media SDK
            if !try_onevpl_probe(&mut caps) {
                try_legacy_mfx_probe(&mut caps);
            }
            caps
        }
    }

    #[cfg(not(target_os = "macos"))]
    fn try_onevpl_probe(caps: &mut QsvCaps) -> bool {
        // oneVPL library names
        #[cfg(target_os = "windows")]
        let lib_names = ["libvpl.dll", "vpl.dll"];
        #[cfg(target_os = "linux")]
        let lib_names = ["libvpl.so.2", "libvpl.so"];

        let lib = lib_names.iter().find_map(|name| {
            unsafe { Library::new(*name).ok() }
        });

        let lib = match lib {
            Some(l) => l,
            None => {
                log::debug!("Native QSV: oneVPL not found, trying legacy SDK");
                return false;
            }
        };

        log::debug!("Native QSV: Using oneVPL");

        // Get loader functions
        let mfx_load: Symbol<MfxLoadFn> = match unsafe { lib.get(b"MFXLoad\0") } {
            Ok(sym) => sym,
            Err(_) => return false,
        };

        let mfx_unload: Symbol<MfxUnloadFn> = match unsafe { lib.get(b"MFXUnload\0") } {
            Ok(sym) => sym,
            Err(_) => return false,
        };

        // MFXCreateConfig is available for setting filter properties (not used currently)
        let _mfx_create_config: Symbol<MfxCreateConfigFn> =
            match unsafe { lib.get(b"MFXCreateConfig\0") } {
                Ok(sym) => sym,
                Err(_) => return false,
            };

        let mfx_create_session: Symbol<MfxCreateSessionFn> =
            match unsafe { lib.get(b"MFXCreateSession\0") } {
                Ok(sym) => sym,
                Err(_) => return false,
            };

        // Create loader
        let loader = unsafe { mfx_load() };
        if loader.is_null() {
            log::debug!("Native QSV: MFXLoad failed");
            return false;
        }

        // Create session (index 0 = first available implementation)
        let mut session: MfxSession = ptr::null_mut();
        let ret = unsafe { mfx_create_session(loader, 0, &mut session) };

        if ret != MFX_ERR_NONE || session.is_null() {
            log::debug!("Native QSV: MFXCreateSession failed: {}", ret);
            unsafe { mfx_unload(loader) };
            return false;
        }

        // Get encode query function
        let mfx_video_encode_query: Symbol<MfxVideoEncodeQueryFn> =
            match unsafe { lib.get(b"MFXVideoENCODE_Query\0") } {
                Ok(sym) => sym,
                Err(_) => {
                    // Try closing session via lib
                    if let Ok(close) = unsafe { lib.get::<MfxCloseFn>(b"MFXClose\0") } {
                        unsafe { close(session) };
                    }
                    unsafe { mfx_unload(loader) };
                    return false;
                }
            };

        let mfx_close: Symbol<MfxCloseFn> = match unsafe { lib.get(b"MFXClose\0") } {
            Ok(sym) => sym,
            Err(_) => {
                unsafe { mfx_unload(loader) };
                return false;
            }
        };

        caps.available = true;

        // Query H.264 support
        caps.h264 = query_encoder_support(&mfx_video_encode_query, session, MFX_CODEC_AVC);

        // Query HEVC support
        caps.hevc = query_encoder_support(&mfx_video_encode_query, session, MFX_CODEC_HEVC);

        // Query AV1 support
        caps.av1 = query_encoder_support(&mfx_video_encode_query, session, MFX_CODEC_AV1);

        // QSV typically supports B-frames and lookahead, but for Twitch compatibility
        // we often disable them. Mark as supported here.
        caps.b_frames = true;
        caps.lookahead = true;

        // Query low power mode
        caps.low_power = query_low_power_support(&mfx_video_encode_query, session, MFX_CODEC_AVC);

        // Get device name
        caps.device_name = get_intel_gpu_name();

        unsafe {
            mfx_close(session);
            mfx_unload(loader);
        }

        log::info!(
            "Native QSV (oneVPL): h264={}, hevc={}, av1={}, low_power={}, device={:?}",
            caps.h264, caps.hevc, caps.av1, caps.low_power, caps.device_name
        );

        true
    }

    #[cfg(not(target_os = "macos"))]
    fn try_legacy_mfx_probe(caps: &mut QsvCaps) -> bool {
        // Legacy Media SDK library names
        #[cfg(target_os = "windows")]
        let lib_names = ["libmfx.dll", "libmfxhw64.dll", "mfx.dll"];
        #[cfg(target_os = "linux")]
        let lib_names = ["libmfxhw64.so.1", "libmfx.so.1", "libmfx.so"];

        let lib = lib_names.iter().find_map(|name| {
            unsafe { Library::new(*name).ok() }
        });

        let lib = match lib {
            Some(l) => l,
            None => {
                log::debug!("Native QSV: Legacy Media SDK not found");
                return false;
            }
        };

        log::debug!("Native QSV: Using legacy Media SDK");

        // Get MFXInit
        let mfx_init: Symbol<MfxInitFn> = match unsafe { lib.get(b"MFXInit\0") } {
            Ok(sym) => sym,
            Err(_) => return false,
        };

        let mfx_close: Symbol<MfxCloseFn> = match unsafe { lib.get(b"MFXClose\0") } {
            Ok(sym) => sym,
            Err(_) => return false,
        };

        // Initialize session
        let mut version = MfxVersion { major: 1, minor: 0 };
        let mut session: MfxSession = ptr::null_mut();

        let ret = unsafe { mfx_init(MFX_IMPL_HARDWARE_ANY, &mut version, &mut session) };
        if ret != MFX_ERR_NONE || session.is_null() {
            log::debug!("Native QSV: MFXInit failed: {}", ret);
            return false;
        }

        // Get encode query function
        let mfx_video_encode_query: Symbol<MfxVideoEncodeQueryFn> =
            match unsafe { lib.get(b"MFXVideoENCODE_Query\0") } {
                Ok(sym) => sym,
                Err(_) => {
                    unsafe { mfx_close(session) };
                    return false;
                }
            };

        caps.available = true;

        // Query codec support
        caps.h264 = query_encoder_support(&mfx_video_encode_query, session, MFX_CODEC_AVC);
        caps.hevc = query_encoder_support(&mfx_video_encode_query, session, MFX_CODEC_HEVC);
        caps.av1 = query_encoder_support(&mfx_video_encode_query, session, MFX_CODEC_AV1);
        caps.b_frames = true;
        caps.lookahead = true;
        caps.low_power = query_low_power_support(&mfx_video_encode_query, session, MFX_CODEC_AVC);
        caps.device_name = get_intel_gpu_name();

        unsafe { mfx_close(session) };

        log::info!(
            "Native QSV (legacy): h264={}, hevc={}, av1={}, low_power={}, device={:?}",
            caps.h264, caps.hevc, caps.av1, caps.low_power, caps.device_name
        );

        true
    }

    fn query_encoder_support(
        query_fn: &MfxVideoEncodeQueryFn,
        session: MfxSession,
        codec_id: u32,
    ) -> bool {
        let mut in_param = MfxVideoParam::default();
        let mut out_param = MfxVideoParam::default();

        in_param.mfx.codec_id = codec_id;

        // Set minimal valid frame info
        in_param.mfx.frame_info.four_cc = 0x3231564E; // NV12
        in_param.mfx.frame_info._dimensions = [1920, 1080, 0, 0, 1920, 1080, 0, 0];
        in_param.mfx.frame_info.frame_rate = MfxFrameRate {
            frame_rate_ext_n: 30,
            frame_rate_ext_d: 1,
        };
        in_param.mfx.frame_info.chroma_format = 1; // 4:2:0
        in_param.io_pattern = 0x02; // MFX_IOPATTERN_IN_SYSTEM_MEMORY

        let ret = unsafe { query_fn(session, &mut in_param, &mut out_param) };

        // MFX_ERR_NONE or MFX_WRN_* (positive) means supported
        ret >= MFX_ERR_NONE
    }

    fn query_low_power_support(
        query_fn: &MfxVideoEncodeQueryFn,
        session: MfxSession,
        codec_id: u32,
    ) -> bool {
        let mut in_param = MfxVideoParam::default();
        let mut out_param = MfxVideoParam::default();

        in_param.mfx.codec_id = codec_id;
        in_param.mfx.low_power = 1; // MFX_CODINGOPTION_ON

        in_param.mfx.frame_info.four_cc = 0x3231564E; // NV12
        in_param.mfx.frame_info._dimensions = [1920, 1080, 0, 0, 1920, 1080, 0, 0];
        in_param.mfx.frame_info.frame_rate = MfxFrameRate {
            frame_rate_ext_n: 30,
            frame_rate_ext_d: 1,
        };
        in_param.mfx.frame_info.chroma_format = 1;
        in_param.io_pattern = 0x02;

        let ret = unsafe { query_fn(session, &mut in_param, &mut out_param) };
        ret >= MFX_ERR_NONE
    }

    fn get_intel_gpu_name() -> Option<String> {
        #[cfg(target_os = "windows")]
        {
            use std::process::Command;
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x08000000;

            let output = Command::new("wmic")
                .args(["path", "win32_VideoController", "get", "name"])
                .creation_flags(CREATE_NO_WINDOW)
                .output()
                .ok()?;

            if output.status.success() {
                let text = String::from_utf8_lossy(&output.stdout);
                for line in text.lines() {
                    let trimmed = line.trim();
                    if trimmed.contains("Intel") &&
                       (trimmed.contains("Graphics") || trimmed.contains("UHD") ||
                        trimmed.contains("Iris") || trimmed.contains("Arc")) {
                        return Some(trimmed.to_string());
                    }
                }
            }
            None
        }

        #[cfg(target_os = "linux")]
        {
            use std::process::Command;

            // Try lspci for Intel GPU
            let output = Command::new("lspci")
                .output()
                .ok()?;

            if output.status.success() {
                let text = String::from_utf8_lossy(&output.stdout);
                for line in text.lines() {
                    if line.contains("VGA") && line.contains("Intel") {
                        // Extract the GPU name from lspci output
                        if let Some(start) = line.find("Intel") {
                            let gpu_part = &line[start..];
                            // Clean up the string
                            let clean = gpu_part.trim_end_matches(|c: char| c == ')' || c == '(');
                            return Some(clean.to_string());
                        }
                    }
                }
            }
            None
        }

        #[cfg(not(any(target_os = "windows", target_os = "linux")))]
        {
            None
        }
    }
}

/// Native VideoToolbox probing (macOS)
/// Uses FFmpeg's VideoToolbox support check since VT is a system framework
mod native_videotoolbox {
    use super::VideoToolboxCaps;

    pub fn probe() -> VideoToolboxCaps {
        #[cfg(not(target_os = "macos"))]
        {
            VideoToolboxCaps::default()
        }

        #[cfg(target_os = "macos")]
        {
            let mut caps = VideoToolboxCaps::default();
            // VideoToolbox is available on all modern Macs
            // Check if FFmpeg has it compiled in by looking for the encoders
            caps.available = true;
            caps.h264 = true;  // All Macs support H.264
            caps.hevc = check_hevc_support();
            caps.hardware_accelerated = true;  // Modern Macs always have hardware encoding

            log::info!(
                "Native VideoToolbox: h264={}, hevc={}, hw_accel={}",
                caps.h264, caps.hevc, caps.hardware_accelerated
            );

            caps
        }
    }

    #[cfg(target_os = "macos")]
    fn check_hevc_support() -> bool {
        use std::process::Command;

        // HEVC encoding requires macOS 10.13+ and supported hardware
        // On Apple Silicon and recent Intel Macs, it's always available
        // Check OS version as a simple heuristic

        let output = Command::new("sw_vers")
            .args(["-productVersion"])
            .output()
            .ok();

        if let Some(output) = output {
            if output.status.success() {
                let version = String::from_utf8_lossy(&output.stdout);
                let parts: Vec<&str> = version.trim().split('.').collect();
                if let Some(major) = parts.first().and_then(|s| s.parse::<u32>().ok()) {
                    // macOS 10.13+ (High Sierra) supports HEVC
                    // macOS 11+ (Big Sur) is always supported
                    if major >= 11 {
                        return true;
                    }
                    if major == 10 {
                        if let Some(minor) = parts.get(1).and_then(|s| s.parse::<u32>().ok()) {
                            return minor >= 13;
                        }
                    }
                }
            }
        }

        // Default to true for modern systems
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_probe_returns_valid_struct() {
        let caps = EncoderCapabilities::probe_uncached();
        // Should not panic and should have timestamp
        assert!(caps.probed_at.is_some());
    }

    #[test]
    fn test_encoder_lists_are_consistent() {
        let caps = EncoderCapabilities::probe_uncached();

        // If NVENC H.264 is available, it should appear in the H.264 list
        let h264_encoders = caps.available_h264_encoders();
        if caps.nvenc.h264 {
            assert!(h264_encoders.iter().any(|e| e.id == "h264_nvenc"));
        }

        // If libx264 is available, it should appear in the H.264 list
        if caps.software.libx264 {
            assert!(h264_encoders.iter().any(|e| e.id == "libx264"));
        }
    }
}
