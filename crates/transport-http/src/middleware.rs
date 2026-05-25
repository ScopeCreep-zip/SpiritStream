use std::net::SocketAddr;

use axum::{
    extract::{ConnectInfo, State},
    http::{header, HeaderMap, HeaderValue, Method, Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use tower_cookies::Cookies;

use crate::auth_helpers::{bearer_token, verify_token};
use crate::cors::origin_matches;
use crate::rate_limit::KeyedLimiter;
use crate::{AppState, InvokeResponse, AUTH_COOKIE_NAME};

/// Authentication middleware - check for valid session cookie.
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

    // Cookie must exist AND be in the active-session set.
    // The MutexGuard MUST drop before any `await` or the future stops
    // being `Send`; we compute the predicate first, then await.
    let session_valid = cookies
        .get(AUTH_COOKIE_NAME)
        .map(|c| {
            let sessions = state
                .active_sessions
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            sessions.contains(c.value())
        })
        .unwrap_or(false);
    if session_valid {
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

/// Per-endpoint rate limiting middleware.
///
/// Inspects the route and picks the corresponding `KeyedLimiter` from
/// [`crate::rate_limit::EndpointRateLimiters`]. The key is the auth subject
/// when an auth cookie or Bearer token is present, otherwise the peer IP
/// from `ConnectInfo` (or `"unknown"` if the server is reached through a
/// path that doesn't surface `ConnectInfo` — primarily test harnesses).
///
/// Failures are returned as 429 with a JSON body matching the
/// `InvokeResponse` envelope so existing CLI parsers keep working.
pub(crate) async fn rate_limit_middleware(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    cookies: Cookies,
    headers: HeaderMap,
    request: Request<axum::body::Body>,
    next: Next,
) -> Response {
    let path = request.uri().path();
    let method = request.method();

    let (limiter, key) = select_limiter(&state, path, method, &cookies, &headers, addr);

    match limiter.check_key(&key) {
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

/// Resolve the `(limiter, key)` pair for a request. Login uses peer IP
/// because the caller is by definition unauthenticated; every other
/// quota is keyed on the auth-subject hash so an attacker who steals one
/// token can't burn the entire shared budget.
fn select_limiter<'a>(
    state: &'a AppState,
    path: &str,
    method: &Method,
    cookies: &Cookies,
    headers: &HeaderMap,
    peer: SocketAddr,
) -> (&'a KeyedLimiter, String) {
    let peer_ip = peer.ip().to_string();
    let subject = subject_key(cookies, headers).unwrap_or_else(|| peer_ip.clone());

    if method == Method::POST && path == "/api/v1/auth/login" {
        return (&state.endpoint_limiters.login, peer_ip);
    }
    if method == Method::POST && path == "/api/v1/chat/messages" {
        return (&state.endpoint_limiters.chat_send, subject);
    }
    if method == Method::POST && path == "/api/v1/streams" {
        return (&state.endpoint_limiters.stream_start, subject);
    }
    if method == Method::POST && path.starts_with("/api/v1/oauth/") && path.ends_with("/flow") {
        return (&state.endpoint_limiters.oauth_flow, subject);
    }
    (&state.endpoint_limiters.default_auth, subject)
}

/// Derive a stable rate-limit subject key from the request's
/// authentication material. Returns `None` when no auth material is
/// present — callers fall back to peer IP. The key is a SHA-256 of the
/// token bytes (truncated for compactness) so the secret never appears
/// in the limiter's in-memory state.
fn subject_key(cookies: &Cookies, headers: &HeaderMap) -> Option<String> {
    use sha2::{Digest, Sha256};

    let token = cookies
        .get(AUTH_COOKIE_NAME)
        .map(|c| c.value().to_string())
        .or_else(|| bearer_token(headers).map(|t| t.to_string()))?;

    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    let digest = hasher.finalize();
    Some(hex::encode(&digest[..16])) // 128-bit prefix is plenty for keying
}

/// Request-ID middleware. Generates a UUID v7 (time-ordered)
/// at the transport entry if the client didn't send one, then echoes
/// it back as `X-Request-Id` on the response. Downstream handlers can
/// read the value via the request extension; the value also flows into
/// `tracing` spans once the tracing migration runs.
pub(crate) async fn request_id_middleware(
    mut request: Request<axum::body::Body>,
    next: Next,
) -> Response {
    let header_name = "x-request-id";
    let incoming = request
        .headers()
        .get(header_name)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| {
            if s.len() <= 64 && !s.is_empty() {
                Some(s.to_string())
            } else {
                None
            }
        });
    let rid = RequestId(incoming.unwrap_or_else(|| uuid::Uuid::now_v7().to_string()));
    // Store on the request extension so handlers can pull it out via
    // `Extension<RequestId>`.
    request.extensions_mut().insert(rid.clone());

    let mut response = next.run(request).await;
    // Echo the canonical id on the response by reading the struct
    // field through `RequestId::as_str` — the field accessor is the
    // public contract `Extension<RequestId>` consumers use too.
    if let Ok(value) = HeaderValue::from_str(rid.as_str()) {
        response.headers_mut().insert("x-request-id", value);
    }
    response
}

/// Per-request correlation id. Constructed in `request_id_middleware`,
/// stored in the request extension, and echoed on the response via the
/// `X-Request-Id` header. Handlers extract it with `Extension<RequestId>`
/// when they want to thread the id into logs or audit entries.
#[derive(Debug, Clone)]
pub(crate) struct RequestId(pub String);

impl RequestId {
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

/// CSRF guard.
///
/// Defends against cross-site request forgery on every state-changing
/// request (POST/PUT/PATCH/DELETE) and on WebSocket upgrades — a forged
/// upgrade equals a forged push channel, so we treat it the same as a
/// mutating request even though the underlying HTTP method is GET.
///
/// Policy:
/// 1. Pure-read methods (GET/HEAD/OPTIONS) without a WebSocket upgrade are
///    waved through — the browser will not run an authorized cross-site
///    mutation through them.
/// 2. `Sec-Fetch-Site` is the primary signal. `same-origin`, `same-site`,
///    and `none` (direct user action — typed URL, bookmark, redirect from
///    address bar) all pass. `cross-site` and `cross-origin` fall through
///    to the Origin allow-list so the Tauri 2 webview
///    (`tauri://localhost`, `https://tauri.localhost`) still works.
/// 3. The Origin allow-list is the secondary signal. It is also the only
///    check when `Sec-Fetch-Site` is absent (older browsers, CLI tools).
/// 4. Requests with neither header are allowed — these are CLI tools and
///    fall through to the existing `auth_middleware` token check.
///
/// Sources: OWASP CSRF cheat sheet (2024), MDN Sec-Fetch-Site.
pub(crate) async fn csrf_middleware(
    State(state): State<AppState>,
    request: Request<axum::body::Body>,
    next: Next,
) -> Response {
    let method = request.method().clone();
    let headers = request.headers();

    let is_ws_upgrade = headers
        .get(header::UPGRADE)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.eq_ignore_ascii_case("websocket"))
        .unwrap_or(false);

    let is_state_changing = matches!(
        method,
        Method::POST | Method::PUT | Method::PATCH | Method::DELETE
    );

    if !is_state_changing && !is_ws_upgrade {
        return next.run(request).await;
    }

    let sec_fetch_site = headers.get("Sec-Fetch-Site").and_then(|v| v.to_str().ok());
    let origin = headers.get(header::ORIGIN).and_then(|v| v.to_str().ok());

    let allowed = match sec_fetch_site {
        Some("same-origin") | Some("same-site") | Some("none") => true,
        Some(_) => origin
            .map(|o| origin_matches(o, &state.allowed_origins))
            .unwrap_or(false),
        None => match origin {
            Some(o) => origin_matches(o, &state.allowed_origins),
            None => true, // CLI tool — defer to auth_middleware
        },
    };

    if !allowed {
        let response = InvokeResponse {
            ok: false,
            data: None,
            error: Some("CSRF: request origin not allowed".to_string()),
        };
        return (StatusCode::FORBIDDEN, Json(response)).into_response();
    }

    next.run(request).await
}
