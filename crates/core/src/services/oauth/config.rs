use serde::{Deserialize, Serialize};

// Embedded OAuth client credentials, injected at BUILD time by release
// CI via `SPIRITSTREAM_EMBEDDED_*` env vars (client IDs are public by
// design — every Chatterino/Firebot-class app embeds a maintainer-
// registered one; Google documents the installed-app client secret as
// "not treated as a secret"). Dev builds without the injection fall
// back to placeholders and honestly report the provider unconfigured —
// pre-fix, the placeholder rode straight into the authorize URL and
// "Sign in with Twitch" opened a Twitch 400 page.
//
// Per-provider secret requirements (2026 docs):
// - Twitch: PUBLIC client — no secret; sign-in uses the Device Code
//   Flow (Twitch does not support PKCE at the token exchange, so the
//   loopback code flow needs a confidential override to work at all).
// - Google: Desktop-app clients require the client_secret at exchange
//   even with PKCE (officially non-confidential).
// - Kick: no public-client option — secret required at exchange.
// - Facebook: App Secret required; App-Review-gated scopes.
// - Trovo: secret required at `exchangetoken`.
const TWITCH_CLIENT_ID: &str = match option_env!("SPIRITSTREAM_EMBEDDED_TWITCH_CLIENT_ID") {
    Some(v) => v,
    None => "TWITCH_CLIENT_ID_PLACEHOLDER",
};
const YOUTUBE_CLIENT_ID: &str = match option_env!("SPIRITSTREAM_EMBEDDED_YOUTUBE_CLIENT_ID") {
    Some(v) => v,
    None => "YOUTUBE_CLIENT_ID_PLACEHOLDER",
};
const YOUTUBE_CLIENT_SECRET: &str = match option_env!("SPIRITSTREAM_EMBEDDED_YOUTUBE_CLIENT_SECRET")
{
    Some(v) => v,
    None => "",
};
const KICK_CLIENT_ID: &str = match option_env!("SPIRITSTREAM_EMBEDDED_KICK_CLIENT_ID") {
    Some(v) => v,
    None => "KICK_CLIENT_ID_PLACEHOLDER",
};
const KICK_CLIENT_SECRET: &str = match option_env!("SPIRITSTREAM_EMBEDDED_KICK_CLIENT_SECRET") {
    Some(v) => v,
    None => "",
};
const FACEBOOK_CLIENT_ID: &str = match option_env!("SPIRITSTREAM_EMBEDDED_FACEBOOK_CLIENT_ID") {
    Some(v) => v,
    None => "FACEBOOK_CLIENT_ID_PLACEHOLDER",
};
const FACEBOOK_CLIENT_SECRET: &str =
    match option_env!("SPIRITSTREAM_EMBEDDED_FACEBOOK_CLIENT_SECRET") {
        Some(v) => v,
        None => "",
    };
const TROVO_CLIENT_ID: &str = match option_env!("SPIRITSTREAM_EMBEDDED_TROVO_CLIENT_ID") {
    Some(v) => v,
    None => "TROVO_CLIENT_ID_PLACEHOLDER",
};
const TROVO_CLIENT_SECRET: &str = match option_env!("SPIRITSTREAM_EMBEDDED_TROVO_CLIENT_SECRET") {
    Some(v) => v,
    None => "",
};

/// A credential is "real" when it's non-empty after trimming and not an
/// un-injected placeholder. This is the single predicate behind every
/// `has_*` flag — and therefore behind whether sign-in buttons render
/// as live or as "not set up in this build".
fn is_real(value: &str) -> bool {
    let trimmed = value.trim();
    !trimmed.is_empty() && !trimmed.ends_with("_PLACEHOLDER")
}

