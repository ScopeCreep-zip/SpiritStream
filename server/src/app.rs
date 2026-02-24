use axum::{
    http::{header, HeaderValue},
    middleware,
    routing::{get, post},
    Router,
};
use tower_cookies::CookieManagerLayer;
use tower_http::{
    services::{ServeDir, ServeFile},
    set_header::SetResponseHeaderLayer,
};

use crate::routes;
use crate::security;
use crate::state::AppState;

pub(crate) fn build_router(state: AppState, ui_enabled: bool, ui_dir: &str) -> Router {
    let is_localhost = state.is_localhost;

    // Build CORS layer
    let cors = security::cors::build_cors_layer();

    // Build CSP header dynamically based on binding address
    let csp_connect_src = if is_localhost {
        "connect-src 'self' ws://localhost:* wss://localhost:* http://localhost:* http://127.0.0.1:*"
    } else {
        // When serving UI remotely, 'self' covers the server's own origin
        // Also allow ws/wss for WebSocket on same origin
        "connect-src 'self' ws: wss:"
    };
    let csp_string = format!(
        "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; \
         {}; img-src 'self' data:; font-src 'self'",
        csp_connect_src
    );
    let csp_value = HeaderValue::from_str(&csp_string)
        .unwrap_or_else(|_| HeaderValue::from_static("default-src 'self'"));

    // Build router with security layers
    // Protected routes (require authentication)
    let protected_routes = Router::new()
        .route("/api/invoke/:command", post(routes::invoke::invoke))
        .route("/ws", get(routes::ws::ws_handler))
        // File browser endpoints for HTTP mode dialogs
        .route("/api/files/browse", get(routes::files::files_browse))
        .route("/api/files/home", get(routes::files::files_home))
        .route("/api/files/open", post(routes::files::files_open))
        .layer(middleware::from_fn_with_state(state.clone(), security::middleware::auth_middleware));

    // Public routes (no auth required)
    let public_routes = Router::new()
        .route("/health", get(routes::health::health))
        .route("/ready", get(routes::health::ready))
        .route("/auth/login", post(security::auth::auth_login))
        .route("/auth/logout", post(security::auth::auth_logout))
        .route("/auth/check", get(security::auth::auth_check));

    // Combine all routes
    let mut app = Router::new()
        .merge(public_routes)
        .merge(protected_routes)
        .with_state(state.clone())
        .layer(middleware::from_fn_with_state(state.clone(), security::middleware::rate_limit_middleware))
        .layer(CookieManagerLayer::new())
        .layer(cors)
        .layer(SetResponseHeaderLayer::if_not_present(
            header::CONTENT_SECURITY_POLICY,
            csp_value,
        ));

    // Optionally serve static UI files
    let ui_path = std::path::PathBuf::from(ui_dir);
    if ui_enabled && ui_path.exists() {
        app = app.fallback_service(
            ServeDir::new(&ui_path).fallback(ServeFile::new(ui_path.join("index.html"))),
        );
    }

    app
}
