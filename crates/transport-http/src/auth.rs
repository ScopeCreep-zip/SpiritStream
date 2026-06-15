use axum::{extract::State, http::HeaderMap, Json};
use serde::{Deserialize, Serialize};
use tower_cookies::{Cookie, Cookies};
use utoipa::ToSchema;

use spiritstream_core::services::{CONFIRM_TOKEN_TTL_SECS, EVENT_TICKET_TTL_SECS};

use crate::auth_helpers::verify_token;
use crate::error::ApiError;
use crate::session::SessionCookieMode;
use crate::{AppState, AUTH_COOKIE_NAME, COOKIE_MAX_AGE_SECS};

#[derive(Deserialize)]
pub(crate) struct LoginRequest {
    token: String,
}

// Typed REST response structs for the auth + session-management surface.
// All success responses are direct `Json<T>`; failures route through
// `ApiError(CoreError)` per the transport contract.

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct AuthLoginResponse {}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub(crate) struct AuthLogoutResponse {}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AuthCheckResponse {
    pub authenticated: bool,
    pub required: bool,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RevokeAllSessionsResponse {
    pub revoked: usize,
}

const AUTH_ACCOUNT_DEFAULT: &str = "default";

fn lockout_error(remaining: std::time::Duration) -> ApiError {
    let secs = remaining.as_secs().max(1) as u32;
    ApiError(spiritstream_core::CoreError::RateLimited {
        retry_after_secs: secs,
    })
}

/// Build a hardened session cookie under the configured [`SessionCookieMode`]
/// and register the new session ID with the server-side active-sessions
/// set.
///
/// Attribute matrix:
/// * `SameOrigin`   → `Secure; HttpOnly; SameSite=Strict`
/// * `CrossOrigin`  → `Secure; HttpOnly; SameSite=Lax`
/// * `LocalhostDev` → `HttpOnly; SameSite=Strict` (no Secure over plain HTTP)
///
/// All variants set `Path=/` and `Max-Age = COOKIE_MAX_AGE_SECS` (7 days).
fn set_session_cookie(state: &AppState, cookies: &Cookies) -> Result<(), ApiError> {
    let session_id = uuid::Uuid::new_v4().to_string();
    state.sessions.insert(&session_id)?;
    let mode: SessionCookieMode = state.cookie_mode;
    let cookie = Cookie::build((AUTH_COOKIE_NAME, session_id))
        .http_only(true)
        .secure(mode.secure())
        .same_site(mode.same_site())
        .path("/")
        .max_age(tower_cookies::cookie::time::Duration::seconds(
            COOKIE_MAX_AGE_SECS,
        ))
        .build();
    cookies.add(cookie);
    Ok(())
}

/// `POST /api/v1/auth/login` — validate token, set HttpOnly cookie.
///
/// Brute-force defense layered on top of the per-IP rate limit:
/// 1. If the account is currently locked (10 failures within 1h →
///    15min lockout), reply 429 with `Retry-After`. No timing work
///    happens so the rejection is cheap.
/// 2. If the exponential-backoff window since the last failure has not
///    elapsed, reply 429 with `Retry-After` BEFORE any verification —
///    the old design only slept before responding, which a pipelined
///    attacker bypassed by firing requests and ignoring the replies
///    (every request still got an immediate `verify_token` call).
/// 3. Otherwise verify the token. Success resets failure state and
///    sets the session cookie; failure records the attempt (which may
///    trip the lockout → 429) and returns 401 immediately — the gate
///    in step 2 enforces the wait on the NEXT attempt.
pub(crate) async fn auth_login(
    State(state): State<AppState>,
    cookies: Cookies,
    Json(payload): Json<LoginRequest>,
) -> Result<Json<AuthLoginResponse>, ApiError> {
    let account = AUTH_ACCOUNT_DEFAULT;

    if let Some(remaining) = state.auth_service.check_locked(account) {
        return Err(lockout_error(remaining));
    }
    if let Some(remaining) = state.auth_service.check_backoff(account) {
        return Err(lockout_error(remaining));
    }

    let expected_token = state.auth_token.as_deref();
    let token_ok = match expected_token {
        None => true,
        Some(expected) => verify_token(expected, &payload.token),
    };

    if token_ok {
        state.auth_service.record_success(account);
        set_session_cookie(&state, &cookies)?;
        return Ok(Json(AuthLoginResponse {}));
    }

    log::warn!("auth_login: invalid token");
    match state.auth_service.record_failure(account) {
        Ok(_next_backoff) => Err(ApiError(spiritstream_core::CoreError::Unauthorized)),
        Err(remaining) => Err(lockout_error(remaining)),
    }
}

/// POST /api/v1/auth/logout — drop session ID from active set, clear cookie.
#[utoipa::path(
    post,
    path = "/auth/logout",
    tag = "auth",
    responses((status = 200, description = "Session ended; cookie cleared.", body = AuthLogoutResponse)),
    security(("session_cookie" = []), ("bearer" = [])),
)]
pub(crate) async fn auth_logout(
    State(state): State<AppState>,
    cookies: Cookies,
) -> Result<Json<AuthLogoutResponse>, ApiError> {
    if let Some(c) = cookies.get(AUTH_COOKIE_NAME) {
        state.sessions.remove(c.value())?;
    }
    let cookie = Cookie::build((AUTH_COOKIE_NAME, ""))
        .path("/")
        .max_age(tower_cookies::cookie::time::Duration::ZERO)
        .build();
    cookies.remove(cookie);
    Ok(Json(AuthLogoutResponse {}))
}

