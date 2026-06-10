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
use crate::{AppState, MiddlewareErrorResponse, AUTH_COOKIE_NAME};

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

    // Cookie must exist AND validate against the cross-process session
    // store (which revalidates against the on-disk state, so a revoke
    // from another process is honored here immediately).
    let session_valid = cookies
        .get(AUTH_COOKIE_NAME)
        .map(|c| state.sessions.is_valid(c.value()))
        .unwrap_or(false);
    if session_valid {
        return next.run(request).await;
    }

    // Bearer token is a first-class auth path for programmatic clients
    // (scripts, reverse proxies, tests) alongside the session cookie.
    if let Some(token) = bearer_token(&headers) {
        if let Some(expected) = state.auth_token.as_deref() {
            if verify_token(expected, token) {
                return next.run(request).await;
            }
        }
    }

    // No valid session
    let response = MiddlewareErrorResponse {
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
/// Failures are returned as 429 with the middleware error envelope
/// (`{ ok, data, error }`).
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
            let response = MiddlewareErrorResponse {
                ok: false,
                data: None,
                error: Some("Rate limit exceeded. Please try again later.".to_string()),
            };
            (StatusCode::TOO_MANY_REQUESTS, Json(response)).into_response()
        }
    }
}

/// Resolve the client IP for rate-limit keying.
///
/// When the direct peer is inside a configured trusted-proxy CIDR
/// (`SPIRITSTREAM_TRUSTED_PROXIES` — mandatory in cloud mode, where a
/// TLS reverse proxy fronts the server), walk `X-Forwarded-For` from
/// RIGHT to LEFT, skipping trusted hops; the first untrusted entry is
/// the client. Rightmost-untrusted is the only trustworthy reading —
/// the left entries are client-supplied and trivially spoofable. When
/// the peer is NOT a trusted proxy, the header is ignored entirely.
/// Pre-fix the key was always the direct peer, so behind the mandatory
/// cloud proxy every client shared ONE login bucket: any single
/// attacker (or just normal traffic) could 429-lock login for the
/// whole deployment.
pub(crate) fn client_ip(
    peer: std::net::IpAddr,
    headers: &HeaderMap,
    trusted: &[ipnet::IpNet],
) -> std::net::IpAddr {
    if trusted.is_empty() || !trusted.iter().any(|net| net.contains(&peer)) {
        return peer;
    }
    let Some(xff) = headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
    else {
        return peer;
    };
    for entry in xff.split(',').rev() {
        let Some(ip) = parse_forwarded_ip(entry.trim()) else {
            // Unparseable hop → refuse to trust the header at all.
            return peer;
        };
        if !trusted.iter().any(|net| net.contains(&ip)) {
            return ip;
        }
    }
    peer
}

/// Parse one `X-Forwarded-For` entry: bare IP, `ip:port`,
/// `[v6]`/`[v6]:port` all occur in the wild.
fn parse_forwarded_ip(entry: &str) -> Option<std::net::IpAddr> {
    if let Ok(ip) = entry.parse::<std::net::IpAddr>() {
        return Some(ip);
    }
    if let Ok(sock) = entry.parse::<SocketAddr>() {
        return Some(sock.ip());
    }
    entry
        .strip_prefix('[')
        .and_then(|rest| rest.strip_suffix(']'))
        .and_then(|inner| inner.parse::<std::net::IpAddr>().ok())
}

/// Resolve the `(limiter, key)` pair for a request. Login uses client IP
/// (proxy-aware — see [`client_ip`]) because the caller is by definition
/// unauthenticated; every other quota is keyed on the auth-subject hash
/// so an attacker who steals one token can't burn the entire shared
/// budget.
fn select_limiter<'a>(
    state: &'a AppState,
    path: &str,
    method: &Method,
    cookies: &Cookies,
    headers: &HeaderMap,
    peer: SocketAddr,
) -> (&'a KeyedLimiter, String) {
    let peer_ip = client_ip(peer.ip(), headers, &state.trusted_proxies).to_string();
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
    if method == Method::POST && path == "/api/v1/security/confirm-token" {
        return (&state.endpoint_limiters.confirm_token, subject);
    }
    if method == Method::POST && path == "/api/v1/security/sessions/revoke-all" {
        return (&state.endpoint_limiters.revoke_sessions, subject);
    }
    if method == Method::POST && path == "/api/v1/security/machine-key/rotate" {
        return (&state.endpoint_limiters.machine_key_rotate, subject);
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
        let response = MiddlewareErrorResponse {
            ok: false,
            data: None,
            error: Some("CSRF: request origin not allowed".to_string()),
        };
        return (StatusCode::FORBIDDEN, Json(response)).into_response();
    }

    next.run(request).await
}

