//! HTTP error mapping for `spiritstream_core::CoreError`.
//!
//! This is the *only* place in the entire HTTP transport where `CoreError`
//! becomes an HTTP status code and a JSON body. Handlers return
//! `Result<Json<T>, ApiError>` and rely on `From<CoreError> for ApiError` so
//! they never write status mapping inline.
//!
//! Wire-shape contract:
//! - Successful responses are the handler's `Json<T>` value directly.
//! - Error responses are `{ kind: "...", details: { ... } }` exactly as
//!   `CoreError` serializes with `#[serde(tag = "kind", content = "details")]`.
//! - `CoreError::Internal` is redacted on the wire — the `context` field is
//!   logged server-side but **never** sent to the client.

use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Json, Response};
use serde_json::json;
use spiritstream_core::CoreError;

/// Wrap a `CoreError` so it can be returned from any Axum handler.
#[derive(Debug)]
pub struct ApiError(pub CoreError);

impl From<CoreError> for ApiError {
    fn from(err: CoreError) -> Self {
        ApiError(err)
    }
}

// Chain conversions: any error that has `From<E> for CoreError` becomes an
// `ApiError` without each handler writing `.map_err(...)`. Keeps the wire
// contract (kind/details + redacted Internal) intact while letting handlers
// stay bare `?` instead of inlined adaptor closures.
impl From<serde_json::Error> for ApiError {
    fn from(err: serde_json::Error) -> Self {
        ApiError(CoreError::from(err))
    }
}

impl From<std::io::Error> for ApiError {
    fn from(err: std::io::Error) -> Self {
        ApiError(CoreError::from(err))
    }
}

impl From<tokio::task::JoinError> for ApiError {
    fn from(err: tokio::task::JoinError) -> Self {
        ApiError(CoreError::Internal {
            context: format!("Task join error: {err}"),
        })
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = status_for(&self.0);

        // RFC 9110 — 429 responses SHOULD include `Retry-After`. The
        // `CoreError::RateLimited` variant already carries the wait window;
        // surface it here so callers (auth login backoff, future per-endpoint
        // limits) don't have to re-implement header injection per handler.
        let retry_after_header = match &self.0 {
            CoreError::RateLimited { retry_after_secs } => {
                Some(HeaderValue::from_str(&retry_after_secs.to_string()).ok()).flatten()
            }
            _ => None,
        };

        // The `Internal` variant carries `context` that may include service-
        // internal detail (file paths, error messages). Log it server-side
        // and return an opaque body to the client.
        let body = match &self.0 {
            CoreError::Internal { context } => {
                log::error!("internal error: {context}");
                json!({ "kind": "internal" })
            }
            other => {
                // Defense in depth: every other variant gets piped
                // through `redact_payload` before serialisation. If a
                // future variant ever carries a stream key, OAuth
                // token, or anything that matches the redact regexes,
                // the wire body is scrubbed even though no individual
                // variant is supposed to carry secrets today.
                let raw =
                    serde_json::to_value(other).unwrap_or_else(|_| json!({ "kind": "internal" }));
                crate::redact_payload(&raw)
            }
        };

        match retry_after_header {
            Some(value) => (status, [(header::RETRY_AFTER, value)], Json(body)).into_response(),
            None => (status, Json(body)).into_response(),
        }
    }
}

