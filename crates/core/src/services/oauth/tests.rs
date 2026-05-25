use super::{OAuthConfig, OAuthService};

fn svc() -> OAuthService {
    OAuthService::new(OAuthConfig::default())
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
