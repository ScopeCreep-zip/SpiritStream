//! Chat credential surface — `ChatConfig`, per-platform `ChatCredentials`,
//! and the provider-specific auth enums (`TwitchAuth`, `YouTubeAuth`).
//!
//! Pulled out of `models/chat.rs` (K2) to keep the chat module under the
//! 600 LOC ceiling. Pure data with hand-written `Debug` impls (F5) so
//! tokens never reach log lines.
//!
//! ts-rs `export_to` paths point four directories up from this file
//! (`crates/core/src/models/chat/`) to reach `packages/types/`.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::ChatPlatform;

/// Configuration for connecting a chat platform.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../../packages/types/src/generated/")]
pub struct ChatConfig {
    /// Platform to connect to
    pub platform: ChatPlatform,
    /// Whether this platform is enabled
    pub enabled: bool,
    /// Platform-specific configuration
    pub credentials: ChatCredentials,
}

/// Platform-specific credentials.
///
/// `Debug` is hand-written (F5) so OAuth tokens, session cookies, and
/// API keys never reach a panic message, log line, or `dbg!()`
/// rendering. Serde is unaffected.
#[derive(Clone, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "lowercase")]
#[ts(export, export_to = "../../../../packages/types/src/generated/")]
pub enum ChatCredentials {
    #[serde(rename_all = "camelCase")]
    Twitch {
        /// Twitch channel name to join
        channel: String,
        /// Authentication method (optional - anonymous read-only if not provided)
        auth: Option<TwitchAuth>,
    },
    #[serde(rename_all = "camelCase")]
    TikTok {
        /// TikTok username to monitor
        username: String,
        /// Session cookies/token (may be needed for some unofficial APIs)
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
        /// OAuth bearer (`Authorization: OAuth <token>` — Trovo's
        /// scheme) for the signed-in account. `None` = read-only chat
        /// via the client-id-only channel chat token; `Some(token)`
        /// enables send through `openplatform/chat/send`
        /// (`chat_send_self` scope).
        #[serde(default)]
        oauth_token: Option<String>,
    },
    #[serde(rename_all = "camelCase")]
    Kick {
        /// Kick channel name (case-insensitive — Kick normalises internally).
        channel: String,
        /// OAuth bearer for the user account doing the chatting. `None`
        /// means read-only (anonymous Pusher subscription works without
        /// auth); `Some(token)` enables send via `api.kick.com/public/v1/chat`.
        #[serde(default)]
        oauth_token: Option<String>,
        /// Kick broadcaster user id (Kick's REST POST /chat expects the
        /// numeric broadcaster id, NOT the username). The chat lifecycle
        /// fetches this once when activating the profile.
        #[serde(default)]
        broadcaster_user_id: Option<u64>,
    },
    #[serde(rename_all = "camelCase")]
    Facebook {
        /// Facebook Live video ID
        video_id: String,
        /// Facebook access token
        access_token: String,
    },
}

/// Twitch authentication options.
///
/// `Debug` is hand-written (F5) so OAuth bearers and refresh tokens
/// never reach a log line via the default derive. Serde is unaffected.
#[derive(Clone, Serialize, Deserialize, TS)]
#[serde(tag = "method", rename_all = "camelCase")]
#[ts(export, export_to = "../../../../packages/types/src/generated/")]
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
        refresh_token: Option<String>,
        /// Token expiration timestamp (Unix epoch seconds, JSON `number`).
        #[ts(type = "number | null")]
        expires_at: Option<i64>,
    },
}

/// YouTube authentication options.
///
/// `Debug` is hand-written (F5) so API keys and OAuth tokens never
/// reach a log line via the default derive. Serde is unaffected.
#[derive(Clone, Serialize, Deserialize, TS)]
#[serde(tag = "method", rename_all = "camelCase")]
#[ts(export, export_to = "../../../../packages/types/src/generated/")]
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
        refresh_token: Option<String>,
        /// Token expiration timestamp (Unix epoch seconds, JSON `number`).
        #[ts(type = "number | null")]
        expires_at: Option<i64>,
    },
}

// F5: hand-written Debug impls for secret-bearing credential enums.
// Default derives would print tokens; these emit `<redacted>` markers
// instead while leaving the non-secret discriminator + channel fields
// visible for diagnostics.