/// HTTP status mapping. Kept as a free function so unit tests can assert it
/// without going through `IntoResponse`.
fn status_for(err: &CoreError) -> StatusCode {
    match err {
        CoreError::ProfileNotFound { .. } => StatusCode::NOT_FOUND,
        CoreError::ProfileAlreadyExists { .. } => StatusCode::CONFLICT,
        CoreError::PasswordRequired { .. } => StatusCode::UNAUTHORIZED,
        CoreError::PasswordIncorrect => StatusCode::UNAUTHORIZED,
        // Weak-password rejection is a client validation failure (400),
        // NOT an authentication failure (401). The user supplied a password
        // we refused to accept — they aren't "wrong" credentials yet.
        CoreError::PasswordTooShort { .. } => StatusCode::BAD_REQUEST,
        CoreError::Unauthorized => StatusCode::UNAUTHORIZED,
        CoreError::NoActiveProfile => StatusCode::CONFLICT,
        // 409: the request is valid but this BUILD can't serve it until
        // credentials are configured — distinct from 400 (caller error).
        CoreError::OAuthProviderNotConfigured { .. } => StatusCode::CONFLICT,
        CoreError::OAuthFlowRequiresDevice { .. } => StatusCode::CONFLICT,
        CoreError::ChatPlatformNotConnected { .. } => StatusCode::UNPROCESSABLE_ENTITY,
        CoreError::ChatSendingDisabled { .. } => StatusCode::UNPROCESSABLE_ENTITY,
        CoreError::ChatMessageLengthExceeded { .. } => StatusCode::BAD_REQUEST,
        // PII filter rejection. 422 (Unprocessable Entity)
        // mirrors the other "rejected by policy" chat errors. The
        // response carries only the phrase_id, never the matched text.
        CoreError::ChatBlockedByPii { .. } => StatusCode::UNPROCESSABLE_ENTITY,
        // A broken anonymous-mode salt is profile data the client can
        // repair (re-save the profile / re-run the safety wizard), so
        // 422 like the other policy rejections — not a 500.
        CoreError::AnonymousSaltInvalid => StatusCode::UNPROCESSABLE_ENTITY,
        CoreError::InvalidStreamConfig { .. } => StatusCode::BAD_REQUEST,
        CoreError::ValidationFailed { .. } => StatusCode::BAD_REQUEST,
        CoreError::EncoderUnavailable { .. } => StatusCode::UNPROCESSABLE_ENTITY,
        CoreError::PortConflict { .. } => StatusCode::CONFLICT,
        CoreError::PathOutsideAllowedRoot { .. } => StatusCode::FORBIDDEN,
        CoreError::NotFound { .. } => StatusCode::NOT_FOUND,
        // FFmpeg is a server-side environment dependency, not request data.
        // RFC 9110: 503 Service Unavailable signals the server is
        // currently unable to handle the request due to a backend condition.
        // 422 (Unprocessable Entity) would imply the client could fix the
        // payload — they can't; FFmpeg has to be installed on the host.
        CoreError::FfmpegNotFound => StatusCode::SERVICE_UNAVAILABLE,
        CoreError::NetworkError { .. } => StatusCode::BAD_GATEWAY,
        CoreError::RateLimited { .. } => StatusCode::TOO_MANY_REQUESTS,
        CoreError::NotImplemented { .. } => StatusCode::NOT_IMPLEMENTED,
        CoreError::Internal { .. } => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use spiritstream_core::errors::ValidationIssue;

    #[test]
    fn status_mapping_pins_user_facing_codes() {
        assert_eq!(
            status_for(&CoreError::ProfileNotFound { name: "x".into() }),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            status_for(&CoreError::PasswordIncorrect),
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            status_for(&CoreError::PortConflict {
                port: 1935,
                owner: "y".into()
            }),
            StatusCode::CONFLICT
        );
        assert_eq!(
            status_for(&CoreError::ValidationFailed {
                reasons: vec![ValidationIssue {
                    code: "x".into(),
                    message: "y".into(),
                    path: None
                }],
            }),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            status_for(&CoreError::Internal {
                context: "leak-this-server-side-only".into()
            }),
            StatusCode::INTERNAL_SERVER_ERROR
        );
    }

    /// New chat-error variants must serialize to the wire as
    /// `{ kind, details }` so clients can branch on `kind` without parsing
    /// the human-readable message. Pins the variant tag + payload shape
    /// against accidental renames or schema drift.
    #[test]
    fn chat_platform_not_connected_wire_shape() {
        let err = CoreError::ChatPlatformNotConnected {
            platform: "twitch".into(),
        };
        let body = serde_json::to_value(&err).expect("variant must serialize");
        assert_eq!(body["kind"], "chat_platform_not_connected");
        assert_eq!(body["details"]["platform"], "twitch");
        assert_eq!(status_for(&err), StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[test]
    fn chat_sending_disabled_wire_shape() {
        let err = CoreError::ChatSendingDisabled {
            platform: "youtube".into(),
        };
        let body = serde_json::to_value(&err).expect("variant must serialize");
        assert_eq!(body["kind"], "chat_sending_disabled");
        assert_eq!(body["details"]["platform"], "youtube");
        assert_eq!(status_for(&err), StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[test]
    fn chat_message_length_exceeded_wire_shape() {
        let err = CoreError::ChatMessageLengthExceeded {
            platform: "twitch".into(),
            limit: 500,
            actual: 612,
        };
        let body = serde_json::to_value(&err).expect("variant must serialize");
        assert_eq!(body["kind"], "chat_message_length_exceeded");
        assert_eq!(body["details"]["platform"], "twitch");
        assert_eq!(body["details"]["limit"], 500);
        assert_eq!(body["details"]["actual"], 612);
        assert_eq!(status_for(&err), StatusCode::BAD_REQUEST);
    }

    /// `NoActiveProfile` is the "client picked a profile-dependent
    /// action but no profile is active" variant. No payload — clients branch
    /// on `kind` alone. HTTP maps to 409 so it's distinguishable from
    /// `Unauthorized` (401) which means "re-authenticate".
    #[test]
    fn no_active_profile_wire_shape() {
        let err = CoreError::NoActiveProfile;
        let body = serde_json::to_value(&err).expect("variant must serialize");
        assert_eq!(body["kind"], "no_active_profile");
        assert_eq!(status_for(&err), StatusCode::CONFLICT);
    }

    #[test]
    fn internal_variant_is_redacted_in_body() {
        let resp = ApiError(CoreError::Internal {
            context: "do not leak".into(),
        })
        .into_response();
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
        // Body inspection happens via the integration test in http_surface.rs;
        // here we just confirm the status mapping + that conversion didn't panic.
    }
}
