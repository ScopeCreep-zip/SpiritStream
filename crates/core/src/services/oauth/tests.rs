use super::{OAuthConfig, OAuthProvider, OAuthService};

fn svc() -> OAuthService {
    OAuthService::new(OAuthConfig::default())
}

/// Service with REAL (test) credentials for every provider — flow tests
/// must get past the configured pre-flight guard; the default config's
/// placeholders honestly refuse to start a flow.
fn configured_svc() -> OAuthService {
    OAuthService::new(configured_cfg())
}

fn configured_cfg() -> OAuthConfig {
    OAuthConfig {
        twitch_client_id: Some("test-twitch-id".into()),
        youtube_client_id: Some("test-yt-id".into()),
        youtube_client_secret: Some("test-yt-secret".into()),
        kick_client_id: Some("test-kick-id".into()),
        kick_client_secret: Some("test-kick-secret".into()),
        facebook_client_id: Some("test-fb-id".into()),
        facebook_client_secret: Some("test-fb-secret".into()),
        trovo_client_id: Some("test-trovo-id".into()),
        trovo_client_secret: Some("test-trovo-secret".into()),
        ..OAuthConfig::default()
    }
}

/// `expires_at == 0` means "no expiry recorded" — must NOT trigger
/// refresh, otherwise freshly-stored profiles would churn refresh
/// requests on every API call.
#[test]
fn zero_expires_at_never_needs_refresh() {
    let s = svc();
    assert!(!s.token_needs_refresh(0, 300));
}

/// A token expiring inside the leeway window must trigger refresh; one
/// outside the window must not.
#[test]
fn token_inside_leeway_window_needs_refresh() {
    let s = svc();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    assert!(
        s.token_needs_refresh(now + 60, 300),
        "60s away within 300s leeway"
    );
    assert!(
        !s.token_needs_refresh(now + 600, 300),
        "600s away outside 300s leeway"
    );
}

/// `refresh_if_expiring` returns `Unauthorized` when an expiring token
/// has no refresh_token to spend — transports map this to 401 so the UI
/// re-prompts login instead of silently retrying.
#[tokio::test]
async fn refresh_if_expiring_without_refresh_token_returns_unauthorized() {
    let s = svc();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    let result = s.refresh_if_expiring("twitch", "", now + 60, 300).await;
    match result {
        Err(crate::errors::CoreError::Unauthorized) => {}
        other => panic!("expected Unauthorized, got {other:?}"),
    }
}

/// `refresh_if_expiring` is a no-op when the token is still valid.
#[tokio::test]
async fn refresh_if_expiring_skips_when_token_still_valid() {
    let s = svc();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    // 1 hour out, 5 minute leeway → don't refresh.
    let result = s
        .refresh_if_expiring("twitch", "any", now + 3600, 300)
        .await;
    assert!(matches!(result, Ok(None)));
}

// ===== provider endpoint metadata =====

#[test]
fn provider_twitch_metadata() {
    let p = OAuthProvider::twitch();
    assert_eq!(p.name, "twitch");
    assert_eq!(p.auth_url, "https://id.twitch.tv/oauth2/authorize");
    assert_eq!(p.token_url, "https://id.twitch.tv/oauth2/token");
    assert!(p.scopes.contains(&"chat:read"));
    assert!(p.scopes.contains(&"chat:edit"));
}

#[test]
fn provider_youtube_metadata() {
    let p = OAuthProvider::youtube();
    assert_eq!(p.name, "youtube");
    assert!(p.auth_url.starts_with("https://accounts.google.com/"));
    assert_eq!(p.token_url, "https://oauth2.googleapis.com/token");
    assert!(p.scopes.iter().any(|s| s.contains("youtube.force-ssl")));
}

#[test]
fn provider_kick_metadata() {
    let p = OAuthProvider::kick();
    assert_eq!(p.name, "kick");
    assert_eq!(p.auth_url, "https://id.kick.com/oauth/authorize");
    assert_eq!(p.token_url, "https://id.kick.com/oauth/token");
    assert!(p.scopes.contains(&"user:read"));
    assert!(p.scopes.contains(&"chat:write"));
}

