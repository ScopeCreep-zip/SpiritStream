// OutputGroup Model
// Encoding profile for stream targets

use serde::{Deserialize, Serialize};
use crate::models::StreamTarget;

/// Video encoding settings
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoSettings {
    /// FFmpeg video codec (e.g., "libx264", "h264_nvenc")
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
}

impl Default for VideoSettings {
    fn default() -> Self {
        Self {
            codec: "libx264".to_string(),
            width: 1920,
            height: 1080,
            fps: 60,
            bitrate: "6000k".to_string(),
            preset: Some("veryfast".to_string()),
            profile: Some("high".to_string()),
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
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioSettings {
    /// FFmpeg audio codec (e.g., "aac", "libmp3lame")
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
            codec: "aac".to_string(),
            bitrate: "160k".to_string(),
            channels: 2,
            sample_rate: 48000,
        }
    }
}

/// Container/muxing settings
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
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
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutputGroup {
    /// Unique identifier
    pub id: String,

    /// Display name for the group
    pub name: String,

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
    /// Create a new output group with default settings
    pub fn new() -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name: "New Output Group".to_string(),
            video: VideoSettings::default(),
            audio: AudioSettings::default(),
            container: ContainerSettings::default(),
            stream_targets: Vec::new(),
        }
    }
}

impl Default for OutputGroup {
    fn default() -> Self {
        Self::new()
    }
}
