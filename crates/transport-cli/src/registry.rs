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
    warn_if_desktop_install_elsewhere(&data_dir);
    // Documented env contract: SPIRITSTREAM_LOG_DIR overrides; defaults
    // to {DATA_DIR}/logs (previously the CLI silently ignored the env).
    let log_dir = std::env::var("SPIRITSTREAM_LOG_DIR")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| data_dir.join("logs"));
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

/// The desktop app stores its data under the Tauri bundle-identifier
/// directory, NOT the CLI's `spiritstream` default. Silently switching
/// would be a forbidden fallback; silently showing an empty install is
/// a trap. Loudly name both paths instead.
fn warn_if_desktop_install_elsewhere(data_dir: &std::path::Path) {
    let profiles = data_dir.join("profiles");
    let cli_has_profiles = std::fs::read_dir(&profiles)
        .map(|mut entries| entries.next().is_some())
        .unwrap_or(false);
    if cli_has_profiles {
        return;
    }
    let Some(desktop_dir) =
        dirs_next::data_local_dir().map(|d| d.join("com.spiritstream.desktop"))
    else {
        return;
    };
    let desktop_has_profiles = std::fs::read_dir(desktop_dir.join("profiles"))
        .map(|mut entries| entries.next().is_some())
        .unwrap_or(false);
    if desktop_has_profiles && desktop_dir != data_dir {
        eprintln!(
            "note: no profiles under {} but the desktop app has data at {} — \
             pass --data-dir {} to operate on the desktop install",
            data_dir.display(),
            desktop_dir.display(),
            desktop_dir.display(),
        );
    }
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
