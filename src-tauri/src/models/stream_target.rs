// StreamTarget Model
// RTMP destination configuration

use serde::{Deserialize, Serialize};

/// Supported streaming services/platforms
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum Platform {
    Youtube,
    Twitch,
    Kick,
    Facebook,
    #[default]
    Custom,
}

impl std::fmt::Display for Platform {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Platform::Youtube => write!(f, "youtube"),
            Platform::Twitch => write!(f, "twitch"),
            Platform::Kick => write!(f, "kick"),
            Platform::Facebook => write!(f, "facebook"),
            Platform::Custom => write!(f, "custom"),
        }
    }
}

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

