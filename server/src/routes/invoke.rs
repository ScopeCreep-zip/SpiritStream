use axum::{
    extract::{Json, Path, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};
use serde_json::Value;
use tower_cookies::Cookies;

use spiritstream_server::constants::AUTH_COOKIE_NAME;
use crate::handlers;
use crate::security::auth::{bearer_token, verify_token};
use crate::security::redaction::sanitize_error;
use crate::state::{AppState, InvokeResponse};

fn unauthorized_response() -> impl IntoResponse {
    let response = InvokeResponse {
        ok: false,
        data: None,
        error: Some("Unauthorized".to_string()),
    };
    (StatusCode::UNAUTHORIZED, Json(response))
}

pub(crate) async fn invoke(
    Path(command): Path<String>,
    State(state): State<AppState>,
    headers: HeaderMap,
    cookies: Cookies,
    Json(payload): Json<Value>,
) -> impl IntoResponse {
    // Authentication check (cookie or bearer token)
    if let Some(expected) = state.auth_token.as_deref() {
        let authenticated = cookies.get(AUTH_COOKIE_NAME).is_some()
            || bearer_token(&headers).is_some_and(|t| verify_token(expected, t));

        if !authenticated {
            return unauthorized_response().into_response();
        }
    }

    let result = handlers::dispatch(&state, &command, payload).await;

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
