// Profile Model
// Top-level configuration entity

use serde::{Deserialize, Serialize};
use std::collections::{HashSet, HashMap};
use crate::models::{OutputGroup, Platform, Source, RtmpSource, Scene, SourceLayer, AudioMixer, AudioTrack, Transform, SceneTransition};

/// RTMP Input configuration - where the stream enters the system
/// LEGACY: Kept for backward compatibility, use Sources instead
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
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

    /// LEGACY: RTMP input configuration (for backward compatibility)
    /// New profiles should use sources and scenes instead
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input: Option<RtmpInput>,

    /// NEW: Multiple input sources (RTMP, cameras, screens, files, etc.)
    #[serde(default)]
    pub sources: Vec<Source>,

    /// NEW: Scene compositions (layouts with positioned sources)
    #[serde(default)]
    pub scenes: Vec<Scene>,

    /// NEW: Currently active scene ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_scene_id: Option<String>,

    /// Default transition for scene switching
    #[serde(default)]
    pub default_transition: Option<SceneTransition>,

    /// Encoding configurations with their targets
    pub output_groups: Vec<OutputGroup>,
}

impl Profile {
    /// Migrate legacy profile format to new multi-source format
    /// Call this after loading a profile to ensure it uses the new format
    pub fn migrate_if_needed(&mut self) {
        // Skip if already migrated (has sources)
        if !self.sources.is_empty() {
            return;
        }

        // Check if legacy input exists
        if let Some(rtmp_input) = self.input.take() {
            // Convert legacy RtmpInput to Source::Rtmp
            let source_id = uuid::Uuid::new_v4().to_string();
            let source = Source::Rtmp(RtmpSource {
                id: source_id.clone(),
                name: "Main Input".to_string(),
                bind_address: rtmp_input.bind_address,
                port: rtmp_input.port,
                application: rtmp_input.application,
                capture_audio: true,
            });

            // Create default scene with fullscreen source
            let scene = Scene {
                id: uuid::Uuid::new_v4().to_string(),
                name: "Main".to_string(),
                canvas_width: 1920,
                canvas_height: 1080,
                layers: vec![SourceLayer {
                    id: uuid::Uuid::new_v4().to_string(),
                    source_id: source_id.clone(),
                    visible: true,
                    locked: false,
                    transform: Transform {
                        x: 0,
                        y: 0,
                        width: 1920,
                        height: 1080,
                        rotation: 0.0,
                        crop: None,
                    },
                    z_index: 0,
                }],
                audio_mixer: AudioMixer {
                    master_volume: 1.0,
                    master_muted: false,
                    tracks: vec![AudioTrack {
                        source_id: source_id.clone(),
                        volume: 1.0,
                        muted: false,
                        solo: false,
                    }],
                },
                transition_in: None,
            };

            let scene_id = scene.id.clone();
            self.sources.push(source);
            self.scenes.push(scene);
            self.active_scene_id = Some(scene_id);
        }
    }

    /// Get the active scene, if any
    pub fn active_scene(&self) -> Option<&Scene> {
        self.active_scene_id.as_ref().and_then(|id| {
            self.scenes.iter().find(|s| &s.id == id)
        })
    }

    /// Get the active scene mutably
    pub fn active_scene_mut(&mut self) -> Option<&mut Scene> {
        let active_id = self.active_scene_id.clone()?;
        self.scenes.iter_mut().find(|s| s.id == active_id)
    }

    /// Get a source by ID
    pub fn get_source(&self, source_id: &str) -> Option<&Source> {
        self.sources.iter().find(|s| s.id() == source_id)
    }

    /// Get incoming URL for the active scene (for FFmpeg handler)
    /// Returns the RTMP URL if the first source is RTMP, otherwise None
    pub fn get_incoming_url(&self) -> Option<String> {
        // First check legacy input
        if let Some(ref input) = self.input {
            return Some(format!(
                "rtmp://{}:{}/{}",
                input.bind_address, input.port, input.application
            ));
        }

        // Check new sources - find first RTMP source
        for source in &self.sources {
            if let Source::Rtmp(rtmp) = source {
                return Some(format!(
                    "rtmp://{}:{}/{}",
                    rtmp.bind_address, rtmp.port, rtmp.application
                ));
            }
        }

        None
    }
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

pub type OrderIndexMap = HashMap<String, i32>;

