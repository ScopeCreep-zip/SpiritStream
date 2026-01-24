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
        let mut caps = EncoderCapabilities::default();
        caps.probed_at = Some(chrono::Utc::now().to_rfc3339());

        log::info!("=== Starting encoder capability probe ===");

        // Probe each encoder type
        // For now, use CLI-based probing until ffmpeg-sys-next is integrated
        // Later phases will add native OBS-style probing via FFmpeg libs

        log::info!("Probing NVENC (NVIDIA)...");
        caps.nvenc = Self::probe_nvenc_via_cli();
        log::info!("  NVENC: available={}, h264={}, hevc={}, av1={}, gpu={:?}",
            caps.nvenc.available, caps.nvenc.h264, caps.nvenc.hevc, caps.nvenc.av1, caps.nvenc.gpu_name);

        log::info!("Probing AMF (AMD)...");
        caps.amf = Self::probe_amf_via_cli();
        log::info!("  AMF: available={}, h264={}, hevc={}, av1={}",
            caps.amf.available, caps.amf.h264, caps.amf.hevc, caps.amf.av1);

        log::info!("Probing QSV (Intel)...");
        caps.qsv = Self::probe_qsv_via_cli();
        log::info!("  QSV: available={}, h264={}, hevc={}, av1={}, low_power={}",
            caps.qsv.available, caps.qsv.h264, caps.qsv.hevc, caps.qsv.av1, caps.qsv.low_power);

        log::info!("Probing VideoToolbox (Apple)...");
        caps.videotoolbox = Self::probe_videotoolbox_via_cli();
        log::info!("  VideoToolbox: available={}, h264={}, hevc={}",
            caps.videotoolbox.available, caps.videotoolbox.h264, caps.videotoolbox.hevc);

        log::info!("Probing software encoders...");
        caps.software = Self::probe_software_via_cli();
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

    /// Check if an encoder is available in FFmpeg
    fn encoder_available(encoder_name: &str) -> bool {
        Self::run_ffmpeg(&["-encoders", "-hide_banner"])
            .map(|output| output.contains(encoder_name))
            .unwrap_or(false)
    }

    /// Probe NVENC via CLI
    fn probe_nvenc_via_cli() -> NvencCaps {
        let mut caps = NvencCaps::default();

        // Check if NVENC encoders are compiled into FFmpeg
        let h264_available = Self::encoder_available("h264_nvenc");
        let hevc_available = Self::encoder_available("hevc_nvenc");
        let av1_available = Self::encoder_available("av1_nvenc");

        log::debug!("  NVENC FFmpeg support: h264={}, hevc={}, av1={}",
            h264_available, hevc_available, av1_available);

        if !h264_available && !hevc_available && !av1_available {
            log::debug!("  NVENC: No encoders compiled into FFmpeg");
            return caps;
        }

        // Try to actually initialize the encoder with a test encode
        // This verifies driver/hardware availability
        log::debug!("  NVENC: Testing encoder initialization...");
        if let Some(output) = Self::run_ffmpeg(&[
            "-f", "lavfi",
            "-i", "color=black:s=64x64:d=0.1",
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
        }

        caps
    }

    /// Probe AMF via CLI
    fn probe_amf_via_cli() -> AmfCaps {
        let mut caps = AmfCaps::default();

        // AMF is Windows-only
        #[cfg(not(target_os = "windows"))]
        {
            log::debug!("  AMF: Not available (Windows-only)");
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
            }

            caps
        }
    }

    /// Probe QSV via CLI
    fn probe_qsv_via_cli() -> QsvCaps {
        let mut caps = QsvCaps::default();

        // QSV is Windows/Linux only
        #[cfg(target_os = "macos")]
        {
            log::debug!("  QSV: Not available (Windows/Linux only)");
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
                return caps;
            }

            // Try to initialize QSV encoder
            log::debug!("  QSV: Testing encoder initialization...");
            if let Some(output) = Self::run_ffmpeg(&[
                "-f", "lavfi",
                "-i", "color=black:s=64x64:d=0.1",
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
            }

            caps
        }
    }

    /// Probe VideoToolbox via CLI
    fn probe_videotoolbox_via_cli() -> VideoToolboxCaps {
        // VideoToolbox is macOS only
        #[cfg(not(target_os = "macos"))]
        return VideoToolboxCaps::default();

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
    fn probe_software_via_cli() -> SoftwareCaps {
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
// Future: Native OBS-style probing (Phase 3+)
// These will use ffmpeg-sys-next to directly load vendor DLLs and query caps
// =========================================================================

/// Native NVENC probing (future implementation)
/// Will load nvEncodeAPI64.dll and call NvEncodeAPICreateInstance
#[allow(dead_code)]
mod native_nvenc {
    // To be implemented when ffmpeg-sys-next is integrated
    // See ffmpeg-libs-plan.md for implementation details
}

/// Native AMF probing (future implementation)
/// Will load amfrt64.dll and call AMFInit
#[allow(dead_code)]
mod native_amf {
    // To be implemented when ffmpeg-sys-next is integrated
}

/// Native QSV probing (future implementation)
/// Will use MFXVideoENCODE_Query to check capabilities
#[allow(dead_code)]
mod native_qsv {
    // To be implemented when ffmpeg-sys-next is integrated
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