#[test]
fn provider_facebook_metadata() {
    let p = OAuthProvider::facebook();
    assert_eq!(p.name, "facebook");
    assert!(p.auth_url.contains("facebook.com"));
    assert!(p.token_url.contains("graph.facebook.com"));
    assert!(p.scopes.contains(&"pages_show_list"));
    assert!(p.scopes.contains(&"publish_video"));
}

// ===== config getters: explicit override + placeholder fall-through =====

#[test]
fn config_client_ids_use_explicit_override() {
    let cfg = OAuthConfig {
        twitch_client_id: Some("  tw-id  ".into()),
        youtube_client_id: Some("yt-id".into()),
        kick_client_id: Some("kk-id".into()),
        facebook_client_id: Some("fb-id".into()),
        ..Default::default()
    };
    // Override wins and is trimmed.
    assert_eq!(cfg.get_twitch_client_id(), "tw-id");
    assert_eq!(cfg.get_youtube_client_id(), "yt-id");
    assert_eq!(cfg.get_kick_client_id(), "kk-id");
    assert_eq!(cfg.get_facebook_client_id(), "fb-id");
}

#[test]
fn config_client_secrets_use_explicit_override() {
    let cfg = OAuthConfig {
        twitch_client_secret: Some("tw-secret".into()),
        youtube_client_secret: Some("yt-secret".into()),
        kick_client_secret: Some("kk-secret".into()),
        facebook_client_secret: Some("fb-secret".into()),
        ..Default::default()
    };
    assert_eq!(cfg.get_twitch_client_secret().as_deref(), Some("tw-secret"));
    assert_eq!(
        cfg.get_youtube_client_secret().as_deref(),
        Some("yt-secret")
    );
    assert_eq!(cfg.get_kick_client_secret().as_deref(), Some("kk-secret"));
    assert_eq!(
        cfg.get_facebook_client_secret().as_deref(),
        Some("fb-secret")
    );
}

#[test]
fn config_blank_override_falls_through_to_placeholder() {
    // An all-whitespace override is treated as unset and falls through.
    // With no env configured the embedded placeholder constant is returned.
    let cfg = OAuthConfig {
        twitch_client_id: Some("   ".into()),
        ..Default::default()
    };
    let id = cfg.get_twitch_client_id();
    assert!(!id.is_empty());
    // Default config (no override, no env) yields the same placeholder.
    let default_id = OAuthConfig::default().get_twitch_client_id();
    assert_eq!(id, default_id);
}

#[test]
fn config_secrets_default_to_none() {
    // No override, no env → secrets are absent (not an empty string).
    let cfg = OAuthConfig::default();
    // These only hold when the corresponding env vars are unset, which is
    // the case under a normal test runner.
    if std::env::var("SPIRITSTREAM_TWITCH_CLIENT_SECRET").is_err() {
        assert!(cfg.get_twitch_client_secret().is_none());
    }
    if std::env::var("SPIRITSTREAM_FACEBOOK_CLIENT_SECRET").is_err() {
        assert!(cfg.get_facebook_client_secret().is_none());
    }
}

#[test]
fn config_has_flags_require_real_credentials() {
    // Placeholders → unconfigured (the dead-link fix); real values → configured.
    let cfg = configured_cfg();
    assert!(cfg.has_twitch());
    assert!(cfg.has_youtube());
    assert!(cfg.has_kick());
    assert!(cfg.has_facebook());
    assert!(cfg.has_trovo());
}

// ===== PKCE pair shape =====

#[test]
fn pkce_pair_is_distinct_and_url_safe() {
    let (verifier, challenge) = super::pkce::generate_pkce_pair();
    assert!(!verifier.is_empty());
    assert!(!challenge.is_empty());
    assert_ne!(verifier, challenge);
    let url_safe = |s: &str| {
        s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    };
    assert!(url_safe(&verifier), "verifier must be base64url-no-pad");
    assert!(url_safe(&challenge), "challenge must be base64url-no-pad");
    // Two successive pairs differ (randomised verifier).
    let (verifier2, _) = super::pkce::generate_pkce_pair();
    assert_ne!(verifier, verifier2);
}

