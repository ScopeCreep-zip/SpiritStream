//! CSRF middleware end-to-end behaviour.
//!
//! State-changing requests (POST/PUT/PATCH/DELETE) are gated on the
//! Sec-Fetch-Site header (primary) and the Origin allow-list (fallback).
//! The middleware is mounted across the whole router, so any handler that
//! accepts a mutating method exercises it. We pick `/api/v1/auth/login`
//! because it's available without prior state and returns a stable shape.

use super::common::{boot, post_with_headers};
use serde_json::Value;

#[test]
fn csrf_same_origin_request_passes() {
    let server = boot();
    let (status, _) = post_with_headers(
        &server,
        "/api/v1/auth/login",
        r#"{"token":""}"#,
        &[("Sec-Fetch-Site", "same-origin")],
    );
    assert_eq!(status, 200, "Sec-Fetch-Site: same-origin must be allowed");
}

#[test]
fn csrf_same_site_request_passes() {
    let server = boot();
    let (status, _) = post_with_headers(
        &server,
        "/api/v1/auth/login",
        r#"{"token":""}"#,
        &[("Sec-Fetch-Site", "same-site")],
    );
    assert_eq!(status, 200, "Sec-Fetch-Site: same-site must be allowed");
}

#[test]
fn csrf_none_user_initiated_request_passes() {
    let server = boot();
    // Sec-Fetch-Site: none means the request was initiated by the user
    // (typed URL, bookmark, redirect from the address bar) — these are
    // not CSRF vectors.
    let (status, _) = post_with_headers(
        &server,
        "/api/v1/auth/login",
        r#"{"token":""}"#,
        &[("Sec-Fetch-Site", "none")],
    );
    assert_eq!(status, 200);
}

#[test]
fn csrf_cross_site_without_trusted_origin_is_rejected() {
    let server = boot();
    let (status, body) = post_with_headers(
        &server,
        "/api/v1/auth/login",
        r#"{"token":""}"#,
        &[
            ("Sec-Fetch-Site", "cross-site"),
            ("Origin", "http://evil.example.com"),
        ],
    );
    assert_eq!(
        status, 403,
        "Sec-Fetch-Site: cross-site from untrusted origin must be rejected: {body}"
    );
    let json: Value = serde_json::from_str(&body).expect("CSRF rejection emits JSON");
    assert_eq!(json["ok"], Value::Bool(false));
}

#[test]
fn csrf_cross_site_with_tauri_origin_passes() {
    // Tauri webview requests are technically cross-site (tauri:// → http://)
    // so the cross-site → Origin-allow-list fallback must accept the
    // built-in Tauri origins.
    let server = boot();
    let (status, _) = post_with_headers(
        &server,
        "/api/v1/auth/login",
        r#"{"token":""}"#,
        &[
            ("Sec-Fetch-Site", "cross-site"),
            ("Origin", "tauri://localhost"),
        ],
    );
    assert_eq!(status, 200, "Tauri webview origin must pass via allow-list");
}

#[test]
fn csrf_missing_headers_falls_back_to_cli_pass() {
    // CLI tools (and tests like this harness) send neither Sec-Fetch-Site
    // nor Origin. The middleware lets these through and defers to
    // auth_middleware.
    let server = boot();
    let (status, _) = post_with_headers(&server, "/api/v1/auth/login", r#"{"token":""}"#, &[]);
    assert_eq!(
        status, 200,
        "no CSRF headers + no Origin must pass (CLI fallback)"
    );
}

#[test]
fn csrf_missing_sec_fetch_with_evil_origin_is_rejected() {
    // Older browser path: Sec-Fetch-Site absent → fall back to Origin
    // allow-list. An attacker forging an Origin header pointing at their
    // own domain must still get rejected.
    let server = boot();
    let (status, _) = post_with_headers(
        &server,
        "/api/v1/auth/login",
        r#"{"token":""}"#,
        &[("Origin", "http://evil.example.com")],
    );
    assert_eq!(
        status, 403,
        "untrusted Origin must be rejected when Sec-Fetch-Site is absent"
    );
}
