//! Small helpers extracted from `lib.rs` so the orchestrator stays
//! under the 600 LOC ceiling (K4):
//! * `parse_host` — fallback-on-error host string parser.
//! * `find_themes_dir_fallback` — walk-up theme-directory probe used
//!   when `SPIRITSTREAM_THEMES_DIR` isn't set or doesn't resolve.
//! * `ws_handler` + `handle_socket` + `AuthQuery` — the `/api/v1/events`
//!   one-way server-push WebSocket; gated on the same auth material
//!   `auth_middleware` accepts (cookie OR `?token=` query param so
//!   browsers that don't carry cookies in WS upgrades can still
//!   authenticate).

use std::net::{IpAddr, Ipv4Addr};

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Query, State,
    },
    http::StatusCode,
    response::IntoResponse,
};
use serde::Deserialize;
use tokio::sync::broadcast;
use tower_cookies::Cookies;

use crate::events::ServerEvent;
use crate::{AppState, AUTH_COOKIE_NAME};

pub(crate) fn parse_host(host: &str) -> IpAddr {
    host.parse().unwrap_or(IpAddr::V4(Ipv4Addr::LOCALHOST))
}

/// Find themes directory by searching common relative paths from CWD.
/// Used as fallback when SPIRITSTREAM_THEMES_DIR is not set or invalid.
pub(crate) fn find_themes_dir_fallback() -> Option<String> {
    let cwd = std::env::current_dir().ok()?;

    let candidates = [
        cwd.join("themes"),
        cwd.join("../themes"),
        cwd.join("../../themes"),
        cwd.join("../../../themes"),
    ];

    for candidate in candidates {
        if let Ok(canonical) = candidate.canonicalize() {
            if canonical.is_dir() {
                // Verify it has theme files
                if std::fs::read_dir(&canonical)
                    .map(|entries| {
                        entries.flatten().any(|e| {
                            e.path()
                                .extension()
                                .map(|ext| ext == "jsonc" || ext == "json")
                                .unwrap_or(false)
                        })
                    })
                    .unwrap_or(false)
                {
                    return Some(canonical.to_string_lossy().to_string());
                }
            }
        }
    }
    None
}

#[derive(Debug, Deserialize)]
pub(crate) struct AuthQuery {
    /// One-shot upgrade ticket from `POST /api/v1/events/ticket`.
    ticket: Option<String>,
}

pub(crate) async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    Query(query): Query<AuthQuery>,
    cookies: Cookies,
) -> impl IntoResponse {
    // Check authentication: no token configured, session cookie, or a
    // one-shot `?ticket=` from `POST /api/v1/events/ticket`. The cookie
    // path MUST verify against the session store — `revoke-all-sessions`
    // clears it, and anyone whose cookie still exists in their browser
    // would otherwise stay subscribed to the event bus after their
    // session was revoked (visible to a harassment-prone user as their
    // attacker continuing to see go-live / chat events). The previous
    // `?token=` bearer branch was unreachable in production (the auth
    // middleware rejected the upgrade first) AND undesirable: a
    // long-lived credential in a URL lands in proxy logs. Tickets are
    // single-use and expire in seconds.
    let cookie_valid = cookies
        .get(AUTH_COOKIE_NAME)
        .map(|c| state.sessions.is_valid(c.value()))
        .unwrap_or(false);
    let authenticated = state.auth_token.is_none()
        || cookie_valid
        || query
            .ticket
            .as_deref()
            .is_some_and(|ticket| state.event_tickets.consume(ticket));

    if !authenticated {
        return (StatusCode::UNAUTHORIZED, "Unauthorized").into_response();
    }

    ws.on_upgrade(move |socket| handle_socket(socket, state.event_bus.subscribe()))
}

async fn handle_socket(mut socket: WebSocket, mut receiver: broadcast::Receiver<ServerEvent>) {
    while let Ok(event) = receiver.recv().await {
        if let Ok(payload) = serde_json::to_string(&event) {
            if socket.send(Message::Text(payload)).await.is_err() {
                break;
            }
        }
    }
}
