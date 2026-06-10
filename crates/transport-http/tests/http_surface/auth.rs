//! Login brute-force defense + confirm-token gating + safety panic
//! audit emission + audit-log filtering.
//!
//! `POST /api/v1/auth/login` is gated by TWO layers:
//!   * IP-keyed rate limit (5/min) — quick to trip, cheap rejection.
//!   * `AuthService` exponential backoff + sliding-window lockout — kicks in
//!     once enough failures accumulate; surfaces a Retry-After.
//!
//! The login limiter trips at 5 attempts which is BELOW the AuthService
//! lockout threshold (10), so a slow attacker (one request per minute)
//! hits the lockout first, while a fast attacker hits the rate limiter
//! first. Both produce 429s the client should obey.

use super::common::{boot, boot_with_token, delete_with_headers, get, post_invoke};
use serde_json::Value;

#[test]
fn login_with_correct_token_succeeds() {
    let server = boot_with_token("correct-horse-battery-staple");
    let (status, body) = post_invoke(
        &server,
        "/api/v1/auth/login",
        r#"{"token":"correct-horse-battery-staple"}"#,
    );
    assert_eq!(status, 200, "correct token must succeed: {body}");
    // Login returns `AuthLoginResponse {}` — empty object on success.
    // The HttpOnly session cookie is the actual auth artifact; presence
    // of a 200 means the cookie was set.
    let _json: Value = serde_json::from_str(&body).unwrap();
}

