use axum::http::{header, HeaderMap};
use subtle::ConstantTimeEq;

/// Constant-time API-token comparison.
///
/// `subtle::ConstantTimeEq` on raw `&[u8]` returns false **early** when
/// lengths differ — leaking the token length via timing. The leak is
/// small (the expected length is also bounded by env-var size), but a
/// motivated attacker could probe it with millions of requests. We
/// neutralise it by SHA-256-hashing both sides to a fixed 32-byte width
/// before the constant-time compare. This costs a single SHA-256 per
/// auth attempt — negligible compared to network RTT.
pub(crate) fn verify_token(expected: &str, provided: &str) -> bool {
    use sha2::{Digest, Sha256};
    let mut expected_h = Sha256::new();
    expected_h.update(expected.as_bytes());
    let mut provided_h = Sha256::new();
    provided_h.update(provided.as_bytes());
    expected_h
        .finalize()
        .as_slice()
        .ct_eq(provided_h.finalize().as_slice())
        .into()
}

/// Extract bearer token from the Authorization header.
pub(crate) fn bearer_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .map(str::trim)
        .filter(|value| !value.is_empty())
}
