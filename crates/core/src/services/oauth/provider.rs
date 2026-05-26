/// OAuth provider endpoint configuration.
#[derive(Debug, Clone)]
pub struct OAuthProvider {
    pub name: &'static str,
    pub auth_url: &'static str,
    pub token_url: &'static str,
    pub scopes: Vec<&'static str>,
}

impl OAuthProvider {
    pub fn twitch() -> Self {
        Self {
            name: "twitch",
            auth_url: "https://id.twitch.tv/oauth2/authorize",
            token_url: "https://id.twitch.tv/oauth2/token",
            scopes: vec!["chat:read", "chat:edit"],
        }
    }

    pub fn youtube() -> Self {
        Self {
            name: "youtube",
            auth_url: "https://accounts.google.com/o/oauth2/v2/auth",
            token_url: "https://oauth2.googleapis.com/token",
            scopes: vec!["https://www.googleapis.com/auth/youtube.force-ssl"],
        }
    }

    /// Kick OAuth 2.1 + PKCE (mandatory for Kick; no implicit flow).
    /// Endpoints from id.kick.com/.well-known/openid-configuration —
    /// Kick uses its own identity host separate from api.kick.com.
    /// `chat:write` is needed for outbound chat messages; `user:read`
    /// is needed to resolve the broadcaster user id.
    pub fn kick() -> Self {
        Self {
            name: "kick",
            auth_url: "https://id.kick.com/oauth/authorize",
            token_url: "https://id.kick.com/oauth/token",
            scopes: vec!["user:read", "chat:write"],
        }
    }
}
