use axum::{
    body::Body,
    extract::{Json, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::path::PathBuf;

use crate::app_state::AppState;
use crate::services::validate_path_within_any;

#[derive(Debug, Deserialize)]
pub(crate) struct FileBrowseQuery {
    path: Option<String>,
}

#[derive(Debug, Serialize)]
struct FileEntry {
    name: String,
    #[serde(rename = "type")]
    entry_type: String, // "file" or "directory"
    size: Option<u64>,
}

#[derive(Debug, Serialize)]
struct BrowseResponse {
    path: String,
    entries: Vec<FileEntry>,
    parent: Option<String>,
}

/// GET /api/files/browse - List directory contents
/// Query params: path (optional, defaults to home directory)
pub(crate) async fn files_browse(
    State(state): State<AppState>,
    Query(params): Query<FileBrowseQuery>,
) -> impl IntoResponse {
    // Determine the directory to browse
    let browse_path = match params.path {
        Some(p) if !p.is_empty() => {
            // Expand ~ to home directory
            if p.starts_with("~/") {
                match &state.home_dir {
                    Some(home) => home.join(&p[2..]),
                    None => PathBuf::from(&p),
                }
            } else if p == "~" {
                match &state.home_dir {
                    Some(home) => home.clone(),
                    None => PathBuf::from(&p),
                }
            } else {
                PathBuf::from(&p)
            }
        }
        _ => match &state.home_dir {
            Some(home) => home.clone(),
            None => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({ "ok": false, "error": "Cannot determine home directory" })),
                )
                    .into_response();
            }
        },
    };

    // Security: Validate path is within allowed directories
    // Include common binary directories for finding executables like FFmpeg
    let system_bin_paths: Vec<PathBuf> = if cfg!(target_os = "windows") {
        vec![
            PathBuf::from("C:\\Program Files"),
            PathBuf::from("C:\\Program Files (x86)"),
        ]
    } else {
        vec![
            PathBuf::from("/opt"),        // Homebrew on Apple Silicon
            PathBuf::from("/usr/local"),  // Homebrew on Intel, common installs
            PathBuf::from("/usr/bin"),    // System binaries
        ]
    };

    let mut allowed_dirs: Vec<&std::path::Path> = vec![state.app_data_dir.as_path()];
    if let Some(ref home) = state.home_dir {
        allowed_dirs.push(home.as_path());
    }
    for sys_path in &system_bin_paths {
        if sys_path.exists() {
            allowed_dirs.push(sys_path.as_path());
        }
    }

    if validate_path_within_any(&browse_path, &allowed_dirs).is_err() {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({ "ok": false, "error": "Access to this directory is not allowed" })),
        )
            .into_response();
    }

    // Check if path exists and is a directory
    if !browse_path.exists() {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "ok": false, "error": "Directory not found" })),
        )
            .into_response();
    }

    if !browse_path.is_dir() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "ok": false, "error": "Path is not a directory" })),
        )
            .into_response();
    }

    // Read directory entries
    let entries = match std::fs::read_dir(&browse_path) {
        Ok(entries) => entries,
        Err(e) => {
            log::error!("Failed to read directory {browse_path:?}: {e}");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "ok": false, "error": "Failed to read directory" })),
            )
                .into_response();
        }
    };

    let mut file_entries: Vec<FileEntry> = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();

        // Skip hidden files/directories (starting with .)
        if name.starts_with('.') {
            continue;
        }

        let metadata = entry.metadata().ok();
        let entry_type = if metadata.as_ref().map(|m| m.is_dir()).unwrap_or(false) {
            "directory"
        } else {
            "file"
        };
        let size = if entry_type == "file" {
            metadata.as_ref().map(|m| m.len())
        } else {
            None
        };

        file_entries.push(FileEntry {
            name,
            entry_type: entry_type.to_string(),
            size,
        });
    }

    // Sort: directories first, then alphabetically
    file_entries.sort_by(|a, b| {
        match (&a.entry_type[..], &b.entry_type[..]) {
            ("directory", "file") => std::cmp::Ordering::Less,
            ("file", "directory") => std::cmp::Ordering::Greater,
            _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
        }
    });

    // Calculate parent directory (if not at root)
    let parent = browse_path.parent().and_then(|p| {
        let parent_path = p.to_path_buf();
        // Only include parent if it's within allowed directories
        if validate_path_within_any(&parent_path, &allowed_dirs).is_ok() {
            Some(parent_path.to_string_lossy().to_string())
        } else {
            None
        }
    });

    let response = BrowseResponse {
        path: browse_path.to_string_lossy().to_string(),
        entries: file_entries,
        parent,
    };

    Json(json!({ "ok": true, "data": response })).into_response()
}

