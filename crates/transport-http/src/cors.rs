use std::env;
use std::sync::Arc;

use axum::http::{header, HeaderName, HeaderValue, Method};
use tower_http::cors::{AllowOrigin, CorsLayer};

/// Parse the `SPIRITSTREAM_CORS_ORIGINS` env var into a flat allow-list.
/// Shared by `build_cors_layer` and the CSRF middleware so the policy can
/// never drift between the browser-enforced CORS check and the
/// server-enforced CSRF fallback.
pub(crate) fn allowed_origins_from_env() -> Vec<String> {
    let cors_origins = env::var("SPIRITSTREAM_CORS_ORIGINS")
        .unwrap_or_else(|_| "http://localhost:*,http://127.0.0.1:*,tauri://localhost,http://tauri.localhost,https://tauri.localhost".to_string());
    cors_origins
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// True if `origin` matches any pattern in `allowed`. Patterns ending in
/// `:*` wildcard the port (e.g. `http://localhost:*` matches any port).
pub(crate) fn origin_matches(origin: &str, allowed: &[String]) -> bool {
    allowed.iter().any(|pattern| {
        if let Some(prefix) = pattern.strip_suffix(":*") {
            origin.starts_with(prefix) && origin[prefix.len()..].starts_with(':')
        } else {
            origin == pattern
        }
    })
}

pub(crate) fn build_cors_layer(allowed_origins: Arc<Vec<String>>) -> CorsLayer {
    CorsLayer::new()
        .allow_origin(AllowOrigin::predicate(move |origin: &HeaderValue, _| {
            let Ok(origin_str) = origin.to_str() else {
                return false;
            };
            origin_matches(origin_str, &allowed_origins)
        }))
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::PATCH,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([
            header::CONTENT_TYPE,
            header::COOKIE,
            header::AUTHORIZATION,
            // Destructive ops (clear-data, rotate-machine-key,
            // revoke-all-sessions) carry a one-shot confirm token in this
            // header. Without it on the CORS allow-list, the preflight
            // OPTIONS response omits the header and the browser blocks
            // the actual POST with a generic "Load failed". Plain-fetch
            // requests don't show this because they have no custom headers
            // to preflight on.
            HeaderName::from_static("x-confirm-token"),
        ])
        .allow_credentials(true)
}