// ===== service config accessors (mod.rs) =====

#[tokio::test]
async fn is_configured_reflects_real_credentials_only() {
    let s = configured_svc();
    assert!(s.is_configured("twitch").await);
    assert!(s.is_configured("youtube").await);
    assert!(s.is_configured("kick").await);
    assert!(s.is_configured("facebook").await);
    assert!(s.is_configured("trovo").await);
    assert!(!s.is_configured("myspace").await);
}

#[tokio::test]
async fn start_flow_refuses_unconfigured_provider_before_binding_ports() {
    // Default config = placeholders. The typed refusal is the dead-link
    // fix: no callback port binds, no browser opens onto a 400 page.
    if std::env::var("SPIRITSTREAM_TWITCH_CLIENT_ID").is_ok() {
        return; // dev shell injected real creds; guard not observable
    }
    let s = svc();
    match s.start_flow("twitch").await {
        Err(crate::errors::CoreError::OAuthProviderNotConfigured { provider }) => {
            assert_eq!(provider, "twitch");
        }
        other => panic!("expected OAuthProviderNotConfigured, got {other:?}"),
    }
}

#[tokio::test]
async fn update_config_round_trips_through_get_config() {
    let s = svc();
    let new_cfg = OAuthConfig {
        twitch_client_id: Some("rotated".into()),
        ..Default::default()
    };
    s.update_config(new_cfg).await;
    let read_back = s.get_config().await;
    assert_eq!(read_back.twitch_client_id.as_deref(), Some("rotated"));
}

// ===== start_flow: per-provider URL shape (network-free; binds localhost) =====

#[tokio::test]
async fn start_flow_youtube_uses_pkce_code_flow() {
    let s = configured_svc();
    let result = s.start_flow("youtube").await.expect("youtube flow starts");
    assert!(result.auth_url.starts_with("https://accounts.google.com/"));
    assert!(result.auth_url.contains("response_type=code"));
    assert!(result.auth_url.contains("code_challenge="));
    assert!(result.auth_url.contains("code_challenge_method=S256"));
    assert!(result.auth_url.contains("access_type=offline"));
    assert!(result.auth_url.contains("prompt=consent"));
    assert!(!result.state.is_empty());
    assert!(result.callback_port >= 1024);
}

#[tokio::test]
async fn start_flow_kick_uses_pkce_code_flow() {
    let s = configured_svc();
    let result = s.start_flow("kick").await.expect("kick flow starts");
    assert!(result.auth_url.starts_with("https://id.kick.com/"));
    assert!(result.auth_url.contains("response_type=code"));
    assert!(result.auth_url.contains("code_challenge="));
}

#[tokio::test]
async fn start_flow_facebook_uses_auth_code_without_pkce() {
    let s = configured_svc();
    let result = s
        .start_flow("facebook")
        .await
        .expect("facebook flow starts");
    assert!(result.auth_url.contains("facebook.com"));
    assert!(result.auth_url.contains("response_type=code"));
    // Facebook web OAuth uses App Secret, not PKCE.
    assert!(!result.auth_url.contains("code_challenge="));
}

#[tokio::test]
async fn start_flow_twitch_with_secret_uses_auth_code() {
    let cfg = OAuthConfig {
        twitch_client_id: Some("test-twitch-id".into()),
        twitch_client_secret: Some("a-secret".into()),
        ..Default::default()
    };
    let s = OAuthService::new(cfg);
    let result = s.start_flow("twitch").await.expect("twitch flow starts");
    assert!(result.auth_url.contains("response_type=code"));
    assert!(result.auth_url.contains("force_verify=true"));
}

#[tokio::test]
async fn start_flow_unknown_provider_is_not_implemented() {
    let s = svc();
    match s.start_flow("myspace").await {
        Err(crate::errors::CoreError::NotImplemented { .. }) => {}
        other => panic!("expected NotImplemented, got {other:?}"),
    }
}

// ===== exchange_code / complete_flow: no pending flow → Unauthorized =====

#[tokio::test]
async fn exchange_code_without_pending_flow_is_unauthorized() {
    let s = svc();
    match s.exchange_code("twitch", "code", "no-such-state").await {
        Err(crate::errors::CoreError::Unauthorized) => {}
        other => panic!("expected Unauthorized, got {other:?}"),
    }
}

