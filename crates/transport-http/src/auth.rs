use axum::{
    extract::State,
    http::HeaderMap,
    Json,
};
use serde::{Deserialize, Serialize};
use tower_cookies::{Cookie, Cookies};

use spiritstream_core::services::CONFIRM_TOKEN_TTL_SECS;

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

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct AuthLogoutResponse {}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AuthCheckResponse {
    pub authenticated: bool,
    pub required: bool,
}

#[derive(Debug, Serialize, Deserialize)]
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
fn set_session_cookie(state: &AppState, cookies: &Cookies) {
    let session_id = uuid::Uuid::new_v4().to_string();
    {
        let mut sessions = state
            .active_sessions
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        sessions.insert(session_id.clone());
    }
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
}

/// `POST /api/v1/auth/login` — validate token, set HttpOnly cookie.
///
/// Brute-force defense layered on top of the per-IP rate limit:
/// 1. If the account is currently locked (10 failures within 1h →
///    15min lockout), reply 429 with `Retry-After`. No timing work
///    happens so the rejection is cheap.
/// 2. Otherwise verify the token. On success, reset failure state and
///    set the session cookie.
/// 3. On failure, record the attempt; sleep for the resulting
///    exponential backoff before responding (so a pipelined attacker
///    can't bypass the delay by ignoring our response). If the failure
///    pushed the account into a fresh lockout, surface 429.
pub(crate) async fn auth_login(
    State(state): State<AppState>,
    cookies: Cookies,
    Json(payload): Json<LoginRequest>,
) -> Result<Json<AuthLoginResponse>, ApiError> {
    let account = AUTH_ACCOUNT_DEFAULT;

    if let Some(remaining) = state.auth_service.check_locked(account) {
        return Err(lockout_error(remaining));
    }

    let expected_token = state.auth_token.as_deref();
    let token_ok = match expected_token {
        None => true,
        Some(expected) => verify_token(expected, &payload.token),
    };

    if token_ok {
        state.auth_service.record_success(account);
        set_session_cookie(&state, &cookies);
        return Ok(Json(AuthLoginResponse {}));
    }

    log::warn!("auth_login: invalid token");
    match state.auth_service.record_failure(account) {
        Ok(backoff) => {
            // Sleep before returning so pipelined attackers can't outpace
            // the policy by ignoring our response timing.
            tokio::time::sleep(backoff).await;
            Err(ApiError(spiritstream_core::CoreError::Unauthorized))
        }
        Err(remaining) => Err(lockout_error(remaining)),
    }
}

/// POST /api/v1/auth/logout — drop session ID from active set, clear cookie.
pub(crate) async fn auth_logout(
    State(state): State<AppState>,
    cookies: Cookies,
) -> Result<Json<AuthLogoutResponse>, ApiError> {
    if let Some(c) = cookies.get(AUTH_COOKIE_NAME) {
        let mut sessions = state
            .active_sessions
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        sessions.remove(c.value());
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
pub(crate) async fn security_revoke_all_sessions(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<RevokeAllSessionsResponse>, ApiError> {
    require_confirm_token(&state, &headers, "revoke_all_sessions")?;
    let mut sessions = state
        .active_sessions
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let revoked = sessions.len();
    sessions.clear();
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
    let is_authenticated = match cookies.get(AUTH_COOKIE_NAME) {
        Some(c) => {
            let sessions = state
                .active_sessions
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            sessions.contains(c.value())
        }
        None => false,
    };
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
#[derive(Deserialize)]
pub(crate) struct ConfirmTokenRequest {
    intent: String,
}

#[derive(Serialize)]
pub(crate) struct ConfirmTokenResponse {
    token: String,
    /// Seconds the token remains valid. Mirrors `CONFIRM_TOKEN_TTL_SECS`.
    #[serde(rename = "expiresInSeconds")]
    expires_in_seconds: u64,
}

pub(crate) async fn confirm_token_issue(
    State(state): State<AppState>,
    Json(payload): Json<ConfirmTokenRequest>,
) -> Result<Json<ConfirmTokenResponse>, ApiError> {
    let token = state.confirm_tokens.issue(&payload.intent);
    Ok(Json(ConfirmTokenResponse {
        token,
        expires_in_seconds: CONFIRM_TOKEN_TTL_SECS,
    }))
}
