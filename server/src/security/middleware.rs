use axum::{
    extract::{Json, State},
    http::{HeaderMap, Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use tower_cookies::Cookies;

use crate::constants::AUTH_COOKIE_NAME;
use crate::security::auth::{bearer_token, verify_token};
use crate::state::{AppState, InvokeResponse};

/// Authentication middleware - check for valid session cookie
pub(crate) async fn auth_middleware(
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

    // Check for valid session cookie
    if cookies.get(AUTH_COOKIE_NAME).is_some() {
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
pub(crate) async fn rate_limit_middleware(
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
