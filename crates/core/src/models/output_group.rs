// OutputGroup Model
// Encoding profile for stream targets

use crate::models::StreamTarget;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Video encoding settings
/// Default uses "copy" for passthrough (no re-encoding)
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/types/src/generated/")]
pub struct VideoSettings {
    /// FFmpeg video codec (e.g., "libx264", "h264_nvenc", "copy")
    /// Use "copy" for passthrough mode where FFmpeg acts as an RTMP relay
    pub codec: String,

    /// Output width in pixels
    pub width: u32,

    /// Output height in pixels
    pub height: u32,

    /// Frame rate (e.g., 60, 30)
    pub fps: u32,

    /// Bitrate with unit (e.g., "6000k", "8M")
    pub bitrate: String,

    /// Encoder preset (e.g., "ultrafast", "veryfast", "medium")
    #[serde(default)]
    pub preset: Option<String>,

    /// H.264 profile (e.g., "high", "main", "baseline")
    #[serde(default)]
    pub profile: Option<String>,

    /// Keyframe interval in seconds (optional)
    #[serde(default)]
    pub keyframe_interval_seconds: Option<u32>,
}

impl Default for VideoSettings {
    fn default() -> Self {
        Self {
            codec: "copy".to_string(),
            width: 0,
            height: 0,
            fps: 0,
            bitrate: "0k".to_string(),
            preset: None,
            profile: None,
            keyframe_interval_seconds: None,
        }
    }
}

impl VideoSettings {
    /// Get resolution as "WIDTHxHEIGHT" string for FFmpeg
    pub fn resolution(&self) -> String {
        format!("{}x{}", self.width, self.height)
    }
}

/// Audio encoding settings
/// Default uses "copy" for passthrough (no re-encoding)
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/types/src/generated/")]
pub struct AudioSettings {
    /// FFmpeg audio codec (e.g., "aac", "libmp3lame", "copy")
    /// Use "copy" for passthrough mode where FFmpeg acts as an RTMP relay
    pub codec: String,

    /// Bitrate with unit (e.g., "160k", "192k")
    pub bitrate: String,

    /// Number of audio channels (1=mono, 2=stereo)
    pub channels: u8,

    /// Sample rate in Hz (e.g., 48000, 44100)
    pub sample_rate: u32,
}

impl Default for AudioSettings {
    fn default() -> Self {
        Self {
            codec: "copy".to_string(),
            bitrate: "0k".to_string(),
            channels: 0,
            sample_rate: 0,
        }
    }
}

/// Container/muxing settings
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/types/src/generated/")]
pub struct ContainerSettings {
    /// Output format (e.g., "flv" for RTMP)
    pub format: String,
}

impl Default for ContainerSettings {
    fn default() -> Self {
        Self {
            format: "flv".to_string(),
        }
    }
}

/// An output group defines encoding settings for a set of stream targets
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/types/src/generated/")]
pub struct OutputGroup {
    /// Unique identifier
    pub id: String,

    /// Display name for the group
    pub name: String,

    /// Whether this is the default passthrough group (immutable)
    #[serde(default)]
    pub is_default: bool,

    /// Generate presentation timestamps (PTS) for the stream
    /// Helps with timing issues and encoding problems at minor CPU cost
    /// Default: true
    #[serde(default = "default_generate_pts")]
    pub generate_pts: bool,

    /// Whether this group participates in "start all". Persisted
    /// profile data — replaces the frontend's `enabledGroups`
    /// empty-set-means-all sentinel, whose edge cases re-enabled every
    /// group when the user disabled the last one. Eligibility is
    /// decided server-side from this flag.
    #[serde(default = "default_enabled")]
    pub enabled: bool,

    /// Video encoding settings
    pub video: VideoSettings,

    /// Audio encoding settings
    pub audio: AudioSettings,

    /// Container/muxing settings
    pub container: ContainerSettings,

    /// Stream destinations
    pub stream_targets: Vec<StreamTarget>,
}

impl OutputGroup {
    /// Create a new output group with default settings (for custom encoding)
    pub fn new() -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name: "New Output Group".to_string(),
            is_default: false,
            generate_pts: true,
            enabled: true,
            video: VideoSettings::default(),
            audio: AudioSettings::default(),
            container: ContainerSettings::default(),
            stream_targets: Vec::new(),
        }
    }

    /// Whether this group should participate in a stream start: the
    /// group is enabled AND at least one of its targets is enabled.
    /// Single source of truth for `start` / `start_all` admission — the
    /// frontend renders the outcome, it never re-derives this.
    pub fn is_eligible(&self) -> bool {
        self.enabled && self.stream_targets.iter().any(|t| t.enabled)
    }
}

impl Default for OutputGroup {
    fn default() -> Self {
        Self::new()
    }
}

/// Default value for generate_pts field (true)
fn default_generate_pts() -> bool {
    true
}

fn default_enabled() -> bool {
    true
}