/// `POST /api/v1/security/sessions/revoke-all` — drop every active
/// session ID. Existing cookies fail the next `auth_middleware` check
/// even though the client still possesses them. Requires a confirm
/// token issued for the `revoke_all_sessions` intent.
#[utoipa::path(
    post,
    path = "/security/sessions/revoke-all",
    tag = "security",
    responses(
        (status = 200, description = "All sessions revoked; body carries the count.", body = RevokeAllSessionsResponse),
        (status = 403, description = "Missing or invalid X-Confirm-Token.", body = ApiErrorBody),
    ),
    security(("session_cookie" = []), ("bearer" = [])),
)]
pub(crate) async fn security_revoke_all_sessions(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<RevokeAllSessionsResponse>, ApiError> {
    require_confirm_token(&state, &headers, "revoke_all_sessions")?;
    let revoked = state.sessions.revoke_all()?;
    // G2: durable post-incident breadcrumb. If an attacker stole a
    // session and the legit user hit panic-revoke, the chain shows
    // `SessionRevoked { count }` with the wall-clock time + a paired
    // `ConfirmTokenIssued { intent: "revoke_all_sessions" }` from the
    // issue step. Use that pair to reconstruct "user demanded all
    // sessions out at T, N were live" in post-incident review.
    if let Err(e) = state
        .audit
        .record(spiritstream_core::services::AuditAction::SessionRevoked { count: revoked })
    {
        log::error!("auth: SessionRevoked audit append failed: {e}");
    }
    Ok(Json(RevokeAllSessionsResponse { revoked }))
}

/// GET /api/v1/auth/check — is the current session valid?
///
/// "Valid" means: cookie is present AND its value is in the
/// server-side active-session set. A revoked session keeps the raw cookie
/// in the browser but fails this check immediately.
pub(crate) async fn auth_check(
    State(state): State<AppState>,
    cookies: Cookies,
) -> Result<Json<AuthCheckResponse>, ApiError> {
    if state.auth_token.is_none() {
        return Ok(Json(AuthCheckResponse {
            authenticated: true,
            required: false,
        }));
    }
    let is_authenticated = cookies
        .get(AUTH_COOKIE_NAME)
        .map(|c| state.sessions.is_valid(c.value()))
        .unwrap_or(false);
    Ok(Json(AuthCheckResponse {
        authenticated: is_authenticated,
        required: true,
    }))
}

// ============================================================================
// Confirm tokens (one-shot per intent for destructive ops)
// ============================================================================

/// Verify the `X-Confirm-Token` header against the [`ConfirmTokenService`]
/// for the given `intent`, consuming the token on success.
/// Returns an `ApiError(CoreError::PathOutsideAllowedRoot)` shaped 403
/// when missing or invalid — the same flavour of generic forbidden the
/// CSRF layer uses, so frontend error handlers don't need a new branch.
pub(crate) fn require_confirm_token(
    state: &AppState,
    headers: &HeaderMap,
    intent: &str,
) -> Result<(), ApiError> {
    let token = headers
        .get("X-Confirm-Token")
        .and_then(|h| h.to_str().ok())
        .ok_or_else(|| {
            ApiError(spiritstream_core::CoreError::PathOutsideAllowedRoot {
                path: format!("missing X-Confirm-Token for {intent}"),
            })
        })?;
    if !state.confirm_tokens.consume(intent, token) {
        return Err(ApiError(
            spiritstream_core::CoreError::PathOutsideAllowedRoot {
                path: format!("invalid or expired X-Confirm-Token for {intent}"),
            },
        ));
    }
    Ok(())
}

/// Issue a confirmation token. The frontend / CLI calls this immediately
/// before showing the user a "are you sure?" prompt, then includes the
/// returned token in the destructive request's `X-Confirm-Token` header.
#[derive(Deserialize, ToSchema)]
pub(crate) struct ConfirmTokenRequest {
    intent: String,
}

#[derive(Serialize, ToSchema)]
pub(crate) struct ConfirmTokenResponse {
    token: String,
    /// Seconds the token remains valid. Mirrors `CONFIRM_TOKEN_TTL_SECS`.
    #[serde(rename = "expiresInSeconds")]
    expires_in_seconds: u64,
}

/// One-shot ticket for the `/api/v1/events` WebSocket upgrade.
///
/// Browsers can't attach an Authorization header to a WS upgrade, and
/// cross-origin deployments don't send the SameSite=Lax cookie on it
/// either. The client calls this (authenticated) endpoint immediately
/// before connecting and passes `?ticket=` on the upgrade URL; the
/// ticket is single-use and expires in seconds, so a proxy-logged URL
/// is dead on arrival — unlike the previous design, which put a
/// long-lived bearer token in the query string (and which no
/// production code could even use, since the auth middleware rejected
/// the upgrade before the token check ran).
#[derive(Serialize, ToSchema)]
pub(crate) struct EventTicketResponse {
    ticket: String,
    #[serde(rename = "expiresInSeconds")]
    expires_in_seconds: u64,
}

#[utoipa::path(
    post,
    path = "/events/ticket",
    tag = "events",
    responses((status = 200, description = "One-shot WebSocket upgrade ticket + its TTL.", body = EventTicketResponse)),
    security(("session_cookie" = []), ("bearer" = [])),
)]
pub(crate) async fn events_ticket_issue(
    State(state): State<AppState>,
) -> Result<Json<EventTicketResponse>, ApiError> {
    Ok(Json(EventTicketResponse {
        ticket: state.event_tickets.issue(),
        expires_in_seconds: EVENT_TICKET_TTL_SECS,
    }))
}

