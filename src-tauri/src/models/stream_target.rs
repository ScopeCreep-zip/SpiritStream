// StreamTarget Model
// RTMP destination configuration

use serde::{Deserialize, Serialize};

/// A stream target represents an RTMP destination
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamTarget {
    /// Unique identifier
    pub id: String,

    /// RTMP server URL
    pub url: String,

    /// Stream key (authentication)
    pub stream_key: String,

    /// RTMP port (default: 1935)
    #[serde(default = "default_port")]
    pub port: u16,

    /// Platform type for UI display
    #[serde(default)]
    pub platform: Platform,

    /// Display name
    #[serde(default)]
    pub name: String,
}

fn default_port() -> u16 {
    1935
}

/// Supported streaming platforms
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Platform {
    Youtube,
    Twitch,
    Kick,
    Facebook,
    #[default]
    Custom,
}

impl StreamTarget {
    /// Create a new stream target
    pub fn new(url: String, stream_key: String) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            url,
            stream_key,
            port: 1935,
            platform: Platform::Custom,
            name: String::new(),
        }
    }
}