/// Resolution chain shared by every credential getter:
/// explicit struct override → runtime env var → embedded build value.
/// Whitespace-only entries at any tier fall through to the next.
fn resolve(explicit: Option<&str>, env_key: &str, embedded: &'static str) -> String {
    if let Some(value) = explicit {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    if let Ok(value) = std::env::var(env_key) {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    embedded.to_string()
}

fn resolve_secret(explicit: Option<&str>, env_key: &str, embedded: &'static str) -> Option<String> {
    let resolved = resolve(explicit, env_key, embedded);
    if resolved.is_empty() {
        None
    } else {
        Some(resolved)
    }
}

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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trovo_client_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trovo_client_secret: Option<String>,
}

impl OAuthConfig {
    /// Twitch ships as a PUBLIC client (Device Code Flow) — a real
    /// client id alone makes it usable; no secret involved.
    pub fn has_twitch(&self) -> bool {
        is_real(&self.get_twitch_client_id())
    }

    /// Google requires the (non-confidential) client secret at the
    /// token exchange, so "configured" means BOTH credentials are real.
    pub fn has_youtube(&self) -> bool {
        is_real(&self.get_youtube_client_id())
            && self.get_youtube_client_secret().as_deref().is_some_and(is_real)
    }

    /// Kick has no public-client option — secret required at exchange.
    pub fn has_kick(&self) -> bool {
        is_real(&self.get_kick_client_id())
            && self.get_kick_client_secret().as_deref().is_some_and(is_real)
    }

    pub fn has_facebook(&self) -> bool {
        is_real(&self.get_facebook_client_id())
            && self
                .get_facebook_client_secret()
                .as_deref()
                .is_some_and(is_real)
    }

    /// Trovo's `exchangetoken` requires the secret. The client id alone
    /// still powers READ-ONLY chat via the channel chat token — that
    /// path doesn't consult this flag.
    pub fn has_trovo(&self) -> bool {
        is_real(&self.get_trovo_client_id())
            && self.get_trovo_client_secret().as_deref().is_some_and(is_real)
    }

    pub fn get_twitch_client_id(&self) -> String {
        resolve(
            self.twitch_client_id.as_deref(),
            "SPIRITSTREAM_TWITCH_CLIENT_ID",
            TWITCH_CLIENT_ID,
        )
    }

    /// Power-user confidential override only — the shipped Twitch app
    /// is a public client and the Device Code Flow never needs this.
    pub fn get_twitch_client_secret(&self) -> Option<String> {
        resolve_secret(
            self.twitch_client_secret.as_deref(),
            "SPIRITSTREAM_TWITCH_CLIENT_SECRET",
            "",
        )
    }

    pub fn get_youtube_client_id(&self) -> String {
        resolve(
            self.youtube_client_id.as_deref(),
            "SPIRITSTREAM_YOUTUBE_CLIENT_ID",
            YOUTUBE_CLIENT_ID,
        )
    }

    /// Required for Google Desktop apps even with PKCE — Google's docs:
    /// "the client secret is obviously not treated as a secret" for
    /// installed apps, hence the embedded-at-release tier.
    pub fn get_youtube_client_secret(&self) -> Option<String> {
        resolve_secret(
            self.youtube_client_secret.as_deref(),
            "SPIRITSTREAM_YOUTUBE_CLIENT_SECRET",
            YOUTUBE_CLIENT_SECRET,
        )
    }

    pub fn get_kick_client_id(&self) -> String {
        resolve(
            self.kick_client_id.as_deref(),
            "SPIRITSTREAM_KICK_CLIENT_ID",
            KICK_CLIENT_ID,
        )
    }

    /// Kick requires the client secret at the token exchange — there is
    /// no public-client registration option on Kick's developer portal.
    pub fn get_kick_client_secret(&self) -> Option<String> {
        resolve_secret(
            self.kick_client_secret.as_deref(),
            "SPIRITSTREAM_KICK_CLIENT_SECRET",
            KICK_CLIENT_SECRET,
        )
    }

    pub fn get_facebook_client_id(&self) -> String {
        resolve(
            self.facebook_client_id.as_deref(),
            "SPIRITSTREAM_FACEBOOK_CLIENT_ID",
            FACEBOOK_CLIENT_ID,
        )
    }

    /// Facebook OAuth requires the App Secret for the token exchange —
    /// Meta does not support PKCE for web apps.
    pub fn get_facebook_client_secret(&self) -> Option<String> {
        resolve_secret(
            self.facebook_client_secret.as_deref(),
            "SPIRITSTREAM_FACEBOOK_CLIENT_SECRET",
            FACEBOOK_CLIENT_SECRET,
        )
    }

    /// Doubles as the chat-token client id the Trovo connector uses for
    /// read-only chat (same env var, one registration).
    pub fn get_trovo_client_id(&self) -> String {
        resolve(
            self.trovo_client_id.as_deref(),
            "SPIRITSTREAM_TROVO_CLIENT_ID",
            TROVO_CLIENT_ID,
        )
    }

    pub fn get_trovo_client_secret(&self) -> Option<String> {
        resolve_secret(
            self.trovo_client_secret.as_deref(),
            "SPIRITSTREAM_TROVO_CLIENT_SECRET",
            TROVO_CLIENT_SECRET,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env_unset(key: &str) -> bool {
        std::env::var(key).is_err()
    }

    #[test]
    fn placeholder_credentials_report_unconfigured() {
        // Dev builds (no SPIRITSTREAM_EMBEDDED_* injection, no env) must
        // say so — the dead-link bug was these flags lying `true`.
        let c = OAuthConfig::default();
        if env_unset("SPIRITSTREAM_TWITCH_CLIENT_ID") {
            assert!(!c.has_twitch(), "placeholder twitch id must be unconfigured");
        }
        if env_unset("SPIRITSTREAM_YOUTUBE_CLIENT_ID") || env_unset("SPIRITSTREAM_YOUTUBE_CLIENT_SECRET") {
            assert!(!c.has_youtube());
        }
        if env_unset("SPIRITSTREAM_KICK_CLIENT_ID") || env_unset("SPIRITSTREAM_KICK_CLIENT_SECRET") {
            assert!(!c.has_kick());
        }
        if env_unset("SPIRITSTREAM_FACEBOOK_CLIENT_ID") {
            assert!(!c.has_facebook());
        }
        if env_unset("SPIRITSTREAM_TROVO_CLIENT_SECRET") {
            assert!(!c.has_trovo());
        }
    }

    #[test]
    fn real_overrides_flip_configured_true() {
        let c = OAuthConfig {
            twitch_client_id: Some("real-tw-id".into()),
            youtube_client_id: Some("real-yt-id".into()),
            youtube_client_secret: Some("real-yt-secret".into()),
            kick_client_id: Some("real-kick-id".into()),
            kick_client_secret: Some("real-kick-secret".into()),
            facebook_client_id: Some("real-fb-id".into()),
            facebook_client_secret: Some("real-fb-secret".into()),
            trovo_client_id: Some("real-trovo-id".into()),
            trovo_client_secret: Some("real-trovo-secret".into()),
            ..OAuthConfig::default()
        };
        assert!(c.has_twitch());
        assert!(c.has_youtube());
        assert!(c.has_kick());
        assert!(c.has_facebook());
        assert!(c.has_trovo());
    }

    #[test]
    fn twitch_needs_only_a_client_id_but_secret_providers_need_both() {
        // Twitch is a public client; an id alone is fully usable.
        let twitch_only_id = OAuthConfig {
            twitch_client_id: Some("real-tw-id".into()),
            ..OAuthConfig::default()
        };
        assert!(twitch_only_id.has_twitch());
        // Kick with an id but no secret is NOT usable (exchange fails).
        let kick_only_id = OAuthConfig {
            kick_client_id: Some("real-kick-id".into()),
            ..OAuthConfig::default()
        };
        if env_unset("SPIRITSTREAM_KICK_CLIENT_SECRET") {
            assert!(!kick_only_id.has_kick());
        }
    }

    #[test]
    fn is_real_rejects_placeholders_and_blanks() {
        assert!(!is_real(""));
        assert!(!is_real("   "));
        assert!(!is_real("TWITCH_CLIENT_ID_PLACEHOLDER"));
        assert!(is_real("a1b2c3"));
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