#[utoipa::path(
    post,
    path = "/security/confirm-token",
    tag = "security",
    request_body = ConfirmTokenRequest,
    responses(
        (status = 200, description = "One-shot intent-scoped token + its TTL.", body = ConfirmTokenResponse),
        (status = 400, description = "Unknown or malformed intent.", body = ApiErrorBody),
    ),
    security(("session_cookie" = []), ("bearer" = [])),
)]
pub(crate) async fn confirm_token_issue(
    State(state): State<AppState>,
    Json(payload): Json<ConfirmTokenRequest>,
) -> Result<Json<ConfirmTokenResponse>, ApiError> {
    let token = state.confirm_tokens.issue(&payload.intent)?;
    // G2: record the intent (NOT the token) so post-incident review
    // can reconstruct "user asked for X around time T." Paired with
    // the downstream `MachineKeyRotated` / `SessionRevoked` /
    // `ProfileDeleted` / etc. emission via wall-clock proximity.
    if let Err(e) = state.audit.record(
        spiritstream_core::services::AuditAction::ConfirmTokenIssued {
            intent: payload.intent.clone(),
        },
    ) {
        log::error!("auth: ConfirmTokenIssued audit append failed: {e}");
    }
    Ok(Json(ConfirmTokenResponse {
        token,
        expires_in_seconds: CONFIRM_TOKEN_TTL_SECS,
    }))
}