#[cfg(test)]
mod client_ip_tests {
    use super::*;
    use std::net::IpAddr;

    fn ip(s: &str) -> IpAddr {
        s.parse().unwrap()
    }

    fn nets(list: &[&str]) -> Vec<ipnet::IpNet> {
        list.iter().map(|s| s.parse().unwrap()).collect()
    }

    fn headers_with_xff(value: &str) -> HeaderMap {
        let mut h = HeaderMap::new();
        h.insert("x-forwarded-for", value.parse().unwrap());
        h
    }

    /// Untrusted peer → XFF is attacker-controlled, ignore it.
    #[test]
    fn xff_from_untrusted_peer_is_ignored() {
        let trusted = nets(&["172.18.0.0/16"]);
        let got = client_ip(
            ip("203.0.113.9"),
            &headers_with_xff("10.0.0.1"),
            &trusted,
        );
        assert_eq!(got, ip("203.0.113.9"));
    }

    /// Trusted proxy peer → rightmost-untrusted XFF entry is the client.
    #[test]
    fn rightmost_untrusted_entry_wins() {
        let trusted = nets(&["172.18.0.0/16"]);
        // Client spoofed "1.1.1.1"; proxy appended the real client.
        let got = client_ip(
            ip("172.18.0.2"),
            &headers_with_xff("1.1.1.1, 198.51.100.7"),
            &trusted,
        );
        assert_eq!(got, ip("198.51.100.7"));
    }

    /// Chained trusted proxies are skipped right-to-left.
    #[test]
    fn trusted_hops_are_skipped() {
        let trusted = nets(&["172.18.0.0/16"]);
        let got = client_ip(
            ip("172.18.0.2"),
            &headers_with_xff("198.51.100.7, 172.18.0.3"),
            &trusted,
        );
        assert_eq!(got, ip("198.51.100.7"));
    }

    /// No trusted proxies configured (desktop default) → always peer.
    #[test]
    fn no_config_means_peer_only() {
        let got = client_ip(ip("203.0.113.9"), &headers_with_xff("10.0.0.1"), &[]);
        assert_eq!(got, ip("203.0.113.9"));
    }

    /// Garbage in the header → refuse to trust any of it.
    #[test]
    fn unparseable_hop_falls_back_to_peer() {
        let trusted = nets(&["172.18.0.0/16"]);
        let got = client_ip(
            ip("172.18.0.2"),
            &headers_with_xff("not-an-ip, 198.51.100.7"),
            &trusted,
        );
        // The rightmost entry is valid+untrusted so it wins before the
        // garbage is reached…
        assert_eq!(got, ip("198.51.100.7"));
        // …but garbage in the rightmost position kills the whole header.
        let got = client_ip(
            ip("172.18.0.2"),
            &headers_with_xff("198.51.100.7, not-an-ip"),
            &trusted,
        );
        assert_eq!(got, ip("172.18.0.2"));
    }

    /// `ip:port` and bracketed IPv6 forms parse.
    #[test]
    fn port_and_v6_forms_parse() {
        let trusted = nets(&["172.18.0.0/16"]);
        let got = client_ip(
            ip("172.18.0.2"),
            &headers_with_xff("198.51.100.7:4711"),
            &trusted,
        );
        assert_eq!(got, ip("198.51.100.7"));
        let got = client_ip(
            ip("172.18.0.2"),
            &headers_with_xff("[2001:db8::1]"),
            &trusted,
        );
        assert_eq!(got, ip("2001:db8::1"));
    }
}
