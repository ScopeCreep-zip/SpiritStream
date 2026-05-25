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
}
