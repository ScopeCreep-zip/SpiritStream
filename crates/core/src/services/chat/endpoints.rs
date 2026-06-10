//! Centralized chat-connector endpoint configuration.
//!
//! Every network endpoint a chat connector talks to lives here as a
//! single source of truth instead of being scattered as `const` string
//! literals across the individual connector files. The production
//! defaults (`ChatEndpoints::default`) are the real platform URLs; the
//! struct is injected through the connector factory
//! (`ChatManager::create_platform_connector`) so a connector never reads
//! a hardcoded endpoint and the whole network surface can be redirected
//! at one seam.
//!
//! This is deliberately **not** environment-overridable. Letting an env
//! var redirect chat or OAuth traffic would be an exfiltration vector
//! against the vulnerable users SpiritStream is built for (CLAUDE.md
//! threat model). The only non-default construction is
//! [`ChatEndpoints::for_mock`], gated to `#[cfg(test)]`, which points
//! every endpoint at a local mock server for the connector integration
//! harness.
//!
//! Two live endpoints are intentionally absent: the Twitch IRC ride
//! (handled inside the `twitch-irc` crate) and TikTok (inside
//! `piratetok-live-rs`). Neither crate exposes a server-address
//! override, so those endpoints cannot be centralized here or pointed at
//! a mock — see `docs/04-streaming/06-chat-platforms.md`.

/// Network endpoints for every chat connector SpiritStream controls.
///
/// Cloned into each connector at construction. Fields are a mix of full
/// URLs (used verbatim) and origin/prefix bases (the connector appends a
/// variable path segment) — each field is shaped to exactly how its
/// connector consumes it, so refactoring a connector is a literal
/// `const` → `self.field` swap.
#[derive(Clone, Debug)]
pub struct ChatEndpoints {
    /// Twitch GQL channel-lookup endpoint (full URL, no path variable).
    pub twitch_gql: String,
    /// Twitch OAuth token-validate endpoint (full URL, no path variable).
    pub twitch_validate: String,
    /// Twitch Helix chat-settings endpoint (full URL; the room-settings
    /// helper appends `?broadcaster_id=…&moderator_id=…`). Used to apply
    /// the profile's follower-only default at connect time.
    pub twitch_helix_chat_settings: String,
    /// Kick Pusher Channels WebSocket URL (scheme + host + query string).
    pub kick_pusher_ws: String,
    /// Kick public REST channel-lookup prefix; the channel slug is
    /// appended directly (`{prefix}{slug}`).
    pub kick_channel_lookup: String,
    /// Kick official REST chat-send endpoint (full URL).
    pub kick_send_chat: String,
    /// Trovo open-platform API origin; the connector appends
    /// `/openplatform/chat/channel-token/{channel_id}`.
    pub trovo_api_base: String,
    /// Trovo open-chat WebSocket URL.
    pub trovo_chat_ws: String,
    /// Facebook Graph API origin; the connector appends
    /// `/{version}/{video_id}/comments`.
    pub facebook_graph_base: String,
    /// YouTube Data API v3 base; the connector appends each resource path.
    pub youtube_api_base: String,
}

impl Default for ChatEndpoints {
    fn default() -> Self {
        Self {
            twitch_gql: "https://gql.twitch.tv/gql".to_string(),
            twitch_validate: "https://id.twitch.tv/oauth2/validate".to_string(),
            twitch_helix_chat_settings: "https://api.twitch.tv/helix/chat/settings".to_string(),
            kick_pusher_ws: "wss://ws-us2.pusher.com/app/eb1d5f283081a78b932c\
                ?protocol=7&client=spiritstream&version=8.4.0&flash=false"
                .to_string(),
            kick_channel_lookup: "https://kick.com/api/v2/channels/".to_string(),
            kick_send_chat: "https://api.kick.com/public/v1/chat".to_string(),
            trovo_api_base: "https://open-api.trovo.live".to_string(),
            trovo_chat_ws: "wss://open-chat.trovo.live/chat".to_string(),
            facebook_graph_base: "https://graph.facebook.com".to_string(),
            youtube_api_base: "https://www.googleapis.com/youtube/v3".to_string(),
        }
    }
}

#[cfg(test)]
impl ChatEndpoints {
    /// Rebase every endpoint onto local mock servers for the connector
    /// integration harness. `http_base` is a wiremock origin (e.g.
    /// `http://127.0.0.1:PORT`); `ws_base` is a mock WebSocket origin
    /// (e.g. `ws://127.0.0.1:PORT`). Paths are preserved so wiremock
    /// `Mock`s can match on them.
    pub fn for_mock(http_base: &str, ws_base: &str) -> Self {
        Self {
            twitch_gql: format!("{http_base}/gql"),
            twitch_validate: format!("{http_base}/oauth2/validate"),
            twitch_helix_chat_settings: format!("{http_base}/helix/chat/settings"),
            kick_pusher_ws: ws_base.to_string(),
            kick_channel_lookup: format!("{http_base}/api/v2/channels/"),
            kick_send_chat: format!("{http_base}/public/v1/chat"),
            trovo_api_base: http_base.to_string(),
            trovo_chat_ws: ws_base.to_string(),
            facebook_graph_base: http_base.to_string(),
            youtube_api_base: http_base.to_string(),
        }
    }
}
