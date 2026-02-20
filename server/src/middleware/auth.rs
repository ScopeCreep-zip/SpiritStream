use axum::{
    extract::{Json, State},
    http::{header, HeaderMap, Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use subtle::ConstantTimeEq;
use tower_cookies::{Cookie, Cookies};

use crate::app_state::AppState;

pub const AUTH_COOKIE_NAME: &str = "spiritstream_session";
pub const COOKIE_MAX_AGE_SECS: i64 = 7 * 24 * 60 * 60; // 7 days

/// Thread-safe session store for validating cookie values
static ACTIVE_SESSIONS: std::sync::LazyLock<std::sync::Arc<std::sync::RwLock<std::collections::HashSet<String>>>> =
    std::sync::LazyLock::new(|| std::sync::Arc::new(std::sync::RwLock::new(std::collections::HashSet::new())));

fn register_session(session_id: &str) {
    if let Ok(mut sessions) = ACTIVE_SESSIONS.write() {
        sessions.insert(session_id.to_string());
    }
}

fn invalidate_session(session_id: &str) {
    if let Ok(mut sessions) = ACTIVE_SESSIONS.write() {
        sessions.remove(session_id);
    }
}

pub fn is_valid_session(session_id: &str) -> bool {
    ACTIVE_SESSIONS.read().is_ok_and(|s| s.contains(session_id))
}
pub const DEFAULT_RATE_LIMIT_PER_MINUTE: u32 = 300;

#[derive(Serialize)]
pub struct InvokeResponse {
    pub ok: bool,
    pub data: Option<Value>,
    pub error: Option<String>,
}

/// Constant-time token comparison to prevent timing attacks
pub fn verify_token(expected: &str, provided: &str) -> bool {
    expected.as_bytes().ct_eq(provided.as_bytes()).into()
}

/// Extract bearer token from Authorization header
pub fn bearer_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

/// Sanitize error messages to prevent information disclosure
pub fn sanitize_error(error: &str) -> String {
    let lower = error.to_lowercase();

    if lower.contains("failed to read") || lower.contains("no such file") || lower.contains("not found") {
        return "Resource not found".to_string();
    }
    if lower.contains("parse") || lower.contains("invalid") {
        return "Invalid request format".to_string();
    }
    if lower.contains("permission") || lower.contains("access") || lower.contains("denied") {
        return "Access denied".to_string();
    }
    if lower.contains("traversal") || lower.contains("outside") {
        return "Invalid path".to_string();
    }
    if lower.contains("encrypt") || lower.contains("decrypt") {
        return "Encryption error".to_string();
    }

    // Return generic message for unknown errors in production
    // In debug mode, we could log the actual error server-side
    log::debug!("Sanitized error: {error}");
    "Operation failed".to_string()
}

pub fn parse_bool(value: &str) -> Option<bool> {
    match value.trim().to_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}

#[derive(Deserialize)]
pub struct LoginRequest {
    token: String,
}

/// Set a session cookie and register the session server-side
fn set_session_cookie(cookies: &Cookies) {
    let session_id = uuid::Uuid::new_v4().to_string();
    register_session(&session_id);
    let cookie = Cookie::build((AUTH_COOKIE_NAME, session_id))
        .http_only(true)
        .secure(false) // Set to true when using HTTPS
        .same_site(tower_cookies::cookie::SameSite::Strict)
        .path("/")
        .max_age(tower_cookies::cookie::time::Duration::seconds(COOKIE_MAX_AGE_SECS))
        .build();
    cookies.add(cookie);
}

/// POST /auth/login - Validate token and set HttpOnly cookie
pub async fn auth_login(
    State(state): State<AppState>,
    cookies: Cookies,
    Json(payload): Json<LoginRequest>,
) -> impl IntoResponse {
    let expected_token = state.auth_token.as_deref();

    match expected_token {
        None => {
            // No token configured - open access, set session cookie anyway
            set_session_cookie(&cookies);
            Json(json!({ "ok": true }))
        }
        Some(expected) if verify_token(expected, &payload.token) => {
            set_session_cookie(&cookies);
            Json(json!({ "ok": true }))
        }
        _ => {
            // Invalid token - add a small delay to prevent brute force
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            Json(json!({ "ok": false, "error": "Invalid token" }))
        }
    }
}

/// POST /auth/logout - Clear session cookie and invalidate server-side session
pub async fn auth_logout(cookies: Cookies) -> impl IntoResponse {
    // Invalidate the session server-side before removing the cookie
    if let Some(session_cookie) = cookies.get(AUTH_COOKIE_NAME) {
        invalidate_session(session_cookie.value());
    }
    let cookie = Cookie::build((AUTH_COOKIE_NAME, ""))
        .path("/")
        .max_age(tower_cookies::cookie::time::Duration::ZERO)
        .build();
    cookies.remove(cookie);
    Json(json!({ "ok": true }))
}

/// GET /auth/check - Check if session is valid
pub async fn auth_check(
    State(state): State<AppState>,
    cookies: Cookies,
) -> impl IntoResponse {
    // If no token configured, always authenticated
    if state.auth_token.is_none() {
        return Json(json!({ "authenticated": true, "required": false }));
    }

    let is_authenticated = cookies
        .get(AUTH_COOKIE_NAME)
        .is_some_and(|c| is_valid_session(c.value()));
    Json(json!({ "authenticated": is_authenticated, "required": true }))
}

/// Authentication middleware - check for valid session cookie
pub async fn auth_middleware(
    State(state): State<AppState>,
    cookies: Cookies,
    headers: HeaderMap,
    request: Request<axum::body::Body>,
    next: Next,
) -> Response {
    // If no token configured, allow all requests
    if state.auth_token.is_none() {
        return next.run(request).await;
    }

    // Check for valid session cookie (validate value against server-side session store)
    if cookies
        .get(AUTH_COOKIE_NAME)
        .is_some_and(|c| is_valid_session(c.value()))
    {
        return next.run(request).await;
    }

    // Also accept Bearer token for backwards compatibility and programmatic access
    if let Some(token) = bearer_token(&headers) {
        if let Some(expected) = state.auth_token.as_deref() {
            if verify_token(expected, token) {
                return next.run(request).await;
            }
        }
    }

    // No valid session
    let response = InvokeResponse {
        ok: false,
        data: None,
        error: Some("Authentication required".to_string()),
    };
    (StatusCode::UNAUTHORIZED, Json(response)).into_response()
}

/// Rate limiting middleware
pub async fn rate_limit_middleware(
    State(state): State<AppState>,
    request: Request<axum::body::Body>,
    next: Next,
) -> Response {
    match state.rate_limiter.check() {
        Ok(_) => next.run(request).await,
        Err(_) => {
            let response = InvokeResponse {
                ok: false,
                data: None,
                error: Some("Rate limit exceeded. Please try again later.".to_string()),
            };
            (StatusCode::TOO_MANY_REQUESTS, Json(response)).into_response()
        }
    }
}

pub fn unauthorized_response() -> impl IntoResponse {
    let response = InvokeResponse {
        ok: false,
        data: None,
        error: Some("Unauthorized".to_string()),
    };
    (StatusCode::UNAUTHORIZED, Json(response))
}
