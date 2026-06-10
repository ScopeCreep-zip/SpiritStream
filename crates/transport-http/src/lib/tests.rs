//! Pure-function unit tests for the lib.rs transport surface — cookie
//! mode detection, origin allow-list matching, `mask_sensitive`
//! coverage + proptest, and the cloud-mode startup guard.
//!
//! Extracted from `lib.rs` (K4) so the orchestrator stays under the
//! 600 LOC ceiling.

use super::*;

// ----- SessionCookieMode detection ----------------------------
// `detect` is a pure function; tests pass the env override directly so
// they remain parallel-safe (no global process-state mutation).

#[test]
fn cookie_mode_loopback_defaults_to_localhost_dev() {
    assert_eq!(
        SessionCookieMode::detect("127.0.0.1", None, None),
        SessionCookieMode::LocalhostDev,
    );
    assert_eq!(
        SessionCookieMode::detect("localhost", None, None),
        SessionCookieMode::LocalhostDev,
    );
    assert_eq!(
        SessionCookieMode::detect("::1", None, None),
        SessionCookieMode::LocalhostDev,
    );
}

#[test]
fn cookie_mode_cloud_deploy_is_cross_origin() {
    // Cloud deploys must use SameSite=Lax (Strict drops the cookie
    // on cross-site navigation back to the UI).
    assert_eq!(
        SessionCookieMode::detect("0.0.0.0", Some("cloud"), None),
        SessionCookieMode::CrossOrigin,
    );
    assert_eq!(
        SessionCookieMode::detect("203.0.113.5", Some("CLOUD"), None),
        SessionCookieMode::CrossOrigin,
    );
}

#[test]
fn cookie_mode_non_loopback_defaults_to_same_origin() {
    assert_eq!(
        SessionCookieMode::detect("192.168.1.50", Some("desktop"), None),
        SessionCookieMode::SameOrigin,
    );
    assert_eq!(
        SessionCookieMode::detect("server.local", None, None),
        SessionCookieMode::SameOrigin,
    );
}

#[test]
fn cookie_mode_explicit_override_wins() {
    // Explicit override always wins, even when bind address + deploy
    // mode would auto-detect to something else.
    assert_eq!(
        SessionCookieMode::detect("127.0.0.1", None, Some("cross_origin")),
        SessionCookieMode::CrossOrigin,
    );
    assert_eq!(
        SessionCookieMode::detect("127.0.0.1", Some("cloud"), Some("same-origin")),
        SessionCookieMode::SameOrigin,
    );
    assert_eq!(
        SessionCookieMode::detect("public.example.com", Some("cloud"), Some("localhost-dev")),
        SessionCookieMode::LocalhostDev,
    );
    // Unrecognised override falls through to auto-detect.
    assert_eq!(
        SessionCookieMode::detect("127.0.0.1", None, Some("nonsense")),
        SessionCookieMode::LocalhostDev,
    );
}

#[test]
fn cookie_mode_attributes_match_threat_model() {
    // SameOrigin: Strict + Secure
    assert!(SessionCookieMode::SameOrigin.secure());
    assert_eq!(
        SessionCookieMode::SameOrigin.same_site(),
        tower_cookies::cookie::SameSite::Strict,
    );
    // CrossOrigin: Lax + Secure
    assert!(SessionCookieMode::CrossOrigin.secure());
    assert_eq!(
        SessionCookieMode::CrossOrigin.same_site(),
        tower_cookies::cookie::SameSite::Lax,
    );
    // LocalhostDev: no Secure (would be rejected on plain HTTP), but
    // still Strict so even local malicious sites can't post.
    assert!(!SessionCookieMode::LocalhostDev.secure());
    assert_eq!(
        SessionCookieMode::LocalhostDev.same_site(),
        tower_cookies::cookie::SameSite::Strict,
    );
}

// ----- Origin allow-list pattern matching --------------------

