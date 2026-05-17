//! `spiritstream-cli files …` — file-browser primitives.
//!
//! Mirrors the HTTP `/api/v1/files/*` surface used by the frontend file
//! picker. The path-validation rules live in core
//! (`spiritstream_core::services::validate_path_within_any`); both CLI and
//! HTTP enforce the same allow-list (app data dir + user home).

use clap::Subcommand;
use serde::Serialize;
use spiritstream_core::ServiceRegistry;

use crate::error::CliError;
use crate::output::Output;

#[derive(Debug, Subcommand)]
pub enum FilesCmd {
    /// Browse a directory. Omitting `--path` returns the home directory.
    Browse {
        #[arg(long)]
        path: Option<std::path::PathBuf>,
    },
    /// Print the user's home directory path.
    Home,
}

#[derive(Serialize)]
struct FileEntry {
    name: String,
    #[serde(rename = "type")]
    kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    size: Option<u64>,
}

#[derive(Serialize)]
struct BrowseResponse {
    path: std::path::PathBuf,
    entries: Vec<FileEntry>,
    #[serde(skip_serializing_if = "Option::is_none")]
    parent: Option<std::path::PathBuf>,
}

pub async fn run(
    cmd: FilesCmd,
    registry: &ServiceRegistry,
    out: &mut Output,
) -> Result<(), CliError> {
    let home = dirs::home_dir();

    match cmd {
        FilesCmd::Home => {
            let path =
                home.ok_or_else(|| CliError::Argument("home directory not resolvable".into()))?;
            out.emit(&serde_json::json!({ "path": path }))?;
            Ok(())
        }
        FilesCmd::Browse { path } => {
            let target = match path {
                Some(p) => p,
                None => home.clone().ok_or_else(|| {
                    CliError::Argument("no --path and home dir not resolvable".into())
                })?,
            };

            // Enforce the same allow-list the HTTP layer uses: data dir + home.
            let mut allowed: Vec<&std::path::Path> = vec![registry.data_dir.as_path()];
            if let Some(ref h) = home {
                allowed.push(h.as_path());
            }
            spiritstream_core::services::validate_path_within_any(&target, &allowed)?;

            let read = std::fs::read_dir(&target)
                .map_err(|e| CliError::Io(format!("read_dir {}: {e}", target.display())))?;

            let mut entries = Vec::new();
            for r in read {
                let entry = r.map_err(|e| CliError::Io(format!("dir entry: {e}")))?;
                let meta = entry
                    .metadata()
                    .map_err(|e| CliError::Io(format!("metadata: {e}")))?;
                let kind = if meta.is_dir() { "directory" } else { "file" };
                let size = if meta.is_file() {
                    Some(meta.len())
                } else {
                    None
                };
                entries.push(FileEntry {
                    name: entry.file_name().to_string_lossy().to_string(),
                    kind,
                    size,
                });
            }
            entries.sort_by(|a, b| a.kind.cmp(b.kind).then_with(|| a.name.cmp(&b.name)));

            let parent = target.parent().map(|p| p.to_path_buf());
            out.emit(&BrowseResponse {
                path: target,
                entries,
                parent,
            })?;
            Ok(())
        }
    }
}
