//! Wire-mirror types for the chat surface — utoipa is transport-only,
//! so `ToSchema` lives on these mirrors rather than the core types.
//!
//! K5 split: pulled out of `v1/chat.rs` so the handlers file stays
//! under the 600 LOC ceiling. Same pattern as `v1/audit::AuditChainStatusWire`.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use spiritstream_core::models::{
    ChatConfig, ChatConnectionStatus, ChatCredentials, ChatLogStatus, ChatPlatform,
    ChatPlatformStatus, ChatSendResult, TwitchAuth, YouTubeAuth,
};

/// Mirror of [`ChatPlatform`] with `ToSchema`.
#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum ChatPlatformWire {
    Twitch,
    #[serde(rename = "tiktok")]
    TikTok,
    YouTube,
    Trovo,
    Kick,
    Facebook,
}

impl From<ChatPlatform> for ChatPlatformWire {
    fn from(value: ChatPlatform) -> Self {
        match value {
            ChatPlatform::Twitch => Self::Twitch,
            ChatPlatform::TikTok => Self::TikTok,
            ChatPlatform::YouTube => Self::YouTube,
            ChatPlatform::Trovo => Self::Trovo,
            ChatPlatform::Kick => Self::Kick,
            ChatPlatform::Facebook => Self::Facebook,
        }
    }
}

/// Mirror of [`ChatConnectionStatus`] with `ToSchema`.
#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum ChatConnectionStatusWire {
    Disconnected,
    Connecting,
    Connected,
    Error,
}

impl From<ChatConnectionStatus> for ChatConnectionStatusWire {
    fn from(value: ChatConnectionStatus) -> Self {
        match value {
            ChatConnectionStatus::Disconnected => Self::Disconnected,
            ChatConnectionStatus::Connecting => Self::Connecting,
            ChatConnectionStatus::Connected => Self::Connected,
            ChatConnectionStatus::Error => Self::Error,
        }
    }
}

/// Mirror of [`ChatPlatformStatus`] with `ToSchema`.
#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChatPlatformStatusWire {
    pub platform: ChatPlatformWire,
    pub status: ChatConnectionStatusWire,
    pub message_count: u64,
    pub error: Option<String>,
    // No `skip_serializing_if`: the ts-rs type is `number | null`
    // (non-optional), so the field must always be present — serialize
    // `null` when absent, matching `error` above. Omitting it would make
    // the runtime shape (`undefined`) disagree with the generated type.
    pub last_activity_ms: Option<i64>,
}

impl From<ChatPlatformStatus> for ChatPlatformStatusWire {
    fn from(value: ChatPlatformStatus) -> Self {
        Self {
            platform: value.platform.into(),
            status: value.status.into(),
            message_count: value.message_count,
            error: value.error,
            last_activity_ms: value.last_activity_ms,
        }
    }
}

/// Mirror of [`ChatSendResult`] with `ToSchema`.
#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChatSendResultWire {
    pub platform: ChatPlatformWire,
    pub success: bool,
    pub error: Option<String>,
    pub error_code: Option<String>,
}

impl From<ChatSendResult> for ChatSendResultWire {
    fn from(value: ChatSendResult) -> Self {
        Self {
            platform: value.platform.into(),
            success: value.success,
            error: value.error,
            error_code: value.error_code,
        }
    }
}

/// Mirror of [`ChatLogStatus`] with `ToSchema`. The core type already
/// has `serde` derives; the mirror exists purely to add `ToSchema`.
#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChatLogStatusWire {
    pub active: bool,
    pub started_at: i64,
}

impl From<ChatLogStatus> for ChatLogStatusWire {
    fn from(value: ChatLogStatus) -> Self {
        Self {
            active: value.active,
            started_at: value.started_at,
        }
    }
}

/// Empty 200 OK response — used for handlers whose success payload is
/// just acknowledgement (`connect`, `disconnect`, `retry`, `export`).
/// Serialises as `{}`.
#[derive(Serialize, Deserialize, ToSchema)]
pub struct ChatAckResponse {}

/// `GET /chat/connected` response.
#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChatConnectedResponse {
    pub connected: bool,
}

// ---------------------------------------------------------------------------
// G5: typed request body for POST /chat/connections. Replaces the prior
// `Json<serde_json::Value>` placeholder; wire shape is byte-identical to
// the ts-rs export at `@spiritstream/types/ChatConfig`.
// ---------------------------------------------------------------------------

/// Mirror of [`TwitchAuth`].
#[derive(Serialize, Deserialize, ToSchema)]
#[serde(tag = "method", rename_all = "camelCase")]
pub enum TwitchAuthWire {
    #[serde(rename_all = "camelCase")]
    UserToken { oauth_token: String },
    #[serde(rename_all = "camelCase")]
    AppOAuth {
        #[serde(default)]
        access_token: String,
        refresh_token: Option<String>,
        expires_at: Option<i64>,
    },
}