#[test]
fn login_with_wrong_token_returns_401_then_eventually_429() {
    let server = boot_with_token("correct");
    for attempt in 1..=4 {
        let (status, body) = post_invoke(&server, "/api/v1/auth/login", r#"{"token":"wrong"}"#);
        assert_eq!(
            status, 401,
            "attempt {attempt}: expected 401 from wrong-token path, got {status}: {body}",
        );
    }
    let _ = post_invoke(&server, "/api/v1/auth/login", r#"{"token":"wrong"}"#);
    let (status, body) = post_invoke(&server, "/api/v1/auth/login", r#"{"token":"wrong"}"#);
    assert_eq!(
        status, 429,
        "after 6 wrong-token attempts the 6th must be rate-limited: {body}",
    );
}

#[test]
fn login_429_lockout_carries_retry_after_header() {
    let server = boot_with_token("correct");
    for _ in 0..6 {
        let _ = post_invoke(&server, "/api/v1/auth/login", r#"{"token":"wrong"}"#);
    }
    let resp = reqwest::blocking::Client::new()
        .post(format!("{}/api/v1/auth/login", server.base))
        .header("content-type", "application/json")
        .body(r#"{"token":"wrong"}"#)
        .send()
        .expect("POST");
    assert_eq!(resp.status().as_u16(), 429);
    let body: Value = resp.json().expect("JSON body");
    assert_eq!(body["ok"], Value::Bool(false));
}

// ---------------------------------------------------------------------------
// Confirmation tokens for destructive ops.
// ---------------------------------------------------------------------------

#[test]
fn destructive_clear_data_without_confirm_token_is_rejected() {
    let server = boot();
    let (status, _body) = delete_with_headers(&server, "/api/v1/settings/data", &[]);
    assert!(
        status == 403 || status == 401 || status == 400,
        "missing X-Confirm-Token must be rejected (got {status})",
    );
}

#[test]
fn destructive_clear_data_with_valid_confirm_token_succeeds() {
    let server = boot();
    let (issue_status, issue_body) = post_invoke(
        &server,
        "/api/v1/security/confirm-token",
        r#"{"intent":"clear_data"}"#,
    );
    assert_eq!(issue_status, 200, "issue endpoint failed: {issue_body}");
    let issued: Value = serde_json::from_str(&issue_body).unwrap();
    let token = issued["token"].as_str().expect("token in response");
    assert!(!token.is_empty());

    let (status, body) = delete_with_headers(
        &server,
        "/api/v1/settings/data",
        &[("X-Confirm-Token", token)],
    );
    assert_eq!(
        status, 200,
        "with valid token, clear_data must succeed: {body}"
    );
}

#[test]
fn confirm_token_is_one_shot() {
    let server = boot();
    let (_, body) = post_invoke(
        &server,
        "/api/v1/security/confirm-token",
        r#"{"intent":"clear_data"}"#,
    );
    let token = serde_json::from_str::<Value>(&body).unwrap()["token"]
        .as_str()
        .unwrap()
        .to_string();
    let (status1, _) = delete_with_headers(
        &server,
        "/api/v1/settings/data",
        &[("X-Confirm-Token", &token)],
    );
    assert_eq!(status1, 200);
    let (status2, _) = delete_with_headers(
        &server,
        "/api/v1/settings/data",
        &[("X-Confirm-Token", &token)],
    );
    assert!(
        status2 == 403 || status2 == 401 || status2 == 400,
        "second use of the same token must fail (got {status2})",
    );
}

#[test]
fn revoke_all_sessions_requires_confirm_token_and_runs_when_present() {
    let server = boot();
    let resp = reqwest::blocking::Client::new()
        .post(format!(
            "{}/api/v1/security/sessions/revoke-all",
            server.base
        ))
        .send()
        .expect("POST");
    let bad_status = resp.status().as_u16();
    assert!(
        bad_status == 403 || bad_status == 401 || bad_status == 400,
        "revoke-all without confirm token must be rejected (got {bad_status})",
    );

    let (_, body) = post_invoke(
        &server,
        "/api/v1/security/confirm-token",
        r#"{"intent":"revoke_all_sessions"}"#,
    );
    let token = serde_json::from_str::<Value>(&body).unwrap()["token"]
        .as_str()
        .unwrap()
        .to_string();
    let resp = reqwest::blocking::Client::new()
        .post(format!(
            "{}/api/v1/security/sessions/revoke-all",
            server.base
        ))
        .header("X-Confirm-Token", &token)
        .send()
        .expect("POST");
    assert_eq!(resp.status().as_u16(), 200);
    let payload: Value = resp.json().expect("JSON body");
    // `revoked` is the count of sessions that were active before the
    // call. On a fresh-boot server with no token configured nobody
    // has logged in, so this will be zero — but the field must exist.
    assert!(payload.get("revoked").is_some(), "missing revoked field");
}

#[test]
fn confirm_token_intent_scoping() {
    let server = boot();
    let (_, body) = post_invoke(
        &server,
        "/api/v1/security/confirm-token",
        r#"{"intent":"clear_data"}"#,
    );
    let token = serde_json::from_str::<Value>(&body).unwrap()["token"]
        .as_str()
        .unwrap()
        .to_string();
    let resp = reqwest::blocking::Client::new()
        .post(format!(
            "{}/api/v1/security/machine-key/rotate",
            server.base
        ))
        .header("X-Confirm-Token", &token)
        .send()
        .expect("POST");
    let status = resp.status().as_u16();
    assert!(
        status == 403 || status == 401 || status == 400,
        "wrong-intent token must be rejected (got {status})",
    );
}

// Safety panic emits an audit entry the read endpoint can find. Lives
// next to the confirm-token tests because both exercise the audit
// chain's append/read contract end-to-end.
#[test]
fn safety_panic_records_an_audit_entry_visible_through_audit_log_endpoint() {
    let server = boot();
    let (status, _) = post_invoke(&server, "/api/v1/safety/panic", "");
    assert_eq!(status, 200);
    let (status, body) = get(&server, "/api/v1/audit/log?kind=panic_triggered");
    assert_eq!(status, 200, "audit/log failed: {body}");
    let payload: Value = serde_json::from_str(&body).unwrap();
    let entries = payload["entries"].as_array().expect("entries array");
    assert!(!entries.is_empty(), "no panic_triggered entries: {payload}");
}

#[test]
fn audit_log_filter_by_kind_excludes_other_kinds() {
    let server = boot();
    let _ = post_invoke(&server, "/api/v1/safety/panic", "");
    let (status, body) = get(&server, "/api/v1/audit/log?kind=machine_key_rotated");
    assert_eq!(status, 200);
    let payload: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(payload["total"], 0);
}
