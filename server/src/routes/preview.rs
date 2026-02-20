use axum::{
    body::Body,
    extract::{Json, Path, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use futures_util::StreamExt;
use serde::Deserialize;
use serde_json::json;
use tokio_stream::wrappers::BroadcastStream;
use tower_cookies::Cookies;

use crate::app_state::AppState;
use crate::middleware::{bearer_token, is_valid_session, verify_token, AUTH_COOKIE_NAME};
use crate::services::PreviewParams;

#[derive(Debug, Deserialize)]
pub(crate) struct PreviewQuery {
    width: Option<u32>,
    height: Option<u32>,
    fps: Option<u32>,
    quality: Option<u32>,
    token: Option<String>,
}

impl From<PreviewQuery> for PreviewParams {
    fn from(query: PreviewQuery) -> Self {
        let defaults = PreviewParams::default();
        PreviewParams {
            width: query.width.unwrap_or(defaults.width).min(1280), // Cap at 720p width
            height: query.height.unwrap_or(defaults.height).min(720),
            fps: query.fps.unwrap_or(defaults.fps).min(30).max(5),
            quality: query.quality.unwrap_or(defaults.quality).clamp(1, 15),
        }
    }
}

/// GET /api/preview/source/:source_id - Stream MJPEG preview for a source
pub(crate) async fn source_preview_handler(
    State(state): State<AppState>,
    Path(source_id): Path<String>,
    Query(query): Query<PreviewQuery>,
    cookies: Cookies,
    headers: HeaderMap,
) -> impl IntoResponse {
    log::info!("Preview request for source: {}", source_id);

    // Authentication check — accept cookie, bearer header, or ?token= query param (for img tags)
    if let Some(expected) = state.auth_token.as_deref() {
        let authenticated = cookies
            .get(AUTH_COOKIE_NAME)
            .is_some_and(|c| is_valid_session(c.value()))
            || bearer_token(&headers).is_some_and(|t| verify_token(expected, t))
            || query.token.as_deref().is_some_and(|t| verify_token(expected, t));

        if !authenticated {
            log::warn!("Preview request unauthorized for source: {}", source_id);
            return (StatusCode::UNAUTHORIZED, "Unauthorized").into_response();
        }
    }

    let params: PreviewParams = query.into();
    log::debug!("Preview params: {}x{} @ {} fps, quality {}", params.width, params.height, params.fps, params.quality);

    // Find source from active profile
    let source = {
        // Get last profile from settings
        let settings = match state.settings_manager.load() {
            Ok(s) => s,
            Err(e) => {
                log::error!("Failed to load settings for preview: {}", e);
                return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to load settings").into_response();
            }
        };

        let profile_name = match settings.last_profile.as_ref() {
            Some(name) => name.clone(),
            None => {
                log::warn!("No active profile for preview request");
                return (StatusCode::BAD_REQUEST, "No active profile").into_response();
            }
        };

        log::debug!("Loading profile '{}' for preview", profile_name);

        // Load profile (without password for preview - encrypted profiles not supported in preview yet)
        match state.profile_manager.load(&profile_name, None).await {
            Ok(profile) => {
                log::debug!("Profile has {} sources", profile.sources.len());
                profile.sources.into_iter().find(|s| s.id() == source_id)
            }
            Err(e) => {
                log::error!("Failed to load profile for preview: {}", e);
                return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to load profile").into_response();
            }
        }
    };

    let source = match source {
        Some(s) => {
            log::info!("Found source for preview: {} (type: {:?})", s.name(), s.id());
            s
        }
        None => {
            log::warn!("Source {} not found in profile", source_id);
            return (StatusCode::NOT_FOUND, "Source not found").into_response();
        }
    };

    // Start preview stream
    let rx = match state.preview_handler.start_source_preview(&source, params) {
        Ok(rx) => rx,
        Err(e) => {
            log::error!("Failed to start preview: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to start preview: {}", e)).into_response();
        }
    };

    // Convert broadcast receiver to stream
    let stream = BroadcastStream::new(rx);

    // Build MJPEG streaming response using standard multipart format:
    // "--frame\r\nContent-Type: image/jpeg\r\nContent-Length: <len>\r\n\r\n" + jpeg_data + "\r\n"
    let body_stream = stream.filter_map(move |result| {
        async move {
            match result {
                Ok(frame_data) => {
                    // Build multipart MJPEG frame
                    let header = format!(
                        "--frame\r\nContent-Type: image/jpeg\r\nContent-Length: {}\r\n\r\n",
                        frame_data.len()
                    );
                    let mut response = Vec::with_capacity(frame_data.len() + header.len() + 2);
                    response.extend_from_slice(header.as_bytes());
                    response.extend_from_slice(&frame_data);
                    response.extend_from_slice(b"\r\n");
                    Some(Ok::<_, std::io::Error>(axum::body::Bytes::from(response)))
                }
                Err(_) => None, // Skip lagged frames
            }
        }
    });

    let body = Body::from_stream(body_stream);

    Response::builder()
        .status(StatusCode::OK)
        .header(
            header::CONTENT_TYPE,
            "multipart/x-mixed-replace; boundary=frame",
        )
        .header(header::CACHE_CONTROL, "no-cache, no-store, must-revalidate")
        .header(header::PRAGMA, "no-cache")
        .header(header::EXPIRES, "0")
        .body(body)
        .unwrap()
        .into_response()
}

/// POST /api/preview/source/:source_id/stop - Stop a source preview
pub(crate) async fn stop_source_preview_handler(
    State(state): State<AppState>,
    Path(source_id): Path<String>,
    cookies: Cookies,
    headers: HeaderMap,
) -> impl IntoResponse {
    // Authentication check
    if let Some(expected) = state.auth_token.as_deref() {
        let authenticated = cookies.get(AUTH_COOKIE_NAME).is_some_and(|c| is_valid_session(c.value()))
            || bearer_token(&headers).is_some_and(|t| verify_token(expected, t));

        if !authenticated {
            return (StatusCode::UNAUTHORIZED, Json(json!({ "ok": false, "error": "Unauthorized" })));
        }
    }

    state.preview_handler.stop_source_preview(&source_id);
    (StatusCode::OK, Json(json!({ "ok": true })))
}

/// POST /api/preview/stop-all - Stop all previews
pub(crate) async fn stop_all_previews_handler(
    State(state): State<AppState>,
    cookies: Cookies,
    headers: HeaderMap,
) -> impl IntoResponse {
    // Authentication check
    if let Some(expected) = state.auth_token.as_deref() {
        let authenticated = cookies.get(AUTH_COOKIE_NAME).is_some_and(|c| is_valid_session(c.value()))
            || bearer_token(&headers).is_some_and(|t| verify_token(expected, t));

        if !authenticated {
            return (StatusCode::UNAUTHORIZED, Json(json!({ "ok": false, "error": "Unauthorized" })));
        }
    }

    state.preview_handler.stop_all_previews();
    (StatusCode::OK, Json(json!({ "ok": true })))
}

/// GET /api/preview/source/:source_id/snapshot - Get a single JPEG snapshot
/// More reliable than MJPEG streaming for WebKit-based browsers (Safari, Tauri)
pub(crate) async fn source_snapshot_handler(
    State(state): State<AppState>,
    Path(source_id): Path<String>,
    Query(query): Query<PreviewQuery>,
    cookies: Cookies,
    headers: HeaderMap,
) -> impl IntoResponse {
    // Authentication check
    if let Some(expected) = state.auth_token.as_deref() {
        let authenticated = cookies.get(AUTH_COOKIE_NAME).is_some_and(|c| is_valid_session(c.value()))
            || bearer_token(&headers).is_some_and(|t| verify_token(expected, t));

        if !authenticated {
            return (StatusCode::UNAUTHORIZED, "Unauthorized").into_response();
        }
    }

    let params: PreviewParams = query.into();

    // Find source from active profile
    let source = {
        let settings = match state.settings_manager.load() {
            Ok(s) => s,
            Err(e) => {
                log::error!("Failed to load settings for snapshot: {}", e);
                return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to load settings").into_response();
            }
        };

        let profile_name = match settings.last_profile.as_ref() {
            Some(name) => name.clone(),
            None => {
                return (StatusCode::BAD_REQUEST, "No active profile").into_response();
            }
        };

        match state.profile_manager.load(&profile_name, None).await {
            Ok(profile) => profile.sources.into_iter().find(|s| s.id() == source_id),
            Err(e) => {
                log::error!("Failed to load profile for snapshot: {}", e);
                return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to load profile").into_response();
            }
        }
    };

    let source = match source {
        Some(s) => s,
        None => {
            return (StatusCode::NOT_FOUND, "Source not found").into_response();
        }
    };

    // Capture snapshot (async with timeout to prevent blocking)
    match state.preview_handler.capture_snapshot(&source, &params).await {
        Ok(jpeg_data) => {
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "image/jpeg")
                .header(header::CACHE_CONTROL, "no-cache, no-store, must-revalidate")
                .body(Body::from(jpeg_data))
                .unwrap()
                .into_response()
        }
        Err(e) => {
            log::warn!("Snapshot capture failed: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, format!("Snapshot failed: {}", e)).into_response()
        }
    }
}

/// GET /api/preview/scene/:profile/:scene_id - Stream MJPEG preview for a composed scene
pub(crate) async fn scene_preview_handler(
    State(state): State<AppState>,
    Path((profile_name, scene_id)): Path<(String, String)>,
    Query(query): Query<PreviewQuery>,
    cookies: Cookies,
    headers: HeaderMap,
) -> impl IntoResponse {
    log::info!("Scene preview request for profile:{} scene:{}", profile_name, scene_id);

    // Authentication check
    if let Some(expected) = state.auth_token.as_deref() {
        let authenticated = cookies.get(AUTH_COOKIE_NAME).is_some_and(|c| is_valid_session(c.value()))
            || bearer_token(&headers).is_some_and(|t| verify_token(expected, t));

        if !authenticated {
            log::warn!("Scene preview request unauthorized");
            return (StatusCode::UNAUTHORIZED, "Unauthorized").into_response();
        }
    }

    let params: PreviewParams = query.into();
    log::debug!("Scene preview params: {}x{} @ {} fps, quality {}", params.width, params.height, params.fps, params.quality);

    // Load profile and find scene
    let (scene, sources) = {
        match state.profile_manager.load(&profile_name, None).await {
            Ok(profile) => {
                let scene = profile.scenes.into_iter().find(|s| s.id == scene_id);
                let sources = profile.sources;
                match scene {
                    Some(s) => (s, sources),
                    None => {
                        log::warn!("Scene {} not found in profile {}", scene_id, profile_name);
                        return (StatusCode::NOT_FOUND, "Scene not found").into_response();
                    }
                }
            }
            Err(e) => {
                log::error!("Failed to load profile for scene preview: {}", e);
                return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to load profile").into_response();
            }
        }
    };

    // Start scene preview
    let rx = match state.preview_handler.start_scene_preview(&scene, &sources, params) {
        Ok(rx) => rx,
        Err(e) => {
            log::error!("Failed to start scene preview: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to start preview: {}", e)).into_response();
        }
    };

    // Convert broadcast receiver to stream
    let stream = BroadcastStream::new(rx);

    // Build MJPEG streaming response
    let body_stream = stream.filter_map(move |result| {
        async move {
            match result {
                Ok(frame_data) => {
                    let header = format!(
                        "--frame\r\nContent-Type: image/jpeg\r\nContent-Length: {}\r\n\r\n",
                        frame_data.len()
                    );
                    let mut response = Vec::with_capacity(frame_data.len() + header.len() + 2);
                    response.extend_from_slice(header.as_bytes());
                    response.extend_from_slice(&frame_data);
                    response.extend_from_slice(b"\r\n");
                    Some(Ok::<_, std::io::Error>(axum::body::Bytes::from(response)))
                }
                Err(_) => None,
            }
        }
    });

    let body = Body::from_stream(body_stream);

    Response::builder()
        .status(StatusCode::OK)
        .header(
            header::CONTENT_TYPE,
            "multipart/x-mixed-replace; boundary=frame",
        )
        .header(header::CACHE_CONTROL, "no-cache, no-store, must-revalidate")
        .header(header::PRAGMA, "no-cache")
        .header(header::EXPIRES, "0")
        .body(body)
        .unwrap()
        .into_response()
}

