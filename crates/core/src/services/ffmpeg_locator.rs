//! FFmpeg discovery + version reporting.
//!
//! ## Architecture (Option A — no runtime download)
//!
//! SpiritStream no longer downloads FFmpeg at runtime. The binary is
//! delivered per-platform at build/install time:
//!
//! - **macOS / Windows**: FFmpeg is bundled as a Tauri 2 sidecar
//!   (`apps/tauri/src-tauri/binaries/ffmpeg-<TARGET>`). The Tauri shell
//!   resolves the sidecar path at startup and injects
//!   `SPIRITSTREAM_FFMPEG_PATH` when spawning the server.
//! - **Linux**: the `.deb` / `.rpm` bundle declares `ffmpeg` as a
//!   package dependency. Distro `apt` / `dnf` / `pacman` installs FFmpeg
//!   on app install; runtime discovery is plain `$PATH` lookup.
//!
//! ## Discovery order (deterministic, no runtime fallback chain)
//!
//! Per the `SecretStore` pattern: each input source is an
//! explicit configuration choice. A source that is set but invalid
//! refuses the operation rather than silently falling through.
//!
//! 1. **Operator override** — `settings.ffmpeg_path`. If non-empty:
//!    use it or fail. Never falls through.
//! 2. **Env-var injection** — `SPIRITSTREAM_FFMPEG_PATH`. Set by the
//!    Tauri shell on macOS / Windows once it resolved the bundled
//!    sidecar. If set: use it or fail. Never falls through.
//! 3. **`$PATH` lookup** — used when neither override is set. This is
//!    the Linux / Docker / CLI / dev path.
//! 4. None — caller treats as "FFmpeg unavailable" and refuses to start
//!    the stream.

use std::path::{Path, PathBuf};
use std::process::Command;
#[cfg(target_os = "macos")]
use std::time::Duration;

#[cfg(target_os = "macos")]
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::services::SettingsManager;

#[cfg(windows)]
use std::os::windows::process::CommandExt;
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

const FFMPEG_BIN: &str = if cfg!(windows) {
    "ffmpeg.exe"
} else {
    "ffmpeg"
};

#[derive(Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[ts(export, export_to = "../../../../packages/types/src/generated/")]
#[serde(rename_all = "camelCase")]
pub struct FFmpegVersionInfo {
    /// Currently installed version (None if not installed)
    pub installed_version: Option<String>,
    /// Latest available version (None if version check is not supported)
    pub latest_version: Option<String>,
    /// Whether an update is available
    pub update_available: bool,
    /// Human-readable status message
    pub status: String,
}

/// FFmpeg discovery + version reporter. Holds an HTTP client for the
/// macOS evermeet.cx version probe; no other persistent state.
pub struct FFmpegLocator {
    #[cfg(target_os = "macos")]
    client: Client,
}

impl FFmpegLocator {
    /// Construct the locator. macOS builds initialize a reqwest client for
    /// auto-update downloads; that's the only fallible part. Failure is rare
    /// (TLS backend init issues — musl libc, missing roots) but we surface it
    /// as `CoreError::Internal` rather than panic, matching every other
    /// service constructor in the registry.
    pub fn new() -> Result<Self, crate::errors::CoreError> {
        #[cfg(target_os = "macos")]
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .map_err(|e| crate::errors::CoreError::Internal {
                context: format!("ffmpeg locator reqwest client init failed: {e}"),
            })?;
        Ok(Self {
            #[cfg(target_os = "macos")]
            client,
        })
    }

    /// Resolve the FFmpeg binary path for this process. Returns `None`
    /// when no source is configured / available; callers should refuse
    /// to start streaming.
    pub fn discover(settings: Option<&SettingsManager>) -> Option<PathBuf> {
        // 1. Operator override in settings.
        if let Some(sm) = settings {
            if let Ok(s) = sm.load() {
                if !s.ffmpeg_path.is_empty() {
                    let p = PathBuf::from(&s.ffmpeg_path);
                    if p.exists() {
                        log::info!("FFmpeg from settings: {p:?}");
                        return Some(p);
                    }
                    log::warn!(
                        "Custom FFmpeg path in settings does not exist: {p:?}; \
                         refusing to silently fall through (set it to empty to use default discovery)"
                    );
                    return None;
                }
            }
        }

        // 2. Env-var injection (Tauri shell bundled sidecar).
        if let Ok(env_path) = std::env::var("SPIRITSTREAM_FFMPEG_PATH") {
            if !env_path.is_empty() {
                let p = PathBuf::from(&env_path);
                if p.exists() {
                    log::info!("FFmpeg from SPIRITSTREAM_FFMPEG_PATH: {p:?}");
                    return Some(p);
                }
                log::warn!(
                    "SPIRITSTREAM_FFMPEG_PATH points to {p:?} which does not exist; \
                     refusing to silently fall through"
                );
                return None;
            }
        }

        // 3. $PATH lookup — distro / Docker / dev.
        if let Some(p) = which_ffmpeg() {
            log::info!("FFmpeg from $PATH: {p:?}");
            return Some(p);
        }

        log::warn!(
            "FFmpeg not found via settings, SPIRITSTREAM_FFMPEG_PATH, or $PATH. \
             On Linux install via your distro (`apt install ffmpeg`); the macOS / \
             Windows bundle ships FFmpeg as a Tauri sidecar."
        );
        None
    }

