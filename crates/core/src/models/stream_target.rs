// StreamTarget Model
// RTMP destination configuration

use serde::{Deserialize, Serialize};
use ts_rs::TS;

// Platform enum auto-generated from data/streaming-platforms.json at build time
include!(concat!(env!("OUT_DIR"), "/generated_platforms.rs"));

/// A stream target represents an RTMP destination
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/types/src/generated/")]
pub struct StreamTarget {
    /// Unique identifier
    pub id: String,

    /// Streaming service/platform (youtube, twitch, kick, facebook, custom)
    #[serde(default)]
    pub service: Platform,

    /// Display name
    #[serde(default)]
    pub name: String,

    /// RTMP server URL
    pub url: String,

    /// Stream key (authentication) - supports ${ENV_VAR} syntax
    pub stream_key: String,

    /// Whether this target participates in stream starts. Persisted
    /// profile data — the pre-stream on/off toggle in the UI edits THIS
    /// field (and saves the profile), so the backend is the authority
    /// on which destinations go live. The previous design kept the
    /// toggle in frontend-only state: the UI showed a target as off
    /// while FFmpeg happily streamed to it.
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_enabled() -> bool {
    true
}
