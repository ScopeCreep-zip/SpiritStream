//! Static SPA serving for Docker / cloud / server-bundled deployments.
//!
//! Boot-coordination architecture: while `ServerReadiness::ready` is
//! false (services still initializing), `GET /` returns a static
//! loading page with `<meta http-equiv="refresh" content="1">`; the
//! browser auto-refreshes until services initialize, at which point the
//! same path serves the SPA. Tauri shells don't load via `/` — they
//! bundle the SPA (`frontendDist`) — so this only matters when
//! `SPIRITSTREAM_UI_ENABLED=1`.
//!
//! IMPORTANT: `mount_static_ui` must run BEFORE the router's
//! `.layer(...)` stack is applied. axum layers wrap only pre-existing
//! routes; mounting the SPA after the layers shipped the document and
//! every JS/CSS asset with zero security headers.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::http::{header, HeaderValue, StatusCode};
use axum::response::IntoResponse;
use axum::routing::get;
use axum::Router;
use tower_http::services::{ServeDir, ServeFile};

use crate::ServerReadiness;

/// Static HTML served at `GET /` while `ServerReadiness::ready == false`.
/// Uses `<meta http-equiv="refresh" content="1">` so the browser polls
/// without running any JavaScript; once services initialize, the next
/// refresh serves the real SPA. Same dark background as the Tauri shell
/// so users see no flash. No Content-Security-Policy issues (no inline
/// scripts, no external resources — pure HTML + minimal inline CSS).
pub(crate) const LOADING_PAGE_HTML: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1.0" />
  <meta http-equiv="refresh" content="1" />
  <title>SpiritStream — Starting…</title>
  <style>
    html, body {
      margin: 0;
      padding: 0;
      height: 100%;
      background-color: #0F0A14;
      color: #F4F2F7;
      font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
    }
    .center {
      position: absolute;
      inset: 0;
      display: flex;
      flex-direction: column;
      align-items: center;
      justify-content: center;
      gap: 1rem;
    }
    .spinner {
      width: 28px;
      height: 28px;
      border-radius: 50%;
      border: 3px solid rgba(167, 139, 250, 0.25);
      border-top-color: #A78BFA;
      animation: spin 0.8s linear infinite;
    }
    @keyframes spin { to { transform: rotate(360deg); } }
    .label { color: #B8AECA; font-size: 0.95rem; }
  </style>
</head>
<body>
  <div class="center">
    <div class="spinner" aria-hidden="true"></div>
    <div class="label">Starting SpiritStream…</div>
  </div>
</body>
</html>
"#;

/// CSP `style-src` source for the inline `<style>` in `LOADING_PAGE_HTML`.
///
/// The loading page is server-rendered static HTML carrying one inline
/// `<style>`. The response CSP forbids `'unsafe-inline'`, so that block is
/// allow-listed by the SHA-256 of its exact text content (the bytes between
/// `<style>` and `</style>`). Computed from the served HTML at startup so the
/// policy and the markup can never drift. Returns a `'sha256-…'` expression
/// (base64-standard encoded).
pub(crate) fn loading_page_style_csp_hash() -> String {
    use base64::Engine as _;
    use sha2::{Digest, Sha256};

    const OPEN: &str = "<style>";
    const CLOSE: &str = "</style>";
    let html = LOADING_PAGE_HTML;
    // Fail loud, not silent: if the markers ever leave LOADING_PAGE_HTML the
    // CSP would otherwise hash the wrong bytes and quietly break the loading
    // page's styling. A startup panic on this programmer-error invariant is the
    // correct CSP-safety behavior.
    let start = html
        .find(OPEN)
        .map(|i| i + OPEN.len())
        .expect("LOADING_PAGE_HTML must contain an inline <style> block");
    let end = html[start..]
        .find(CLOSE)
        .map(|i| start + i)
        .expect("LOADING_PAGE_HTML <style> block must be closed");
    let style = &html[start..end];
    let digest = Sha256::digest(style.as_bytes());
    format!(
        "'sha256-{}'",
        base64::engine::general_purpose::STANDARD.encode(digest)
    )
}

/// Mount `/`, `/index.html`, and the asset fallback when UI serving is
/// enabled and the bundle directory exists.
pub(crate) fn mount_static_ui(
    app: Router<()>,
    ui_enabled: bool,
    ui_path: &Path,
    readiness: Arc<ServerReadiness>,
) -> Router<()> {
    if !(ui_enabled && ui_path.exists()) {
        return app;
    }
    let index_path = ui_path.join("index.html");
    let readiness_for_root = readiness;
    let index_for_root: PathBuf = index_path.clone();
    let root_handler = move || {
        let readiness = readiness_for_root.clone();
        let index_path = index_for_root.clone();
        async move {
            use std::sync::atomic::Ordering;
            if readiness.ready.load(Ordering::Acquire) {
                match tokio::fs::read_to_string(&index_path).await {
                    Ok(html) => (
                        StatusCode::OK,
                        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
                        html,
                    )
                        .into_response(),
                    Err(err) => {
                        log::error!("failed to read SPA index.html: {err}");
                        (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            "index.html missing".to_string(),
                        )
                            .into_response()
                    }
                }
            } else {
                let mut resp = (
                    StatusCode::SERVICE_UNAVAILABLE,
                    [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
                    LOADING_PAGE_HTML,
                )
                    .into_response();
                resp.headers_mut()
                    .insert(header::RETRY_AFTER, HeaderValue::from_static("1"));
                resp
            }
        }
    };
    app.route("/", get(root_handler.clone()))
        .route("/index.html", get(root_handler))
        .fallback_service(
            ServeDir::new(ui_path).fallback(ServeFile::new(ui_path.join("index.html"))),
        )
}
