// OutputGroup Model
// Encoding profile for stream targets

use serde::{Deserialize, Serialize};
use crate::models::StreamTarget;

/// An output group defines encoding settings for a set of stream targets
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutputGroup {
    /// Unique identifier
    pub id: String,

    /// Display name for the group
    #[serde(default)]
    pub name: Option<String>,

    /// FFmpeg video codec (e.g., "libx264", "h264_nvenc")
    pub video_encoder: String,

    /// Output resolution (e.g., "1920x1080")
    pub resolution: String,

    /// Video bitrate in kbps
    pub video_bitrate: u32,

    /// Frame rate
    pub fps: u32,

    /// FFmpeg audio codec (e.g., "aac", "libmp3lame")
    pub audio_codec: String,

    /// Audio bitrate in kbps
    pub audio_bitrate: u32,

    /// Whether to generate PTS timestamps
    pub generate_pts: bool,

    /// Encoder preset (e.g., "ultrafast", "fast", "medium", "slow")
    #[serde(default)]
    pub preset: Option<String>,

    /// Rate control mode (e.g., "cbr", "vbr", "cqp")
    #[serde(default)]
    pub rate_control: Option<String>,

    /// Stream destinations
    pub stream_targets: Vec<StreamTarget>,
}

impl OutputGroup {
    /// Create a new output group with default settings
    pub fn new() -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name: None,
            video_encoder: "libx264".to_string(),
            resolution: "1920x1080".to_string(),
            video_bitrate: 6000,
            fps: 60,
            audio_codec: "aac".to_string(),
            audio_bitrate: 128,
            generate_pts: false,
            preset: None,
            rate_control: None,
            stream_targets: Vec::new(),
        }
    }
}

impl Default for OutputGroup {
    fn default() -> Self {
        Self::new()
    }
}
