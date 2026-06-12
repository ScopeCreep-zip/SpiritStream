/// Pinned Meta Graph API version, shared by the OAuth dialog/token
/// URLs and the Facebook chat connector. Meta expires versions on a
/// ~2-year cycle — bump deliberately (pinned-deps rule), in ONE place.
pub const FACEBOOK_GRAPH_VERSION: &str = "v24.0";

/// OAuth provider endpoint configuration. Owned `String`s (not
/// `&'static str`) so tests can rebase endpoints onto a wiremock
/// server via `OAuthService::override_provider` — same philosophy as
/// `ChatEndpoints::for_mock`.
#[derive(Debug, Clone)]
pub struct OAuthProvider {
    pub name: String,
    pub auth_url: String,
    pub token_url: String,
    /// RFC 8628 device-authorization endpoint — only providers that
    /// support the Device Code Flow set this (Twitch, which MANDATES
    /// it for desktop-class public clients).
    pub device_url: Option<String>,
    /// Bearer-identified user/channel lookup (per-provider shape).
    pub user_info_url: String,
    pub scopes: Vec<&'static str>,
}

impl OAuthProvider {
    pub fn twitch() -> Self {
        Self {
            name: "twitch".into(),
            auth_url: "https://id.twitch.tv/oauth2/authorize".into(),
            token_url: "https://id.twitch.tv/oauth2/token".into(),
            device_url: Some("https://id.twitch.tv/oauth2/device".into()),
            user_info_url: "https://api.twitch.tv/helix/users".into(),
            // `moderator:manage:chat_settings` powers the safety
            // wizard's follower-only default (Helix PATCH
            // /helix/chat/settings at chat-connect time). Tokens
            // granted before this scope existed fail the scope probe
            // and surface a `follower_only_unsupported` event telling
            // the user to reconnect Twitch.
            //
            // Scope-migration note: `chat:read`/`chat:edit` are the
            // legacy-IRC scopes (current connector). Twitch's
            // new-integration guidance is EventSub chat with
            // `user:read:chat`/`user:write:chat` — switch when the
            // connector migrates off IRC.
            scopes: vec!["chat:read", "chat:edit", "moderator:manage:chat_settings"],
        }
    }

    pub fn youtube() -> Self {
        Self {
            name: "youtube".into(),
            auth_url: "https://accounts.google.com/o/oauth2/v2/auth".into(),
            token_url: "https://oauth2.googleapis.com/token".into(),
            // Google's limited-input device flow does not allow the
            // youtube.force-ssl scope — loopback + PKCE stays.
            device_url: None,
            user_info_url: "https://www.googleapis.com/youtube/v3/channels".into(),
            scopes: vec!["https://www.googleapis.com/auth/youtube.force-ssl"],
        }
    }

    /// Kick OAuth 2.1 + PKCE (mandatory for Kick; no implicit flow).
    /// Endpoints from id.kick.com/.well-known/openid-configuration —
    /// Kick uses its own identity host separate from api.kick.com.
    /// `chat:write` is needed for outbound chat messages; `user:read`
    /// is needed to resolve the broadcaster user id. NOTE: Kick has no
    /// public-client registration — the client secret is required at
    /// the token exchange even with PKCE.
    pub fn kick() -> Self {
        Self {
            name: "kick".into(),
            auth_url: "https://id.kick.com/oauth/authorize".into(),
            token_url: "https://id.kick.com/oauth/token".into(),
            device_url: None,
            user_info_url: "https://api.kick.com/public/v1/users".into(),
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
            name: "facebook".into(),
            auth_url: format!("https://www.facebook.com/{FACEBOOK_GRAPH_VERSION}/dialog/oauth"),
            token_url: format!(
                "https://graph.facebook.com/{FACEBOOK_GRAPH_VERSION}/oauth/access_token"
            ),
            device_url: None,
            user_info_url: format!("https://graph.facebook.com/{FACEBOOK_GRAPH_VERSION}/me"),
            scopes: vec![
                "publish_video",
                "pages_read_engagement",
                "pages_manage_posts",
                "pages_show_list",
            ],
        }
    }
}