/// GET /api/files/home - Get user home directory path
pub(crate) async fn files_home(State(state): State<AppState>) -> impl IntoResponse {
    match &state.home_dir {
        Some(home) => Json(json!({
            "ok": true,
            "data": { "path": home.to_string_lossy().to_string() }
        })),
        None => Json(json!({
            "ok": false,
            "error": "Cannot determine home directory"
        })),
    }
}

/// GET /api/system/default-paths - Get platform-specific default directories
/// Creates the directories if they don't exist
pub(crate) async fn system_default_paths(State(state): State<AppState>) -> impl IntoResponse {
    let home = match &state.home_dir {
        Some(h) => h.clone(),
        None => {
            return Json(json!({
                "ok": false,
                "error": "Cannot determine home directory"
            })).into_response();
        }
    };

    // Determine platform and appropriate video directory
    let (platform, videos_dir) = if cfg!(target_os = "macos") {
        ("macos", home.join("Movies"))
    } else if cfg!(target_os = "windows") {
        ("windows", home.join("Videos"))
    } else {
        ("linux", home.join("Videos"))
    };

    // Create subdirectories for recordings and replays
    let recordings_dir = videos_dir.join("SpiritStream");
    let replays_dir = videos_dir.join("SpiritStream").join("Replays");

    // Ensure directories exist (create if needed)
    if let Err(e) = std::fs::create_dir_all(&recordings_dir) {
        log::warn!("Failed to create recordings directory {:?}: {}", recordings_dir, e);
    }
    if let Err(e) = std::fs::create_dir_all(&replays_dir) {
        log::warn!("Failed to create replays directory {:?}: {}", replays_dir, e);
    }

    Json(json!({
        "ok": true,
        "data": {
            "platform": platform,
            "home": home.to_string_lossy().to_string(),
            "videos": videos_dir.to_string_lossy().to_string(),
            "recordings": recordings_dir.to_string_lossy().to_string(),
            "replays": replays_dir.to_string_lossy().to_string()
        }
    })).into_response()
}

#[derive(Debug, Deserialize)]
pub(crate) struct OpenPathRequest {
    path: String,
}

