use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path, Query, State,
    },
    http::StatusCode,
    response::IntoResponse,
};
use serde::Deserialize;
use tokio::sync::broadcast;
use tower_cookies::Cookies;

use crate::app_state::{AppState, ServerEvent};
use crate::middleware::{is_valid_session, verify_token, AUTH_COOKIE_NAME};

#[derive(Debug, Deserialize)]
pub(crate) struct AuthQuery {
    token: Option<String>,
}

pub(crate) async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    Query(query): Query<AuthQuery>,
    cookies: Cookies,
) -> impl IntoResponse {
    // Check authentication: no token required, valid cookie, or valid query param
    let authenticated = state.auth_token.is_none()
        || cookies.get(AUTH_COOKIE_NAME).is_some_and(|c| is_valid_session(c.value()))
        || query.token.as_deref().is_some_and(|token| {
            state.auth_token.as_deref().is_some_and(|expected| verify_token(expected, token))
        });

    if !authenticated {
        return (StatusCode::UNAUTHORIZED, "Unauthorized").into_response();
    }

    // Optimize WebSocket buffer sizes for real-time audio level data
    // - write_buffer_size: 0 = eagerly write each message (lower latency for small frequent updates)
    // - max_write_buffer_size: 64KB backpressure limit
    // - max_message_size: 64KB limit for incoming messages (we only receive small control messages)
    ws.write_buffer_size(0)
        .max_write_buffer_size(64 * 1024)
        .max_message_size(64 * 1024)
        .on_upgrade(move |socket| handle_socket(socket, state.event_bus.subscribe()))
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

/// WebSocket handler for preview JPEG frame streaming
/// GET /ws/preview/{source_id}
pub(crate) async fn ws_preview_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    Path(source_id): Path<String>,
    Query(query): Query<AuthQuery>,
    cookies: Cookies,
) -> impl IntoResponse {
    // Check authentication
    let authenticated = state.auth_token.is_none()
        || cookies.get(AUTH_COOKIE_NAME).is_some_and(|c| is_valid_session(c.value()))
        || query.token.as_deref().is_some_and(|token| {
            state.auth_token.as_deref().is_some_and(|expected| verify_token(expected, token))
        });

    if !authenticated {
        return (StatusCode::UNAUTHORIZED, "Unauthorized").into_response();
    }

    // Check if preview exists
    if !state.native_preview.has_preview(&source_id) {
        return (StatusCode::NOT_FOUND, format!("Preview not found: {}", source_id)).into_response();
    }

    // Subscribe to the preview
    let receiver = match state.native_preview.subscribe_preview(&source_id) {
        Some(rx) => rx,
        None => return (StatusCode::NOT_FOUND, "Preview no longer available").into_response(),
    };

    ws.on_upgrade(move |socket| handle_preview_socket(socket, receiver))
}

async fn handle_preview_socket(mut socket: WebSocket, mut receiver: broadcast::Receiver<bytes::Bytes>) {
    while let Ok(frame) = receiver.recv().await {
        // Send JPEG frame as binary
        if socket.send(Message::Binary(frame.to_vec())).await.is_err() {
            break;
        }
    }
}
