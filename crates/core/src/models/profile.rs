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

    /// Full incoming RTMP URL, computed server-side from the fields
    /// above on every save/load (`refresh_url`). Frontends display and
    /// send this value verbatim — they never string-build it (the old
    /// frontend construction existed in three copies that had to stay
    /// in lockstep with `FFmpegHandler`).
    #[serde(default)]
    pub url: String,
}

impl RtmpInput {
    /// Recompute `url` from the constituent fields. Single source of
    /// truth for the incoming-URL shape.
    pub fn refresh_url(&mut self) {
        self.url = format!(
            "rtmp://{}:{}/{}",
            self.bind_address, self.port, self.application
        );
    }
}

impl Default for RtmpInput {
    fn default() -> Self {
        let mut input = Self {
            input_type: "rtmp".to_string(),
            bind_address: "0.0.0.0".to_string(),
            port: 1935,
            application: "live".to_string(),
            url: String::new(),
        };
        input.refresh_url();
        input
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
    /// save and activation both run `ensure_anonymous_salt()` to
    /// populate and persist it. The pseudonymizer itself NEVER falls
    /// back to plaintext: an enabled policy with a broken salt fails
    /// activation, and any message that can't be pseudonymised is
    /// dropped rather than logged with its real username.
    #[serde(default)]
    pub anonymous_salt: String,
}

impl Profile {
    /// Populate `anonymous_salt` if it's currently empty. Called by
    /// the profile save path and by `ProfileActivationService::activate`
    /// so legacy profiles get a salt before anonymous mode can engage.
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{
        AudioSettings, ContainerSettings, OutputGroup, StreamTarget, VideoSettings,
    };

    fn video(codec: &str, height: u32, fps: u32, bitrate: &str) -> VideoSettings {
        VideoSettings {
            codec: codec.into(),
            width: 1920,
            height,
            fps,
            bitrate: bitrate.into(),
            preset: None,
            profile: None,
            keyframe_interval_seconds: None,
        }
    }

    fn audio() -> AudioSettings {
        AudioSettings {
            codec: "aac".into(),
            bitrate: "160k".into(),
            channels: 2,
            sample_rate: 48000,
        }
    }

    fn target(service: Platform) -> StreamTarget {
        StreamTarget {
            id: "t".into(),
            name: "t".into(),
            service,
            url: "rtmp://x/live".into(),
            stream_key: "k".into(),
            enabled: true,
        }
    }

    fn group(video: VideoSettings, targets: Vec<StreamTarget>) -> OutputGroup {
        OutputGroup {
            id: "g".into(),
            name: "g".into(),
            is_default: true,
            generate_pts: true,
            enabled: true,
            video,
            audio: audio(),
            container: ContainerSettings::default(),
            stream_targets: targets,
        }
    }

    fn profile(groups: Vec<OutputGroup>) -> Profile {
        Profile {
            id: "p1".into(),
            name: "Profile One".into(),
            encrypted: false,
            input: RtmpInput::default(),
            output_groups: groups,
            settings: ProfileSettings::default(),
            pii_blocklist: vec![],
            pii_fuzzy: false,
            anonymous_logging: true,
            anonymous_salt: String::new(),
        }
    }

    #[test]
    fn to_summary_passthrough_group_shows_passthrough_and_zero_bitrate() {
        let p = profile(vec![group(
            video("copy", 0, 0, "0k"),
            vec![target(Platform::Twitch)],
        )]);
        let s = p.to_summary(false);
        assert_eq!(s.resolution, "Passthrough");
        assert_eq!(s.bitrate, 0);
        assert_eq!(s.target_count, 1);
        assert!(!s.is_encrypted);
        assert_eq!(s.id, "p1");
        assert_eq!(s.name, "Profile One");
    }

    #[test]
    fn to_summary_encoded_group_formats_resolution_and_parses_bitrate() {
        let p = profile(vec![group(
            video("libx264", 720, 30, "6000k"),
            vec![target(Platform::Twitch), target(Platform::YouTubeRTMPS)],
        )]);
        let s = p.to_summary(true);
        assert_eq!(s.resolution, "720p30");
        assert_eq!(s.bitrate, 6000);
        assert_eq!(s.target_count, 2);
        assert!(s.is_encrypted);
        // Two distinct services collected uniquely.
        assert_eq!(s.services.len(), 2);
    }

    #[test]
    fn to_summary_dedupes_services_across_targets() {
        let p = profile(vec![group(
            video("libx264", 1080, 60, "8000k"),
            vec![target(Platform::Twitch), target(Platform::Twitch)],
        )]);
        let s = p.to_summary(false);
        assert_eq!(s.target_count, 2);
        // Same platform twice collapses to one unique service.
        assert_eq!(s.services.len(), 1);
    }

    #[test]
    fn to_summary_unparseable_bitrate_falls_back_to_zero() {
        let p = profile(vec![group(
            video("libx264", 720, 30, "notanumber"),
            vec![target(Platform::Twitch)],
        )]);
        let s = p.to_summary(false);
        assert_eq!(s.resolution, "720p30");
        assert_eq!(s.bitrate, 0);
    }

    #[test]
    fn to_summary_no_output_groups_reports_none() {
        let p = profile(vec![]);
        let s = p.to_summary(false);
        assert_eq!(s.resolution, "None");
        assert_eq!(s.bitrate, 0);
        assert_eq!(s.target_count, 0);
        assert!(s.services.is_empty());
    }

    #[test]
    fn ensure_anonymous_salt_populates_when_empty_and_preserves_existing() {
        let mut p = profile(vec![]);
        assert!(p.anonymous_salt.is_empty());
        p.ensure_anonymous_salt();
        assert!(!p.anonymous_salt.is_empty(), "salt generated when empty");

        let existing = p.anonymous_salt.clone();
        p.ensure_anonymous_salt();
        assert_eq!(p.anonymous_salt, existing, "existing salt left untouched");
    }

    #[test]
    fn rtmp_input_default_is_rtmp_live_on_1935() {
        let input = RtmpInput::default();
        assert_eq!(input.input_type, "rtmp");
        assert_eq!(input.bind_address, "0.0.0.0");
        assert_eq!(input.port, 1935);
        assert_eq!(input.application, "live");
    }
}