/// POST /api/files/open - Open path in native file manager
pub(crate) async fn files_open(
    State(state): State<AppState>,
    Json(payload): Json<OpenPathRequest>,
) -> impl IntoResponse {
    // Expand ~ to home directory
    let path = if payload.path.starts_with("~/") {
        match &state.home_dir {
            Some(home) => home.join(&payload.path[2..]),
            None => PathBuf::from(&payload.path),
        }
    } else if payload.path == "~" {
        match &state.home_dir {
            Some(home) => home.clone(),
            None => PathBuf::from(&payload.path),
        }
    } else {
        PathBuf::from(&payload.path)
    };

    // Security: Validate path is within allowed directories
    // Include common binary directories for consistency with file browser
    let system_bin_paths: Vec<PathBuf> = if cfg!(target_os = "windows") {
        vec![
            PathBuf::from("C:\\Program Files"),
            PathBuf::from("C:\\Program Files (x86)"),
        ]
    } else {
        vec![
            PathBuf::from("/opt"),
            PathBuf::from("/usr/local"),
            PathBuf::from("/usr/bin"),
        ]
    };

    let mut allowed_dirs: Vec<&std::path::Path> = vec![state.app_data_dir.as_path()];
    if let Some(ref home) = state.home_dir {
        allowed_dirs.push(home.as_path());
    }
    for sys_path in &system_bin_paths {
        if sys_path.exists() {
            allowed_dirs.push(sys_path.as_path());
        }
    }

    if validate_path_within_any(&path, &allowed_dirs).is_err() {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({ "ok": false, "error": "Access to this path is not allowed" })),
        );
    }

    // Check if path exists
    if !path.exists() {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "ok": false, "error": "Path not found" })),
        );
    }

    // Open in native file manager
    match opener::open(&path) {
        Ok(_) => (StatusCode::OK, Json(json!({ "ok": true }))),
        Err(e) => {
            log::error!("Failed to open path {path:?}: {e}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "ok": false, "error": "Failed to open path" })),
            )
        }
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct StaticFileQuery {
    path: String,
}

/// GET /api/static - Serve a static file (images, HTML) from the file system
/// Query params: path (required, the absolute file path to serve)
pub(crate) async fn static_file_handler(
    State(state): State<AppState>,
    Query(params): Query<StaticFileQuery>,
) -> impl IntoResponse {
    let file_path = PathBuf::from(&params.path);

    // Security: Validate file exists
    if !file_path.exists() {
        return (
            StatusCode::NOT_FOUND,
            [("Content-Type", "text/plain")],
            "File not found".to_string(),
        )
            .into_response();
    }

    // Security: Must be a file, not a directory
    if !file_path.is_file() {
        return (
            StatusCode::BAD_REQUEST,
            [("Content-Type", "text/plain")],
            "Path is not a file".to_string(),
        )
            .into_response();
    }

    // Security: Validate path is within allowed directories (home or app data)
    let mut allowed_dirs: Vec<&std::path::Path> = vec![state.app_data_dir.as_path()];
    if let Some(ref home) = state.home_dir {
        allowed_dirs.push(home.as_path());
    }
    // Also allow common media directories
    #[cfg(target_os = "macos")]
    {
        if let Some(ref home) = state.home_dir {
            let movies = home.join("Movies");
            let pictures = home.join("Pictures");
            let downloads = home.join("Downloads");
            let desktop = home.join("Desktop");
            let documents = home.join("Documents");
            // Check if file is under any of these
            let is_allowed = [&movies, &pictures, &downloads, &desktop, &documents]
                .iter()
                .any(|dir| file_path.starts_with(dir));
            if !is_allowed && !allowed_dirs.iter().any(|d| file_path.starts_with(d)) {
                return (
                    StatusCode::FORBIDDEN,
                    [("Content-Type", "text/plain")],
                    "Access denied".to_string(),
                )
                    .into_response();
            }
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        if !allowed_dirs.iter().any(|d| file_path.starts_with(d)) {
            return (
                StatusCode::FORBIDDEN,
                [("Content-Type", "text/plain")],
                "Access denied".to_string(),
            )
                .into_response();
        }
    }

    // Read the file
    let content = match std::fs::read(&file_path) {
        Ok(bytes) => bytes,
        Err(e) => {
            log::error!("Failed to read static file {:?}: {}", file_path, e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                [("Content-Type", "text/plain")],
                "Failed to read file".to_string(),
            )
                .into_response();
        }
    };

    // Determine content type based on file extension
    let content_type = match file_path.extension().and_then(|e| e.to_str()) {
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("svg") => "image/svg+xml",
        Some("bmp") => "image/bmp",
        Some("html") | Some("htm") => "text/html",
        Some("css") => "text/css",
        Some("js") => "application/javascript",
        _ => "application/octet-stream",
    };

    // Build response with appropriate headers
    // Cross-Origin-Resource-Policy: cross-origin is required because the frontend uses
    // Cross-Origin-Embedder-Policy: require-corp for SharedArrayBuffer support in audio workers
    let mut response = Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", content_type)
        .header("Cache-Control", "public, max-age=3600")
        .header("Cross-Origin-Resource-Policy", "cross-origin");

    // For HTML files, add a permissive CSP to allow external resources (fonts, stylesheets, etc.)
    // This is necessary because user-provided HTML content may include external dependencies
    if content_type == "text/html" {
        response = response.header(
            "Content-Security-Policy",
            "default-src * 'unsafe-inline' 'unsafe-eval' data: blob:; \
             script-src * 'unsafe-inline' 'unsafe-eval'; \
             style-src * 'unsafe-inline' https:; \
             font-src * data: https:; \
             img-src * data: blob: https: http:; \
             connect-src * https: http: ws: wss:"
        );
    }

    response.body(Body::from(content)).unwrap().into_response()
}
