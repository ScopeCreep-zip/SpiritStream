use axum::{
    extract::{Json, Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::env;
use std::path::PathBuf;

use crate::state::AppState;
use spiritstream_server::services::validate_path_within_any;

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

#[derive(Debug, Deserialize)]
pub(crate) struct OpenPathRequest {
    path: String,
}

fn system_bin_paths() -> Vec<PathBuf> {
    if cfg!(target_os = "windows") {
        let mut paths = Vec::new();

        if let Some(program_files) = env::var_os("ProgramFiles") {
            paths.push(PathBuf::from(program_files));
        }
        if let Some(program_files_x86) = env::var_os("ProgramFiles(x86)") {
            paths.push(PathBuf::from(program_files_x86));
        }
        if let Some(program_data) = env::var_os("ProgramData") {
            let base = PathBuf::from(program_data);
            paths.push(base.clone());
            paths.push(base.join("chocolatey"));
            paths.push(base.join("chocolatey\\bin"));
        }
        if let Some(choco_install) = env::var_os("ChocolateyInstall") {
            let base = PathBuf::from(choco_install);
            paths.push(base.clone());
            paths.push(base.join("bin"));
        }
        if let Some(system_drive) = env::var_os("SystemDrive") {
            let drive = PathBuf::from(format!("{}\\", system_drive.to_string_lossy()));
            paths.push(drive.join("ffmpeg"));
            paths.push(drive.join("ffmpeg\\bin"));
            paths.push(drive.join("Windows\\System32"));
        }

        if paths.is_empty() {
            paths.push(PathBuf::from("C:\\Program Files"));
            paths.push(PathBuf::from("C:\\Program Files (x86)"));
        }

        return paths;
    }

    vec![
        PathBuf::from("/opt"),
        PathBuf::from("/usr/local"),
        PathBuf::from("/usr/bin"),
        PathBuf::from("/snap/bin"),
    ]
}

/// GET /api/files/browse - List directory contents
/// Query params: path (optional, defaults to home directory)
pub(crate) async fn files_browse(
    State(state): State<AppState>,
    Query(params): Query<FileBrowseQuery>,
) -> impl IntoResponse {
    // Determine the directory to browse
    let browse_path = match params.path {
        Some(p) if !p.is_empty() => PathBuf::from(&p),
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
    let system_bin_paths = system_bin_paths();

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

/// POST /api/files/open - Open path in native file manager
pub(crate) async fn files_open(
    State(state): State<AppState>,
    Json(payload): Json<OpenPathRequest>,
) -> impl IntoResponse {
    let path = PathBuf::from(&payload.path);

    // Security: Validate path is within allowed directories
    // Include common binary directories for consistency with file browser
    let system_bin_paths = system_bin_paths();

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
