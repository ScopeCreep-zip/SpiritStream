//! Builds the `spiritstream_core::ServiceRegistry` the CLI dispatches into.
//!
//! Resolution order for paths:
//!   1. CLI flag (`--data-dir`, `--themes-dir`).
//!   2. Environment variable (handled by clap's `env = …` attribute).
//!   3. Platform defaults.

use std::path::PathBuf;
use std::sync::Arc;

use spiritstream_core::services::build_secret_store;
use spiritstream_core::{NoopEventSink, ServiceRegistry, ServiceRegistryOptions};

use crate::error::CliError;

pub fn build(
    data_dir: Option<PathBuf>,
    themes_dir: Option<PathBuf>,
) -> Result<ServiceRegistry, CliError> {
    let data_dir = data_dir
        .or_else(default_data_dir)
        .ok_or_else(|| CliError::Io("could not resolve data directory".into()))?;
    let log_dir = data_dir.join("logs");
    let themes_dir = themes_dir
        .or_else(default_themes_dir)
        .unwrap_or_else(|| data_dir.join("themes"));

    let events: Arc<dyn spiritstream_core::services::EventSink> = Arc::new(NoopEventSink);
    let override_kind = std::env::var("SPIRITSTREAM_SECRET_STORE").ok();
    let secret_store = build_secret_store(&data_dir, override_kind.as_deref())?;

    ServiceRegistry::build(ServiceRegistryOptions {
        data_dir,
        themes_dir,
        log_dir,
        custom_ffmpeg_path: None,
        events,
        secret_store,
    })
    .map_err(CliError::from)
}

fn default_data_dir() -> Option<PathBuf> {
    dirs_next::data_local_dir().map(|d| d.join("spiritstream"))
}

fn default_themes_dir() -> Option<PathBuf> {
    // Walk up from the CLI binary's manifest dir looking for the workspace
    // `themes/` directory. Useful during development; production installs
    // ship themes inside the data dir via `ThemeManager::sync_project_themes`.
    let mut dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for _ in 0..6 {
        let candidate = dir.join("themes");
        if candidate.is_dir() {
            return Some(candidate);
        }
        if !dir.pop() {
            break;
        }
    }
    None
}