#[tokio::test]
async fn complete_flow_without_pending_flow_is_unauthorized() {
    let s = svc();
    match s.complete_flow("youtube", "code", "no-such-state").await {
        Err(crate::errors::CoreError::Unauthorized) => {}
        other => panic!("expected Unauthorized, got {other:?}"),
    }
}

// ===== refresh_token / revoke_token: unknown provider rejected before network =====

#[tokio::test]
async fn refresh_token_unknown_provider_is_not_implemented() {
    let s = svc();
    match s.refresh_token("myspace", "rt").await {
        Err(crate::errors::CoreError::NotImplemented { .. }) => {}
        other => panic!("expected NotImplemented, got {other:?}"),
    }
}

#[tokio::test]
async fn revoke_token_unknown_provider_is_not_implemented() {
    let s = svc();
    match s.revoke_token("myspace", "tok").await {
        Err(crate::errors::CoreError::NotImplemented { .. }) => {}
        other => panic!("expected NotImplemented, got {other:?}"),
    }
}

// ===== refresh_profile_tokens: empty profile short-circuits without network =====

#[tokio::test]
async fn refresh_profile_tokens_empty_profile_is_noop() {
    let s = svc();
    let mut profile = empty_profile();
    let outcome = s
        .refresh_profile_tokens(&mut profile, 300, None)
        .await
        .expect("empty profile refresh succeeds");
    assert!(outcome.refreshed.is_empty());
    assert!(outcome.failed.is_empty());
}

/// A profile with no OAuth tokens set — every provider short-circuits via
/// the `access.is_empty() || refresh.is_empty()` guard before any network.
fn empty_profile() -> crate::models::Profile {
    crate::models::Profile {
        id: "oauth-test".into(),
        name: "oauth-test".into(),
        encrypted: false,
        input: crate::models::RtmpInput::default(),
        output_groups: Vec::new(),
        settings: crate::models::ProfileSettings::default(),
        pii_blocklist: Vec::new(),
        pii_fuzzy: false,
        anonymous_logging: false,
        anonymous_salt: String::new(),
    }
}

// ===== public-client refresh (wiremock; Twitch ships without a secret) =====

#[tokio::test]
async fn twitch_public_client_refresh_omits_secret_and_returns_rotated_tokens() {
    use wiremock::matchers::{body_string_contains, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/oauth2/token"))
        .and(body_string_contains("grant_type=refresh_token"))
        .and(body_string_contains("client_id=test-twitch-id"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "access_token": "rotated-access",
            "refresh_token": "rotated-refresh",
            "expires_in": 14400,
            "token_type": "bearer"
        })))
        .mount(&server)
        .await;

    let svc = OAuthService::new(OAuthConfig {
        twitch_client_id: Some("test-twitch-id".into()),
        // No secret: the shipped Twitch app is a PUBLIC client; its
        // refresh must not send client_secret (and must still work —
        // public-client refresh tokens rotate on every use and expire
        // after 30 idle days, so this path keeps users signed in).
        ..OAuthConfig::default()
    });
    svc.override_provider(OAuthProvider {
        name: "twitch".into(),
        auth_url: format!("{}/oauth2/authorize", server.uri()),
        token_url: format!("{}/oauth2/token", server.uri()),
        device_url: Some(format!("{}/oauth2/device", server.uri())),
        refresh_url: None,
        user_info_url: format!("{}/helix/users", server.uri()),
        scopes: vec!["chat:read"],
    });

    let tokens = svc
        .refresh_token("twitch", "old-refresh")
        .await
        .expect("public-client refresh succeeds");
    assert_eq!(tokens.access_token, "rotated-access");
    assert_eq!(tokens.refresh_token.as_deref(), Some("rotated-refresh"));

    // The mock matched on body contents; assert the secret was absent.
    let requests = server.received_requests().await.unwrap();
    let body = String::from_utf8_lossy(&requests[0].body).to_string();
    assert!(
        !body.contains("client_secret"),
        "public client must not send a secret: {body}"
    );
}