impl From<TwitchAuthWire> for TwitchAuth {
    fn from(w: TwitchAuthWire) -> Self {
        match w {
            TwitchAuthWire::UserToken { oauth_token } => Self::UserToken { oauth_token },
            TwitchAuthWire::AppOAuth {
                access_token,
                refresh_token,
                expires_at,
            } => Self::AppOAuth {
                access_token,
                refresh_token,
                expires_at,
            },
        }
    }
}

/// Mirror of [`YouTubeAuth`].
#[derive(Serialize, Deserialize, ToSchema)]
#[serde(tag = "method", rename_all = "camelCase")]
pub enum YouTubeAuthWire {
    #[serde(rename_all = "camelCase")]
    ApiKey { key: String },
    #[serde(rename_all = "camelCase")]
    AppOAuth {
        #[serde(default)]
        access_token: String,
        refresh_token: Option<String>,
        expires_at: Option<i64>,
    },
}

impl From<YouTubeAuthWire> for YouTubeAuth {
    fn from(w: YouTubeAuthWire) -> Self {
        match w {
            YouTubeAuthWire::ApiKey { key } => Self::ApiKey { key },
            YouTubeAuthWire::AppOAuth {
                access_token,
                refresh_token,
                expires_at,
            } => Self::AppOAuth {
                access_token,
                refresh_token,
                expires_at,
            },
        }
    }
}

/// Mirror of [`ChatCredentials`].
#[derive(Serialize, Deserialize, ToSchema)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ChatCredentialsWire {
    #[serde(rename_all = "camelCase")]
    Twitch {
        channel: String,
        auth: Option<TwitchAuthWire>,
    },
    #[serde(rename_all = "camelCase")]
    TikTok {
        username: String,
        session_token: Option<String>,
    },
    #[serde(rename_all = "camelCase")]
    YouTube {
        channel_id: String,
        auth: YouTubeAuthWire,
    },
    #[serde(rename_all = "camelCase")]
    Trovo {
        channel_id: String,
        #[serde(default)]
        oauth_token: Option<String>,
    },
    #[serde(rename_all = "camelCase")]
    Kick {
        channel: String,
        #[serde(default)]
        oauth_token: Option<String>,
        #[serde(default)]
        broadcaster_user_id: Option<u64>,
    },
    #[serde(rename_all = "camelCase")]
    Facebook {
        video_id: String,
        access_token: String,
    },
}

impl From<ChatCredentialsWire> for ChatCredentials {
    fn from(w: ChatCredentialsWire) -> Self {
        match w {
            ChatCredentialsWire::Twitch { channel, auth } => Self::Twitch {
                channel,
                auth: auth.map(Into::into),
            },
            ChatCredentialsWire::TikTok {
                username,
                session_token,
            } => Self::TikTok {
                username,
                session_token,
            },
            ChatCredentialsWire::YouTube { channel_id, auth } => Self::YouTube {
                channel_id,
                auth: auth.into(),
            },
            ChatCredentialsWire::Trovo {
                channel_id,
                oauth_token,
            } => Self::Trovo {
                channel_id,
                // Never wire-supplied: the connect handler resolves it
                // from the OAuth config after this conversion.
                client_id: None,
                oauth_token,
            },
            ChatCredentialsWire::Kick {
                channel,
                oauth_token,
                broadcaster_user_id,
            } => Self::Kick {
                channel,
                oauth_token,
                broadcaster_user_id,
            },
            ChatCredentialsWire::Facebook {
                video_id,
                access_token,
            } => Self::Facebook {
                video_id,
                access_token,
            },
        }
    }
}

/// Mirror of [`ChatConfig`] used as the request body for
/// `POST /chat/connections`. Wire shape matches the ts-rs export at
/// `@spiritstream/types/ChatConfig`.
#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChatConfigWire {
    pub platform: ChatPlatformWire,
    pub enabled: bool,
    pub credentials: ChatCredentialsWire,
}

impl From<ChatPlatformWire> for ChatPlatform {
    fn from(w: ChatPlatformWire) -> Self {
        match w {
            ChatPlatformWire::Twitch => Self::Twitch,
            ChatPlatformWire::TikTok => Self::TikTok,
            ChatPlatformWire::YouTube => Self::YouTube,
            ChatPlatformWire::Trovo => Self::Trovo,
            ChatPlatformWire::Kick => Self::Kick,
            ChatPlatformWire::Facebook => Self::Facebook,
        }
    }
}

impl From<ChatConfigWire> for ChatConfig {
    fn from(w: ChatConfigWire) -> Self {
        Self {
            platform: w.platform.into(),
            enabled: w.enabled,
            credentials: w.credentials.into(),
        }
    }
}
