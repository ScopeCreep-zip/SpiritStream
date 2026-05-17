// Profile Model
// Top-level configuration entity

use crate::models::{OutputGroup, Platform, ProfileSettings};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use ts_rs::TS;

/// RTMP Input configuration - where the stream enters the system
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/types/src/generated/")]
pub struct RtmpInput {
    /// Input type (RTMP only)
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
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/types/src/generated/")]
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

    /// Per-profile settings (theme, integrations, security)
    /// Uses #[serde(default)] for backward compatibility with existing profiles
    #[serde(default)]
    pub settings: ProfileSettings,

    /// PII blocklist phrases (real name, deadname,
    /// hometown). Encrypted alongside the rest of the profile body
    /// for password-protected profiles; for plaintext profiles each
    /// entry is wrapped with the `ENC2::` machine-key envelope.
    #[serde(default)]
    pub pii_blocklist: Vec<String>,

    /// When true, the fuzzy leet-speak matcher applies in
    /// addition to the strict substring matcher. Off by default to
    /// minimise false positives.
    #[serde(default)]
    pub pii_fuzzy: bool,

    /// When true (default for new profiles), chat
    /// usernames in logs render as `hash:abcd1234` rather than
    /// plaintext. Reversible by the local user with the per-profile
    /// salt; one-way for anyone else who acquires only the log.
    #[serde(default = "default_anonymous_logging")]
    pub anonymous_logging: bool,

    /// Per-profile HMAC salt for the pseudonymizer.
    /// 64-char hex (32 raw bytes). Generated once at profile
    /// creation; never reused across profiles. If the salt is empty
    /// (legacy profiles loaded from disk before this field existed),
    /// the pseudonymizer falls back to plaintext — the loader runs
    /// `profile.ensure_anonymous_salt()` to populate it on first use.
    #[serde(default)]
    pub anonymous_salt: String,
}

impl Profile {
    /// Populate `anonymous_salt` if it's currently empty. Called by
    /// the profile loader so existing legacy profiles get a salt the
    /// first time they're read.
    pub fn ensure_anonymous_salt(&mut self) {
        if self.anonymous_salt.is_empty() {
            self.anonymous_salt = crate::services::pseudonymizer::generate_salt();
        }
    }
}

fn default_anonymous_logging() -> bool {
    // New profiles default to anonymous mode ON. The user
    // explicitly opts out (with a warning) during the first-run
    // wizard or later in safety settings.
    true
}

impl Profile {
    /// Generate a summary of this profile for list display
    pub fn to_summary(&self, is_encrypted: bool) -> ProfileSummary {
        // Get resolution and bitrate from first output group if available
        let (resolution, bitrate) = self
            .output_groups
            .first()
            .map(|g| {
                // Check if this is a copy (passthrough) output group
                if g.video.codec == "copy" {
                    // For copy mode, show "Passthrough" instead of resolution
                    ("Passthrough".to_string(), 0)
                } else {
                    let res = format!("{}p{}", g.video.height, g.video.fps);
                    let bitrate = g
                        .video
                        .bitrate
                        .trim_end_matches(|c: char| !c.is_numeric())
                        .parse::<u32>()
                        .unwrap_or(0);
                    (res, bitrate)
                }
            })
            .unwrap_or_else(|| ("None".to_string(), 0));

        // Count total targets across all output groups
        let target_count = self
            .output_groups
            .iter()
            .map(|g| g.stream_targets.len())
            .sum::<usize>() as u32;

        // Collect unique services from all targets
        let services: Vec<Platform> = self
            .output_groups
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
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/types/src/generated/")]
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

pub type OrderIndexMap = HashMap<String, i32>;
