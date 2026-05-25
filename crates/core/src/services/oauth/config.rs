use serde::{Deserialize, Serialize};

// Embedded OAuth Client IDs (PKCE flow — no secrets needed for Twitch's
// implicit / for YouTube's PKCE). Placeholders are replaced at release
// time; runtime overrides via OAuthConfig or env vars take precedence.
const TWITCH_CLIENT_ID: &str = "TWITCH_CLIENT_ID_PLACEHOLDER";
const YOUTUBE_CLIENT_ID: &str = "YOUTUBE_CLIENT_ID_PLACEHOLDER";

/// User-provided OAuth credentials. Each field falls back through:
/// (1) explicit override on this struct, (2) env var, (3) embedded
/// placeholder constant.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct OAuthConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub twitch_client_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub twitch_client_secret: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub youtube_client_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub youtube_client_secret: Option<String>,
}

impl OAuthConfig {
    pub fn has_twitch(&self) -> bool {
        true
    }

    pub fn has_youtube(&self) -> bool {
        true
    }

    pub fn get_twitch_client_id(&self) -> String {
        if let Some(value) = self.twitch_client_id.as_deref() {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
        if let Ok(value) = std::env::var("SPIRITSTREAM_TWITCH_CLIENT_ID") {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
        TWITCH_CLIENT_ID.to_string()
    }

    pub fn get_twitch_client_secret(&self) -> Option<String> {
        if let Some(value) = self.twitch_client_secret.as_deref() {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
        if let Ok(value) = std::env::var("SPIRITSTREAM_TWITCH_CLIENT_SECRET") {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
        None
    }

    pub fn get_youtube_client_id(&self) -> String {
        if let Some(value) = self.youtube_client_id.as_deref() {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
        if let Ok(value) = std::env::var("SPIRITSTREAM_YOUTUBE_CLIENT_ID") {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
        YOUTUBE_CLIENT_ID.to_string()
    }

    /// Required for Google Desktop apps even with PKCE (non-standard).
    pub fn get_youtube_client_secret(&self) -> Option<String> {
        if let Some(value) = self.youtube_client_secret.as_deref() {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
        if let Ok(value) = std::env::var("SPIRITSTREAM_YOUTUBE_CLIENT_SECRET") {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
        None
    }
}