impl std::fmt::Debug for ChatCredentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChatCredentials::Twitch { channel, auth } => f
                .debug_struct("ChatCredentials::Twitch")
                .field("channel", channel)
                .field("auth", auth)
                .finish(),
            ChatCredentials::TikTok {
                username,
                session_token,
            } => f
                .debug_struct("ChatCredentials::TikTok")
                .field("username", username)
                .field(
                    "session_token",
                    &session_token.as_ref().map(|_| "<redacted>"),
                )
                .finish(),
            ChatCredentials::YouTube { channel_id, auth } => f
                .debug_struct("ChatCredentials::YouTube")
                .field("channel_id", channel_id)
                .field("auth", auth)
                .finish(),
            ChatCredentials::Trovo {
                channel_id,
                oauth_token,
            } => f
                .debug_struct("ChatCredentials::Trovo")
                .field("channel_id", channel_id)
                .field("oauth_token", &oauth_token.as_ref().map(|_| "<redacted>"))
                .finish(),
            ChatCredentials::Kick {
                channel,
                oauth_token,
                broadcaster_user_id,
            } => f
                .debug_struct("ChatCredentials::Kick")
                .field("channel", channel)
                .field("oauth_token", &oauth_token.as_ref().map(|_| "<redacted>"))
                .field("broadcaster_user_id", broadcaster_user_id)
                .finish(),
            ChatCredentials::Facebook {
                video_id,
                access_token: _,
            } => f
                .debug_struct("ChatCredentials::Facebook")
                .field("video_id", video_id)
                .field("access_token", &"<redacted>")
                .finish(),
        }
    }
}

impl std::fmt::Debug for TwitchAuth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TwitchAuth::UserToken { oauth_token: _ } => f
                .debug_struct("TwitchAuth::UserToken")
                .field("oauth_token", &"<redacted>")
                .finish(),
            TwitchAuth::AppOAuth {
                access_token: _,
                refresh_token,
                expires_at,
            } => f
                .debug_struct("TwitchAuth::AppOAuth")
                .field("access_token", &"<redacted>")
                .field(
                    "refresh_token",
                    &refresh_token.as_ref().map(|_| "<redacted>"),
                )
                .field("expires_at", expires_at)
                .finish(),
        }
    }
}

