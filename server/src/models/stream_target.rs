// StreamTarget Model
// RTMP destination configuration

use serde::{Deserialize, Serialize};

// Platform enum auto-generated from data/streaming-platforms.json at build time
include!(concat!(env!("OUT_DIR"), "/generated_platforms.rs"));

/// A stream target represents an RTMP destination
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
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
}