/// GET /api/preview/scene/:profile/:scene_id/snapshot - Get a single JPEG snapshot of composed scene
pub(crate) async fn scene_snapshot_handler(
    State(state): State<AppState>,
    Path((profile_name, scene_id)): Path<(String, String)>,
    Query(query): Query<PreviewQuery>,
    cookies: Cookies,
    headers: HeaderMap,
) -> impl IntoResponse {
    log::debug!("Scene snapshot request for profile:{} scene:{}", profile_name, scene_id);

    // Authentication check
    if let Some(expected) = state.auth_token.as_deref() {
        let authenticated = cookies.get(AUTH_COOKIE_NAME).is_some_and(|c| is_valid_session(c.value()))
            || bearer_token(&headers).is_some_and(|t| verify_token(expected, t));

        if !authenticated {
            return (StatusCode::UNAUTHORIZED, "Unauthorized").into_response();
        }
    }

    let params: PreviewParams = query.into();

    // Load profile and find scene
    let (scene, sources) = {
        match state.profile_manager.load(&profile_name, None).await {
            Ok(profile) => {
                let scene = profile.scenes.into_iter().find(|s| s.id == scene_id);
                let sources = profile.sources;
                match scene {
                    Some(s) => (s, sources),
                    None => {
                        return (StatusCode::NOT_FOUND, "Scene not found").into_response();
                    }
                }
            }
            Err(e) => {
                log::error!("Failed to load profile for scene snapshot: {}", e);
                return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to load profile").into_response();
            }
        }
    };

    // Capture scene snapshot
    match state.preview_handler.capture_scene_snapshot(&scene, &sources, &params).await {
        Ok(jpeg_data) => {
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "image/jpeg")
                .header(header::CACHE_CONTROL, "no-cache, no-store, must-revalidate")
                .body(Body::from(jpeg_data))
                .unwrap()
                .into_response()
        }
        Err(e) => {
            log::warn!("Scene snapshot capture failed: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, format!("Snapshot failed: {}", e)).into_response()
        }
    }
}

/// POST /api/preview/scene/stop - Stop the scene preview
pub(crate) async fn stop_scene_preview_handler(
    State(state): State<AppState>,
    cookies: Cookies,
    headers: HeaderMap,
) -> impl IntoResponse {
    // Authentication check
    if let Some(expected) = state.auth_token.as_deref() {
        let authenticated = cookies.get(AUTH_COOKIE_NAME).is_some_and(|c| is_valid_session(c.value()))
            || bearer_token(&headers).is_some_and(|t| verify_token(expected, t));

        if !authenticated {
            return (StatusCode::UNAUTHORIZED, Json(json!({ "ok": false, "error": "Unauthorized" })));
        }
    }

    state.preview_handler.stop_scene_preview();
    (StatusCode::OK, Json(json!({ "ok": true })))
}