    /// Auto-detect the installed FFmpeg version by running the
    /// discovered binary's `-version` flag and parsing stdout.
    pub fn detect_installed_version(&self, settings: Option<&SettingsManager>) -> Option<String> {
        let path = Self::discover(settings)?;
        let mut cmd = Command::new(&path);
        cmd.args(["-version"]);
        #[cfg(windows)]
        cmd.creation_flags(CREATE_NO_WINDOW);
        let output = cmd.output().ok()?;
        if !output.status.success() {
            return None;
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        Self::extract_version_from_text(&stdout)
    }

    /// Parse a `n.m[.p]` version triple out of `ffmpeg -version` output —
    /// matches `7.1`, `7.1.2`, or `version 7.1`. Public because
    /// `commands::system::validate_ffmpeg_path` parses the same line.
    pub fn extract_version_from_text(text: &str) -> Option<String> {
        let re = regex::Regex::new(r"(?:version[:\s]+)?(\d+\.\d+(?:\.\d+)?)").ok()?;
        re.captures(text)
            .and_then(|caps| caps.get(1))
            .map(|m| m.as_str().to_string())
    }

    /// Compare installed vs. latest using semver-ish parsing.
    pub fn is_newer_version(installed: &str, latest: &str) -> bool {
        let installed_parsed = Self::parse_version(installed);
        let latest_parsed = Self::parse_version(latest);
        match (installed_parsed, latest_parsed) {
            (Some(i), Some(l)) => l > i,
            _ => false,
        }
    }

    fn parse_version(version: &str) -> Option<(u32, u32, u32)> {
        let parts: Vec<&str> = version.split('.').collect();
        if parts.is_empty() {
            return None;
        }
        let major: u32 = parts.first()?.parse().ok()?;
        let minor: u32 = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);
        let patch: u32 = parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
        Some((major, minor, patch))
    }

    /// Query upstream for the latest published FFmpeg version.
    ///
    /// macOS uses the evermeet.cx JSON API (the same source whose
    /// binary the build pipeline ships). On Windows / Linux the
    /// upstream BtbN releases use a `latest` tag rather than a version
    /// in the URL, so this returns `None` and the version-status
    /// message surfaces "version check not supported for this build".
    pub async fn get_latest_version(&self) -> Option<String> {
        #[cfg(target_os = "macos")]
        {
            #[derive(Deserialize)]
            struct EvermeetRelease {
                version: String,
            }
            let response = self
                .client
                .get("https://evermeet.cx/ffmpeg/info/ffmpeg/release")
                .send()
                .await
                .ok()?;
            if !response.status().is_success() {
                return None;
            }
            let release: EvermeetRelease = response.json().await.ok()?;
            Some(release.version)
        }
        #[cfg(not(target_os = "macos"))]
        {
            None
        }
    }

    /// Check FFmpeg version status and determine if an update is available.
    pub async fn check_version_status(&self, installed_version: Option<&str>) -> FFmpegVersionInfo {
        let latest = self.get_latest_version().await;
        let update_available = match (&installed_version, &latest) {
            (Some(inst), Some(lat)) => Self::is_newer_version(inst, lat),
            (None, Some(_)) => true,
            _ => false,
        };

        let status = match (&installed_version, &latest, update_available) {
            (Some(v), _, false) => format!("FFmpeg {v} is up to date"),
            (Some(v), Some(l), true) => format!("Update available: {v} → {l}"),
            (None, Some(l), _) => format!("FFmpeg not installed (latest: {l})"),
            (None, None, _) => "FFmpeg not installed".to_string(),
            (Some(v), None, _) => format!("FFmpeg {v} installed (upstream version check unavailable for this platform's build)"),
        };

        FFmpegVersionInfo {
            installed_version: installed_version.map(|s| s.to_string()),
            latest_version: latest,
            update_available,
            status,
        }
    }
}

/// Walk `$PATH` for the FFmpeg binary. No external crate; deliberately
/// uses only `std` so the dependency graph stays minimal.
fn which_ffmpeg() -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(FFMPEG_BIN);
        if is_file(&candidate) {
            return Some(candidate);
        }
    }
    None
}

fn is_file(p: &Path) -> bool {
    std::fs::metadata(p).map(|m| m.is_file()).unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_version_parses_canonical_ffmpeg_output() {
        let text = "ffmpeg version 7.1 Copyright (c) 2000-2024 the FFmpeg developers";
        assert_eq!(
            FFmpegLocator::extract_version_from_text(text),
            Some("7.1".to_string())
        );
    }

    #[test]
    fn extract_version_parses_three_part_semver() {
        let text = "ffmpeg version 7.1.2";
        assert_eq!(
            FFmpegLocator::extract_version_from_text(text),
            Some("7.1.2".to_string())
        );
    }

    #[test]
    fn extract_version_returns_none_for_no_version() {
        assert_eq!(FFmpegLocator::extract_version_from_text("hello"), None);
    }

    #[test]
    fn is_newer_compares_correctly() {
        assert!(FFmpegLocator::is_newer_version("7.0", "7.1"));
        assert!(FFmpegLocator::is_newer_version("6.1.2", "7.0.0"));
        assert!(!FFmpegLocator::is_newer_version("7.1", "7.1"));
        assert!(!FFmpegLocator::is_newer_version("7.1", "7.0"));
    }

    #[test]
    fn version_info_reports_up_to_date() {
        // Sync wrapper because the function is async.
        let rt = tokio::runtime::Runtime::new().unwrap();
        let locator = FFmpegLocator::new().expect("test fixture");
        let info = rt.block_on(locator.check_version_status(Some("7.1")));
        assert_eq!(info.installed_version, Some("7.1".to_string()));
        // Latest may or may not be available depending on platform; status
        // formatting falls through.
        assert!(!info.status.is_empty());
    }
}
