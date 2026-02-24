use axum::{
    extract::{Json, State},
    http::{header, HeaderMap},
    response::IntoResponse,
};
use serde::Deserialize;
use serde_json::json;
use subtle::ConstantTimeEq;
use tower_cookies::{Cookie, Cookies};

use crate::constants::{AUTH_COOKIE_NAME, COOKIE_MAX_AGE_SECS};
use crate::state::AppState;

/// Constant-time token comparison to prevent timing attacks
pub(crate) fn verify_token(expected: &str, provided: &str) -> bool {
    expected.as_bytes().ct_eq(provided.as_bytes()).into()
}

/// Extract bearer token from Authorization header
pub(crate) fn bearer_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

#[derive(Deserialize)]
pub(crate) struct LoginRequest {
    token: String,
}

/// Determine if cookies should use the Secure flag.
/// True when: server is bound to non-loopback address, OR request came via HTTPS proxy.
pub(crate) fn should_use_secure_cookie(state: &AppState, headers: &HeaderMap) -> bool {
    if !state.is_localhost {
        return true;
    }
    // Check for reverse proxy forwarding HTTPS
    if let Some(proto) = headers.get("x-forwarded-proto") {
        if let Ok(proto_str) = proto.to_str() {
            return proto_str.eq_ignore_ascii_case("https");
        }
    }
    false
}

/// Set a session cookie with security flags determined by deployment context
pub(crate) fn set_session_cookie(cookies: &Cookies, use_secure: bool) {
    let session_id = uuid::Uuid::new_v4().to_string();
    let cookie = Cookie::build((AUTH_COOKIE_NAME, session_id))
        .http_only(true)
        .secure(use_secure)
        .same_site(if use_secure {
            tower_cookies::cookie::SameSite::None
        } else {
            tower_cookies::cookie::SameSite::Strict
        })
        .path("/")
        .max_age(tower_cookies::cookie::time::Duration::seconds(COOKIE_MAX_AGE_SECS))
        .build();
    cookies.add(cookie);
}

/// POST /auth/login - Validate token and set HttpOnly cookie
pub(crate) async fn auth_login(
    State(state): State<AppState>,
    cookies: Cookies,
    headers: HeaderMap,
    Json(payload): Json<LoginRequest>,
) -> impl IntoResponse {
    let use_secure = should_use_secure_cookie(&state, &headers);

    match state.auth_token.as_deref() {
        None => {
            // No token configured - open access, set session cookie anyway
            set_session_cookie(&cookies, use_secure);
            Json(json!({ "ok": true }))
        }
        Some(expected) if verify_token(expected, &payload.token) => {
            set_session_cookie(&cookies, use_secure);
            Json(json!({ "ok": true }))
        }
        _ => {
            // Invalid token - add a small delay to prevent brute force
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            Json(json!({ "ok": false, "error": "Invalid token" }))
        }
    }
}

/// POST /auth/logout - Clear session cookie
pub(crate) async fn auth_logout(
    State(state): State<AppState>,
    cookies: Cookies,
    headers: HeaderMap,
) -> impl IntoResponse {
    let use_secure = should_use_secure_cookie(&state, &headers);
    let cookie = Cookie::build((AUTH_COOKIE_NAME, ""))
        .path("/")
        .http_only(true)
        .secure(use_secure)
        .same_site(if use_secure {
            tower_cookies::cookie::SameSite::None
        } else {
            tower_cookies::cookie::SameSite::Strict
        })
        .max_age(tower_cookies::cookie::time::Duration::ZERO)
        .build();
    cookies.remove(cookie);
    Json(json!({ "ok": true }))
}

/// GET /auth/check - Check if session is valid
pub(crate) async fn auth_check(
    State(state): State<AppState>,
    cookies: Cookies,
) -> impl IntoResponse {
    // If no token configured, always authenticated
    if state.auth_token.is_none() {
        return Json(json!({ "authenticated": true, "required": false }));
    }

    let is_authenticated = cookies.get(AUTH_COOKIE_NAME).is_some();
    Json(json!({ "authenticated": is_authenticated, "required": true }))
}