#[test]
fn origin_matcher_wildcard_port() {
    let allowed = vec!["http://localhost:*".to_string()];
    assert!(origin_matches("http://localhost:5173", &allowed));
    assert!(origin_matches("http://localhost:8008", &allowed));
    // Bare "http://localhost" (no port) does NOT match `:*` — the
    // wildcard requires a port to be present.
    assert!(!origin_matches("http://localhost", &allowed));
    // Different host does not match.
    assert!(!origin_matches("http://evil.com:5173", &allowed));
}

#[test]
fn origin_matcher_exact_match() {
    let allowed = vec![
        "tauri://localhost".to_string(),
        "https://tauri.localhost".to_string(),
    ];
    assert!(origin_matches("tauri://localhost", &allowed));
    assert!(origin_matches("https://tauri.localhost", &allowed));
    assert!(!origin_matches("http://tauri.localhost", &allowed));
    assert!(!origin_matches("tauri://evil.com", &allowed));
}

#[test]
fn origin_matcher_rejects_substring_attacks() {
    // An attacker registering `localhost.evil.com` must not match
    // `http://localhost:*` via prefix string matching.
    let allowed = vec!["http://localhost:*".to_string()];
    assert!(!origin_matches("http://localhost.evil.com:5173", &allowed));
}

// ----- mask_sensitive coverage + proptest -----------------

#[test]
fn mask_sensitive_redacts_rtmp_stream_key() {
    let log = "Starting stream to rtmp://live.twitch.tv/app/live_12345_abcdefghijklmnop";
    let masked = mask_sensitive(log);
    assert!(
        !masked.contains("live_12345_abcdefghijklmnop"),
        "stream key leaked: {masked}"
    );
    assert!(masked.contains("[REDACTED]"));
}

#[test]
fn mask_sensitive_redacts_template_expansion() {
    let log = "ffmpeg -f flv rtmp://server.example.com/app/${STREAM_KEY}";
    let masked = mask_sensitive(log);
    assert!(
        !masked.contains("${STREAM_KEY}"),
        "template var leaked: {masked}"
    );
    assert!(masked.contains("[REDACTED]"));
}

#[test]
fn mask_sensitive_redacts_bearer_token() {
    let log = "Authorization: Bearer abcdefghijklmnopqrstuvwxyz1234567890";
    let masked = mask_sensitive(log);
    assert!(
        !masked.contains("abcdefghijklmnopqrstuvwxyz1234567890"),
        "bearer token leaked: {masked}",
    );
}

#[test]
fn mask_sensitive_redacts_enc_v1_and_v2_blobs() {
    let v1 = "ENC::dGVzdHRlc3R0ZXN0dGVzdA==";
    let v2 = "ENC2::aGVsbG93b3JsZGhlbGxvd29ybGQ=";
    let masked_v1 = mask_sensitive(&format!("setting={v1}"));
    let masked_v2 = mask_sensitive(&format!("setting={v2}"));
    assert!(
        masked_v1.contains("[ENCRYPTED]"),
        "v1 blob not masked: {masked_v1}"
    );
    assert!(
        masked_v2.contains("[ENCRYPTED]"),
        "v2 blob not masked: {masked_v2}"
    );
}

proptest::proptest! {
    /// Property: any token-shaped string (≥20 chars of
    /// `[A-Za-z0-9_\-./+]`) that follows a recognised keyword like
    /// `token=`, `bearer `, `password:`, etc. MUST be redacted from
    /// the output of `mask_sensitive`. Fuzz coverage of this
    /// property is required to catch new token shapes.
    #[test]
    fn prop_token_after_keyword_is_always_redacted(
        keyword in "token|key|password|secret|bearer|oauth|access_token|refresh_token|authorization",
        separator in "[:=]| ",
        token in "[A-Za-z0-9_\\-./+]{20,80}",
        prefix in "[a-z ]{0,30}",
        suffix in "[a-z ]{0,30}",
    ) {
        let input = format!("{prefix}{keyword}{separator}{token}{suffix}");
        let masked = mask_sensitive(&input);
        proptest::prop_assert!(
            !masked.contains(&token),
            "token leaked after keyword {keyword:?}: input={input:?}, masked={masked:?}",
        );
    }

    /// Property: any RTMP URL with a trailing path segment of 1+ char
    /// must have that segment masked. The segment is the stream key
    /// in both Twitch and YouTube URLs.
    #[test]
    fn prop_rtmp_path_segment_is_always_redacted(
        scheme in "rtmps?",
        host in "[a-z]{3,12}\\.[a-z]{3,5}",
        app in "[a-z]{3,12}",
        key in "[a-zA-Z0-9_]{8,40}",
    ) {
        let input = format!("{scheme}://{host}/{app}/{key}");
        let masked = mask_sensitive(&input);
        proptest::prop_assert!(
            !masked.contains(&key) || masked.contains("[REDACTED]"),
            "rtmp stream key leaked: input={input:?}, masked={masked:?}",
        );
    }
}

