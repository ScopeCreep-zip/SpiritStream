// Profile Model
// Top-level configuration entity

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use crate::models::{OutputGroup, Platform};

/// RTMP Input configuration - where the stream enters the system
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RtmpInput {
    /// Input type (always "rtmp" for now)
    #[serde(rename = "type")]
    pub input_type: String,

    /// Network interface to bind to (e.g., "0.0.0.0", "127.0.0.1")
    pub bind_address: String,

    /// TCP port to listen on (e.g., 1935)
    pub port: u16,

    /// RTMP application/path (e.g., "live", "ingest")
    pub application: String,
}

impl Default for RtmpInput {
    fn default() -> Self {
        Self {
            input_type: "rtmp".to_string(),
            bind_address: "0.0.0.0".to_string(),
            port: 1935,
            application: "live".to_string(),
        }
    }
}

/// A streaming profile containing all configuration for a stream setup
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    /// Unique identifier
    pub id: String,

    /// User-friendly name
    pub name: String,

    /// Whether this profile is encrypted
    #[serde(default)]
    pub encrypted: bool,

    /// RTMP input configuration
    pub input: RtmpInput,

    /// Encoding configurations with their targets
    pub output_groups: Vec<OutputGroup>,
}

impl Profile {
    /// Generate a summary of this profile for list display
    pub fn to_summary(&self, is_encrypted: bool) -> ProfileSummary {
        // Get resolution and bitrate from first output group if available
        let (resolution, bitrate) = self.output_groups.first().map(|g| {
            // Check if this is a copy (passthrough) output group
            if g.video.codec == "copy" {
                // For copy mode, show "Passthrough" instead of resolution
                ("Passthrough".to_string(), 0)
            } else {
                let res = format!("{}p{}", g.video.height, g.video.fps);
                let bitrate = g.video.bitrate
                    .trim_end_matches(|c: char| !c.is_numeric())
                    .parse::<u32>()
                    .unwrap_or(0);
                (res, bitrate)
            }
        }).unwrap_or_else(|| ("None".to_string(), 0));

        // Count total targets across all output groups
        let target_count = self.output_groups
            .iter()
            .map(|g| g.stream_targets.len())
            .sum::<usize>() as u32;

        // Collect unique services from all targets
        let services: Vec<Platform> = self.output_groups
            .iter()
            .flat_map(|g| g.stream_targets.iter())
            .map(|t| t.service.clone())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();

        ProfileSummary {
            id: self.id.clone(),
            name: self.name.clone(),
            resolution,
            bitrate,
            target_count,
            services,
            is_encrypted,
        }
    }
}

/// Profile summary for list display (Story 1.1, 4.1, 4.2)
/// Shows at a glance which platforms a profile streams to
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileSummary {
    /// Unique identifier
    pub id: String,

    /// User-friendly name
    pub name: String,

    /// Resolution string (e.g., "1080p60")
    pub resolution: String,

    /// Bitrate in kbps
    pub bitrate: u32,

    /// Total number of stream targets
    pub target_count: u32,

    /// List of configured services/platforms (e.g., ["youtube", "twitch"])
    pub services: Vec<Platform>,

    /// Whether the profile file is encrypted
    pub is_encrypted: bool,
}

