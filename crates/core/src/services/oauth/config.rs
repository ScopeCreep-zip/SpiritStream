use serde::{Deserialize, Serialize};

// Embedded OAuth Client IDs (PKCE flow — no secrets needed for Twitch's
// implicit / for YouTube's PKCE). Placeholders are replaced at release
// time; runtime overrides via OAuthConfig or env vars take precedence.
const TWITCH_CLIENT_ID: &str = "TWITCH_CLIENT_ID_PLACEHOLDER";
const YOUTUBE_CLIENT_ID: &str = "YOUTUBE_CLIENT_ID_PLACEHOLDER";
const KICK_CLIENT_ID: &str = "KICK_CLIENT_ID_PLACEHOLDER";
const FACEBOOK_CLIENT_ID: &str = "FACEBOOK_CLIENT_ID_PLACEHOLDER";

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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kick_client_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kick_client_secret: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub facebook_client_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub facebook_client_secret: Option<String>,
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

    pub fn has_kick(&self) -> bool {
        true
    }

    pub fn get_kick_client_id(&self) -> String {
        if let Some(value) = self.kick_client_id.as_deref() {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
        if let Ok(value) = std::env::var("SPIRITSTREAM_KICK_CLIENT_ID") {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
        KICK_CLIENT_ID.to_string()
    }

    /// Kick OAuth requires the client secret for confidential clients;
    /// public PKCE clients can omit it. Same fall-through chain as the
    /// other providers.
    pub fn get_kick_client_secret(&self) -> Option<String> {
        if let Some(value) = self.kick_client_secret.as_deref() {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
        if let Ok(value) = std::env::var("SPIRITSTREAM_KICK_CLIENT_SECRET") {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
        None
    }

    pub fn has_facebook(&self) -> bool {
        true
    }

    pub fn get_facebook_client_id(&self) -> String {
        if let Some(value) = self.facebook_client_id.as_deref() {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
        if let Ok(value) = std::env::var("SPIRITSTREAM_FACEBOOK_CLIENT_ID") {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
        FACEBOOK_CLIENT_ID.to_string()
    }

    /// Facebook OAuth requires the App Secret for the token exchange —
    /// Meta does not support PKCE for web apps. Same fall-through chain
    /// as the other providers.
    pub fn get_facebook_client_secret(&self) -> Option<String> {
        if let Some(value) = self.facebook_client_secret.as_deref() {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
        if let Ok(value) = std::env::var("SPIRITSTREAM_FACEBOOK_CLIENT_SECRET") {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env_unset(key: &str) -> bool {
        std::env::var(key).is_err()
    }

    #[test]
    fn has_provider_flags_are_always_true() {
        let c = OAuthConfig::default();
        assert!(c.has_twitch());
        assert!(c.has_youtube());
        assert!(c.has_kick());
        assert!(c.has_facebook());
    }

    #[test]
    fn explicit_client_id_override_is_trimmed_and_returned() {
        let c = OAuthConfig {
            twitch_client_id: Some("  tw-id  ".into()),
            youtube_client_id: Some("yt-id".into()),
            kick_client_id: Some("kick-id".into()),
            facebook_client_id: Some("fb-id".into()),
            ..OAuthConfig::default()
        };
        assert_eq!(c.get_twitch_client_id(), "tw-id");
        assert_eq!(c.get_youtube_client_id(), "yt-id");
        assert_eq!(c.get_kick_client_id(), "kick-id");
        assert_eq!(c.get_facebook_client_id(), "fb-id");
    }

    #[test]
    fn explicit_client_secret_override_is_trimmed_and_returned() {
        let c = OAuthConfig {
            twitch_client_secret: Some("  tw-secret  ".into()),
            youtube_client_secret: Some("yt-secret".into()),
            kick_client_secret: Some("kick-secret".into()),
            facebook_client_secret: Some("fb-secret".into()),
            ..OAuthConfig::default()
        };
        assert_eq!(c.get_twitch_client_secret().as_deref(), Some("tw-secret"));
        assert_eq!(c.get_youtube_client_secret().as_deref(), Some("yt-secret"));
        assert_eq!(c.get_kick_client_secret().as_deref(), Some("kick-secret"));
        assert_eq!(c.get_facebook_client_secret().as_deref(), Some("fb-secret"));
    }

    #[test]
    fn whitespace_only_override_falls_through_to_non_empty_value() {
        let c = OAuthConfig {
            twitch_client_id: Some("   ".into()),
            ..OAuthConfig::default()
        };
        let resolved = c.get_twitch_client_id();
        // Whitespace-only override is ignored; resolution continues to env
        // or the embedded placeholder — never the blank string itself.
        assert!(!resolved.is_empty());
        assert_ne!(resolved, "   ");
    }

    #[test]
    fn client_id_falls_back_to_placeholder_when_unset() {
        // Only meaningful when the CI/dev shell hasn't injected real IDs.
        let c = OAuthConfig::default();
        if env_unset("SPIRITSTREAM_TWITCH_CLIENT_ID") {
            assert_eq!(c.get_twitch_client_id(), "TWITCH_CLIENT_ID_PLACEHOLDER");
        }
        if env_unset("SPIRITSTREAM_YOUTUBE_CLIENT_ID") {
            assert_eq!(c.get_youtube_client_id(), "YOUTUBE_CLIENT_ID_PLACEHOLDER");
        }
        if env_unset("SPIRITSTREAM_KICK_CLIENT_ID") {
            assert_eq!(c.get_kick_client_id(), "KICK_CLIENT_ID_PLACEHOLDER");
        }
        if env_unset("SPIRITSTREAM_FACEBOOK_CLIENT_ID") {
            assert_eq!(c.get_facebook_client_id(), "FACEBOOK_CLIENT_ID_PLACEHOLDER");
        }
    }

    #[test]
    fn client_secret_defaults_to_none_when_unset() {
        let c = OAuthConfig::default();
        if env_unset("SPIRITSTREAM_TWITCH_CLIENT_SECRET") {
            assert!(c.get_twitch_client_secret().is_none());
        }
        if env_unset("SPIRITSTREAM_YOUTUBE_CLIENT_SECRET") {
            assert!(c.get_youtube_client_secret().is_none());
        }
        if env_unset("SPIRITSTREAM_KICK_CLIENT_SECRET") {
            assert!(c.get_kick_client_secret().is_none());
        }
        if env_unset("SPIRITSTREAM_FACEBOOK_CLIENT_SECRET") {
            assert!(c.get_facebook_client_secret().is_none());
        }
    }
}