// ----- cloud-mode startup guard ----------------------------
// `enforce_cloud_mode_preconditions` is pure (takes the
// tls-declared flag as a parameter, not from env) so these tests
// are parallel-safe without `serial_test`.

#[test]
fn cloud_mode_refuses_to_start_without_strong_token() {
    let weak = Some("short".to_string());
    let err = enforce_cloud_mode_preconditions(&weak, true).unwrap_err();
    let msg = format!("{err}");
    assert!(
        msg.contains("SPIRITSTREAM_API_TOKEN"),
        "expected token error: {msg}"
    );
    assert!(msg.contains("32"), "expected min-length hint: {msg}");
}

#[test]
fn cloud_mode_refuses_to_start_without_token_at_all() {
    let none: Option<String> = None;
    let err = enforce_cloud_mode_preconditions(&none, true).unwrap_err();
    assert!(format!("{err}").contains("SPIRITSTREAM_API_TOKEN"));
}

#[test]
fn cloud_mode_refuses_to_start_without_tls_proxy_declared() {
    let strong = Some("a".repeat(32));
    let err = enforce_cloud_mode_preconditions(&strong, false).unwrap_err();
    let msg = format!("{err}");
    assert!(
        msg.contains("SPIRITSTREAM_BEHIND_TLS_PROXY"),
        "expected TLS-proxy error: {msg}",
    );
}

#[test]
fn cloud_mode_starts_when_both_preconditions_satisfied() {
    let strong = Some("0123456789abcdef0123456789abcdef".to_string());
    let result = enforce_cloud_mode_preconditions(&strong, true);
    assert!(result.is_ok(), "expected Ok, got: {result:?}");
}

#[test]
fn cloud_mode_accepts_exactly_32_char_token() {
    // Boundary check — the policy says "≥ 32 chars".
    let exactly_32 = Some("a".repeat(32));
    assert!(enforce_cloud_mode_preconditions(&exactly_32, true).is_ok());
}

#[test]
fn cloud_mode_rejects_31_char_token() {
    let just_under = Some("a".repeat(31));
    assert!(enforce_cloud_mode_preconditions(&just_under, true).is_err());
}

// ----- CSP loading-page style hash (M3) -----------------------
// The response CSP forbids `'unsafe-inline'`; the loading page's one inline
// <style> is allow-listed by the SHA-256 of its exact content. These guard the
// extraction against drifting and grabbing the wrong slice of the document.

#[test]
fn loading_page_style_csp_hash_is_wellformed() {
    let src = loading_page_style_csp_hash();
    assert!(
        src.starts_with("'sha256-") && src.ends_with('\''),
        "expected a quoted sha256 CSP source, got {src}"
    );
}

#[test]
fn loading_page_style_csp_hash_covers_only_the_style_block() {
    let start = LOADING_PAGE_HTML
        .find("<style>")
        .expect("inline <style> present")
        + "<style>".len();
    let end = LOADING_PAGE_HTML
        .find("</style>")
        .expect("inline <style> closed");
    let style = &LOADING_PAGE_HTML[start..end];
    // The hashed slice is the spinner CSS, not the surrounding document.
    assert!(
        style.contains(".spinner"),
        "extracted slice missing .spinner rule"
    );
    assert!(
        style.contains("@keyframes spin"),
        "extracted slice missing keyframes"
    );
    assert!(
        !style.contains("<body>"),
        "extraction overran the </style> close"
    );
}
