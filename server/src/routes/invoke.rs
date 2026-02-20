use axum::{
    extract::{Json, Path, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};
use serde_json::Value;
use tower_cookies::Cookies;

use crate::app_state::AppState;
use crate::commands;
use crate::middleware::{
    bearer_token, is_valid_session, sanitize_error, verify_token, unauthorized_response,
    InvokeResponse, AUTH_COOKIE_NAME,
};

pub(crate) async fn invoke(
    Path(command): Path<String>,
    State(state): State<AppState>,
    headers: HeaderMap,
    cookies: Cookies,
    Json(payload): Json<Value>,
) -> impl IntoResponse {
    // Authentication check (cookie or bearer token)
    if let Some(expected) = state.auth_token.as_deref() {
        let authenticated = cookies.get(AUTH_COOKIE_NAME).is_some_and(|c| is_valid_session(c.value()))
            || bearer_token(&headers).is_some_and(|t| verify_token(expected, t));

        if !authenticated {
            return unauthorized_response().into_response();
        }
    }

    let result = invoke_command(&state, &command, payload).await;

    match result {
        Ok(data) => {
            let response = InvokeResponse {
                ok: true,
                data: Some(data),
                error: None,
            };
            (StatusCode::OK, Json(response)).into_response()
        }
        Err(error) => {
            let response = InvokeResponse {
                ok: false,
                data: None,
                error: Some(sanitize_error(&error)),
            };
            (StatusCode::BAD_REQUEST, Json(response)).into_response()
        }
    }
}

async fn invoke_command(
    state: &AppState,
    command: &str,
    payload: Value,
) -> Result<Value, String> {
    // Dispatch to domain command modules (chained pattern)
    if let Some(result) = commands::profile::handle(state, command, &payload).await { return result; }
    if let Some(result) = commands::streaming::handle(state, command, &payload).await { return result; }
    if let Some(result) = commands::settings::handle(state, command, &payload).await { return result; }
    if let Some(result) = commands::theme::handle(state, command, &payload).await { return result; }
    if let Some(result) = commands::device::handle(state, command, &payload).await { return result; }
    if let Some(result) = commands::source::handle(state, command, &payload).await { return result; }
    if let Some(result) = commands::scene::handle(state, command, &payload).await { return result; }
    if let Some(result) = commands::layer::handle(state, command, &payload).await { return result; }
    if let Some(result) = commands::mixer::handle(state, command, &payload).await { return result; }
    if let Some(result) = commands::capture::handle(state, command, &payload).await { return result; }
    Err(format!("Unknown command: {command}"))
}
