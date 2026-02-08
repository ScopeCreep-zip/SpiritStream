use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Represents a platform that supports chat
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChatPlatform {
    Twitch,
    #[serde(rename = "tiktok")]
    TikTok,
    YouTube,
    Trovo,
    Stripchat,
    Kick,
    Facebook,
}

impl ChatPlatform {
    pub fn as_str(&self) -> &'static str {
        match self {
            ChatPlatform::Twitch => "twitch",
            ChatPlatform::TikTok => "tiktok",
            ChatPlatform::YouTube => "youtube",
            ChatPlatform::Trovo => "trovo",
            ChatPlatform::Stripchat => "stripchat",
            ChatPlatform::Kick => "kick",
            ChatPlatform::Facebook => "facebook",
        }
    }
}

/// A normalized chat message from any platform
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessage {
    /// Unique message ID
    pub id: String,
    /// Platform this message came from
    pub platform: ChatPlatform,
    /// Platforms this message was sent to (for cross-post / outbound)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub platforms: Option<Vec<ChatPlatform>>,
    /// Username of the sender
    pub username: String,
    /// The message content
    pub message: String,
    /// Timestamp in milliseconds since Unix epoch
    pub timestamp: i64,
    /// Message direction (inbound/outbound)
    #[serde(default)]
    pub direction: ChatMessageDirection,
    /// Source message ID from the platform (if available)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_id: Option<String>,
    /// Optional: User's display color (hex format)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    /// Optional: User badges (moderator, subscriber, etc.)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub badges: Option<Vec<String>>,
}

impl ChatMessage {
    pub fn new(
        platform: ChatPlatform,
        username: String,
        message: String,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            platform,
            platforms: None,
            username,
            message,
            timestamp: chrono::Utc::now().timestamp_millis(),
            direction: ChatMessageDirection::Inbound,
            source_id: None,
            color: None,
            badges: None,
        }
    }

    pub fn new_outbound(
        platforms: Vec<ChatPlatform>,
        username: String,
        message: String,
    ) -> Self {
        let primary = platforms.first().copied().unwrap_or(ChatPlatform::Twitch);
        Self {
            id: Uuid::new_v4().to_string(),
            platform: primary,
            platforms: Some(platforms),
            username,
            message,
            timestamp: chrono::Utc::now().timestamp_millis(),
            direction: ChatMessageDirection::Outbound,
            source_id: None,
            color: None,
            badges: None,
        }
    }

    pub fn with_color(mut self, color: String) -> Self {
        self.color = Some(color);
        self
    }

    pub fn with_source_id(mut self, source_id: String) -> Self {
        self.id = format!("{}:{}", self.platform.as_str(), source_id);
        self.source_id = Some(source_id);
        self
    }

    pub fn with_badges(mut self, badges: Vec<String>) -> Self {
        self.badges = Some(badges);
        self
    }
}

/// Direction for chat messages
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChatMessageDirection {
    Inbound,
    Outbound,
}

impl Default for ChatMessageDirection {
    fn default() -> Self {
        ChatMessageDirection::Inbound
    }
}

/// Configuration for a chat platform connection
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatConfig {
    /// Platform to connect to
    pub platform: ChatPlatform,
    /// Whether this platform is enabled
    pub enabled: bool,
    /// Platform-specific configuration
    pub credentials: ChatCredentials,
}

/// Platform-specific credentials
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ChatCredentials {
    #[serde(rename_all = "camelCase")]
    Twitch {
        /// Twitch channel name to join
        channel: String,
        /// Authentication method (optional - anonymous read-only if not provided)
        #[serde(skip_serializing_if = "Option::is_none")]
        auth: Option<TwitchAuth>,
    },
    #[serde(rename_all = "camelCase")]
    TikTok {
        /// TikTok username to monitor
        username: String,
        /// Session cookies/token (may be needed for some unofficial APIs)
        #[serde(skip_serializing_if = "Option::is_none")]
        session_token: Option<String>,
    },
    #[serde(rename_all = "camelCase")]
    YouTube {
        /// YouTube channel ID or handle (e.g., "UCxxxxxx" or "@channelname")
        /// The backend will automatically find the current live stream
        channel_id: String,
        /// Authentication method
        auth: YouTubeAuth,
    },
    #[serde(rename_all = "camelCase")]
    Trovo {
        /// Trovo channel ID (numeric user/channel ID)
        channel_id: String,
    },
    #[serde(rename_all = "camelCase")]
    Stripchat {
        /// Stripchat model username
        username: String,
    },
    #[serde(rename_all = "camelCase")]
    Kick {
        /// Kick channel name
        channel: String,
    },
    #[serde(rename_all = "camelCase")]
    Facebook {
        /// Facebook Live video ID
        video_id: String,
        /// Facebook access token
        access_token: String,
    },
}

/// Twitch authentication options
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "method", rename_all = "camelCase")]
pub enum TwitchAuth {
    /// User-provided OAuth token (from twitchtokengenerator.com or similar)
    #[serde(rename_all = "camelCase")]
    UserToken {
        /// OAuth token (with or without "oauth:" prefix)
        oauth_token: String,
    },
    /// App OAuth - user authenticated via "Login with Twitch" flow
    #[serde(rename_all = "camelCase")]
    AppOAuth {
        /// Access token from OAuth flow
        #[serde(default)]
        access_token: String,
        /// Refresh token for renewal
        #[serde(skip_serializing_if = "Option::is_none")]
        refresh_token: Option<String>,
        /// Token expiration timestamp (Unix epoch seconds)
        #[serde(skip_serializing_if = "Option::is_none")]
        expires_at: Option<i64>,
    },
}

/// YouTube authentication options
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "method", rename_all = "camelCase")]
pub enum YouTubeAuth {
    /// User-provided API key (preferred - uses user's own quota)
    #[serde(rename_all = "camelCase")]
    ApiKey {
        /// Google API key with YouTube Data API enabled
        key: String,
    },
    /// App OAuth - user authenticated via "Login with Google" flow
    #[serde(rename_all = "camelCase")]
    AppOAuth {
        /// Access token from OAuth flow
        #[serde(default)]
        access_token: String,
        /// Refresh token for renewal
        #[serde(skip_serializing_if = "Option::is_none")]
        refresh_token: Option<String>,
        /// Token expiration timestamp (Unix epoch seconds)
        #[serde(skip_serializing_if = "Option::is_none")]
        expires_at: Option<i64>,
    },
}

/// Connection status for a chat platform
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChatConnectionStatus {
    Disconnected,
    Connecting,
    Connected,
    Error,
}

/// Status information for a chat platform
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatPlatformStatus {
    pub platform: ChatPlatform,
    pub status: ChatConnectionStatus,
    pub message_count: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Result of sending a chat message to a platform
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatSendResult {
    pub platform: ChatPlatform,
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}
