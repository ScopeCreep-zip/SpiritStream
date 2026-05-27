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

    /// Facebook OAuth 2.0 (Meta Graph API). Scopes the plan enumerated
    /// for the Live Video Comments use case, plus `pages_show_list`
    /// which the connector needs to enumerate the user's Pages and
    /// resolve which Page Access Token to use for a given live video:
    ///   - `publish_video` — required to manage / interact with Live
    ///     Video objects (per Meta's Live Video API gate).
    ///   - `pages_read_engagement` — read comments on Page content.
    ///   - `pages_manage_posts` — post + edit comments on Page content
    ///     (including live-video comments).
    ///   - `pages_show_list` — enumerate the user's Pages so the UI
    ///     can pick which Page Access Token to use.
    ///
    /// All four scopes are App-Review-gated for production: the
    /// maintainer's Meta App must be approved before the OAuth flow
    /// returns a Page-scoped token. Until App Review completes, the
    /// operator can paste a long-lived Page Access Token directly into
    /// `oauth.facebook.access_token` via settings — the connector path
    /// reads the same field regardless of how the token was obtained.
    pub fn facebook() -> Self {
        Self {
            name: "facebook",
            auth_url: "https://www.facebook.com/v18.0/dialog/oauth",
            token_url: "https://graph.facebook.com/v18.0/oauth/access_token",
            scopes: vec![
                "publish_video",
                "pages_read_engagement",
                "pages_manage_posts",
                "pages_show_list",
            ],
        }
    }
}
