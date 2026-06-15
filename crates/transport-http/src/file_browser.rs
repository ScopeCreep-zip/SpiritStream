use std::env;
use std::path::PathBuf;

use axum::{
    extract::{Query, State},
    Json,
};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use spiritstream_core::models::{FileBrowseResponse, FileEntry, FileHomeResponse};
use spiritstream_core::services::validate_path_within_any;

use crate::error::ApiError;
use crate::{AppState, FilesOpenResponse};

#[derive(Debug, Deserialize, IntoParams)]
pub(crate) struct FileBrowseQuery {
    pub(crate) path: Option<String>,
}

// Wire mirrors for the file-browser responses. `crates/core` must compile
// without utoipa, so the OpenAPI schema can't derive `ToSchema` on the core
// `FileEntry` / `FileBrowseResponse` / `FileHomeResponse` models directly —
// these transport-side mirrors carry the `ToSchema` derive and convert from
// the core models in the handler (same pattern as `ChatMessageWire`). The
// camelCase shape is byte-identical to the ts-rs export.

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FileEntryWire {
    pub name: String,
    #[serde(rename = "type")]
    pub entry_type: String,
    pub size: Option<u64>,
}

impl From<FileEntry> for FileEntryWire {
    fn from(e: FileEntry) -> Self {
        Self {
            name: e.name,
            entry_type: e.entry_type,
            size: e.size,
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FileBrowseResponseWire {
    pub path: String,
    pub entries: Vec<FileEntryWire>,
    pub parent: Option<String>,
}

impl From<FileBrowseResponse> for FileBrowseResponseWire {
    fn from(r: FileBrowseResponse) -> Self {
        Self {
            path: r.path,
            entries: r.entries.into_iter().map(Into::into).collect(),
            parent: r.parent,
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FileHomeResponseWire {
    pub path: String,
}

impl From<FileHomeResponse> for FileHomeResponseWire {
    fn from(r: FileHomeResponse) -> Self {
        Self { path: r.path }
    }
}

pub(crate) fn system_bin_paths() -> Vec<PathBuf> {
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

/// GET /api/v1/files/browse — list directory contents.
/// Query params: path (optional, defaults to home directory).
#[utoipa::path(
    get,
    path = "/files/browse",
    tag = "files",
    params(FileBrowseQuery),
    responses(
        (status = 200, description = "Directory listing (entries + parent).", body = FileBrowseResponseWire),
        (status = 400, description = "Path outside the allowed roots.", body = ApiErrorBody),
    ),
    security(("session_cookie" = []), ("bearer" = [])),
)]
pub(crate) async fn files_browse(
    State(state): State<AppState>,
    Query(params): Query<FileBrowseQuery>,
) -> Result<Json<FileBrowseResponseWire>, ApiError> {
    use spiritstream_core::errors::ValidationIssue;
    use spiritstream_core::CoreError;

    let browse_path = match params.path {
        Some(p) if !p.is_empty() => PathBuf::from(&p),
        _ => state.home_dir.clone().ok_or_else(|| {
            ApiError(CoreError::Internal {
                context: "cannot determine home directory".into(),
            })
        })?,
    };

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

    validate_path_within_any(&browse_path, &allowed_dirs).map_err(ApiError)?;

    if !browse_path.exists() {
        return Err(ApiError(CoreError::ValidationFailed {
            reasons: vec![ValidationIssue {
                code: "directory_not_found".into(),
                message: format!("directory not found: {}", browse_path.display()),
                path: None,
            }],
        }));
    }

    if !browse_path.is_dir() {
        return Err(ApiError(CoreError::ValidationFailed {
            reasons: vec![ValidationIssue {
                code: "not_a_directory".into(),
                message: format!("path is not a directory: {}", browse_path.display()),
                path: None,
            }],
        }));
    }

    let entries = std::fs::read_dir(&browse_path).map_err(|e| {
        log::error!("Failed to read directory {browse_path:?}: {e}");
        ApiError(CoreError::Internal {
            context: format!("read_dir({}): {e}", browse_path.display()),
        })
    })?;

    let mut file_entries: Vec<FileEntry> = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();

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

    file_entries.sort_by(|a, b| match (&a.entry_type[..], &b.entry_type[..]) {
        ("directory", "file") => std::cmp::Ordering::Less,
        ("file", "directory") => std::cmp::Ordering::Greater,
        _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
    });

    let parent = browse_path.parent().and_then(|p| {
        let parent_path = p.to_path_buf();
        if validate_path_within_any(&parent_path, &allowed_dirs).is_ok() {
            Some(parent_path.to_string_lossy().to_string())
        } else {
            None
        }
    });

    Ok(Json(
        FileBrowseResponse {
            path: browse_path.to_string_lossy().to_string(),
            entries: file_entries,
            parent,
        }
        .into(),
    ))
}

/// GET /api/v1/files/home — get user home directory path.
#[utoipa::path(
    get,
    path = "/files/home",
    tag = "files",
    responses((status = 200, description = "User home directory path.", body = FileHomeResponseWire)),
    security(("session_cookie" = []), ("bearer" = [])),
)]
pub(crate) async fn files_home(
    State(state): State<AppState>,
) -> Result<Json<FileHomeResponseWire>, ApiError> {
    use spiritstream_core::CoreError;

    let home = state.home_dir.as_ref().ok_or_else(|| {
        ApiError(CoreError::Internal {
            context: "cannot determine home directory".into(),
        })
    })?;

    Ok(Json(
        FileHomeResponse {
            path: home.to_string_lossy().to_string(),
        }
        .into(),
    ))
}

#[derive(Debug, Deserialize, ToSchema)]
pub(crate) struct OpenPathRequest {
    pub(crate) path: String,
}

/// POST /api/v1/files/open — open path in the native file manager.
#[utoipa::path(
    post,
    path = "/files/open",
    tag = "files",
    request_body = OpenPathRequest,
    responses(
        (status = 200, description = "Path handed to the OS opener.", body = FilesOpenResponse),
        (status = 400, description = "Path outside the allowed roots.", body = ApiErrorBody),
        (status = 404, description = "Path does not exist.", body = ApiErrorBody),
    ),
    security(("session_cookie" = []), ("bearer" = [])),
)]
pub(crate) async fn files_open(
    State(state): State<AppState>,
    Json(payload): Json<OpenPathRequest>,
) -> Result<Json<FilesOpenResponse>, ApiError> {
    let path = PathBuf::from(&payload.path);

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

    validate_path_within_any(&path, &allowed_dirs).map_err(ApiError)?;

    if !path.exists() {
        return Err(ApiError(spiritstream_core::CoreError::NotFound {
            resource: format!("path: {}", path.display()),
        }));
    }

    opener::open(&path).map_err(|e| {
        log::error!("Failed to open path {path:?}: {e}");
        ApiError(spiritstream_core::CoreError::Internal {
            context: format!("opener::open({}): {e}", path.display()),
        })
    })?;

    Ok(Json(FilesOpenResponse {}))
}