impl std::fmt::Debug for YouTubeAuth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            YouTubeAuth::ApiKey { key: _ } => f
                .debug_struct("YouTubeAuth::ApiKey")
                .field("key", &"<redacted>")
                .finish(),
            YouTubeAuth::AppOAuth {
                access_token: _,
                refresh_token,
                expires_at,
            } => f
                .debug_struct("YouTubeAuth::AppOAuth")
                .field("access_token", &"<redacted>")
                .field(
                    "refresh_token",
                    &refresh_token.as_ref().map(|_| "<redacted>"),
                )
                .field("expires_at", expires_at)
                .finish(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// F5 regression: `Debug` rendering of a credentials variant
    /// holding a token must NOT contain the raw token. Pre-F5, the
    /// derived Debug printed the bearer verbatim.
    #[test]
    fn debug_redacts_chat_credentials_facebook_access_token() {
        let creds = ChatCredentials::Facebook {
            video_id: "vid-1".into(),
            access_token: "SUPER-SECRET-FACEBOOK-BEARER".into(),
        };
        let rendered = format!("{creds:?}");
        assert!(
            !rendered.contains("SUPER-SECRET-FACEBOOK-BEARER"),
            "Facebook access_token leaked via Debug: {rendered}",
        );
        assert!(
            rendered.contains("vid-1"),
            "non-secret field dropped: {rendered}"
        );
        assert!(rendered.contains("<redacted>"));
    }

    #[test]
    fn debug_redacts_twitch_user_token() {
        let auth = TwitchAuth::UserToken {
            oauth_token: "SUPER-SECRET-TWITCH-OAUTH".into(),
        };
        let rendered = format!("{auth:?}");
        assert!(!rendered.contains("SUPER-SECRET-TWITCH-OAUTH"));
        assert!(rendered.contains("<redacted>"));
    }

    #[test]
    fn debug_redacts_youtube_apikey() {
        let auth = YouTubeAuth::ApiKey {
            key: "SUPER-SECRET-YOUTUBE-API-KEY".into(),
        };
        let rendered = format!("{auth:?}");
        assert!(!rendered.contains("SUPER-SECRET-YOUTUBE-API-KEY"));
        assert!(rendered.contains("<redacted>"));
    }

    #[test]
    fn debug_redacts_kick_oauth_token() {
        let creds = ChatCredentials::Kick {
            channel: "kingbob".into(),
            oauth_token: Some("SUPER-SECRET-KICK-OAUTH".into()),
            broadcaster_user_id: Some(123),
        };
        let rendered = format!("{creds:?}");
        assert!(!rendered.contains("SUPER-SECRET-KICK-OAUTH"));
        assert!(
            rendered.contains("kingbob"),
            "non-secret channel name dropped"
        );
    }

    /// F5: the `AppOAuth` Twitch arm holds BOTH an access token and a
    /// refresh token. Rendering `ChatCredentials::Twitch` must redact both
    /// while keeping the channel name and the non-secret expiry visible.
    /// Exercises the outer `ChatCredentials::Twitch` arm and the inner
    /// `TwitchAuth::AppOAuth` arm together.
    #[test]
    fn debug_redacts_twitch_appoauth_access_and_refresh() {
        let creds = ChatCredentials::Twitch {
            channel: "streamer42".into(),
            auth: Some(TwitchAuth::AppOAuth {
                access_token: "SECRET-TWITCH-ACCESS".into(),
                refresh_token: Some("SECRET-TWITCH-REFRESH".into()),
                expires_at: Some(1_700_000_000),
            }),
        };
        let rendered = format!("{creds:?}");
        assert!(!rendered.contains("SECRET-TWITCH-ACCESS"), "{rendered}");
        assert!(!rendered.contains("SECRET-TWITCH-REFRESH"), "{rendered}");
        assert!(
            rendered.contains("streamer42"),
            "channel dropped: {rendered}"
        );
        assert!(
            rendered.contains("1700000000"),
            "expiry dropped: {rendered}"
        );
    }

    /// F5: same dual-token guarantee for the YouTube `AppOAuth` arm, routed
    /// through the outer `ChatCredentials::YouTube` arm.
    #[test]
    fn debug_redacts_youtube_appoauth_access_and_refresh() {
        let creds = ChatCredentials::YouTube {
            channel_id: "UCabc123".into(),
            auth: YouTubeAuth::AppOAuth {
                access_token: "SECRET-YT-ACCESS".into(),
                refresh_token: Some("SECRET-YT-REFRESH".into()),
                expires_at: Some(1_700_000_000),
            },
        };
        let rendered = format!("{creds:?}");
        assert!(!rendered.contains("SECRET-YT-ACCESS"), "{rendered}");
        assert!(!rendered.contains("SECRET-YT-REFRESH"), "{rendered}");
        assert!(
            rendered.contains("UCabc123"),
            "channel_id dropped: {rendered}"
        );
    }

    /// F5: TikTok's optional `session_token` is a credential — it must be
    /// redacted, while the public username stays visible.
    #[test]
    fn debug_redacts_tiktok_session_token() {
        let creds = ChatCredentials::TikTok {
            username: "tikuser".into(),
            session_token: Some("SECRET-TIKTOK-SESSION".into()),
        };
        let rendered = format!("{creds:?}");
        assert!(!rendered.contains("SECRET-TIKTOK-SESSION"), "{rendered}");
        assert!(rendered.contains("tikuser"), "username dropped: {rendered}");
        assert!(rendered.contains("<redacted>"));
    }

    /// Trovo carries no secret — its Debug arm just renders the channel id.
    /// Guards against a future field addition silently going unredacted.
    #[test]
    fn debug_trovo_renders_channel_id() {
        let creds = ChatCredentials::Trovo {
            channel_id: "1234567".into(),
            oauth_token: None,
        };
        let rendered = format!("{creds:?}");
        assert!(
            rendered.contains("1234567"),
            "channel_id dropped: {rendered}"
        );
    }
}
