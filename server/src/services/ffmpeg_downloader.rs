// FFmpeg Auto-Download Service
// Downloads and extracts FFmpeg for the current platform

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use futures_util::StreamExt;
use reqwest::Client;
use serde::Serialize;
#[cfg(target_os = "macos")]
use serde::Deserialize;
use crate::services::{emit_event, EventSink, SettingsManager};
use thiserror::Error;

/// Pinned FFmpeg version for shared libs (for reproducibility)
/// Update this deliberately after testing when bumping versions
/// See .claude/claudedocs/ffmpeg-libs-plan.md for version rationale
pub const FFMPEG_LIBS_VERSION: &str = "n8.0";
/// Short version suffix for BtbN release filenames (e.g., "8.0" for "ffmpeg-n8.0-...-8.0.zip")
const FFMPEG_LIBS_SHORT_VERSION: &str = "8.0";

/// Base URL for BtbN FFmpeg builds
const BTBN_RELEASE_BASE: &str = "https://github.com/BtbN/FFmpeg-Builds/releases/download/latest";

/// Progress information emitted during download
#[derive(Debug, Clone, Serialize)]
pub struct DownloadProgress {
    pub downloaded: u64,
    pub total: u64,
    pub percent: f64,
    pub phase: String,
    pub message: Option<String>,
}

/// Version information for FFmpeg
#[derive(Debug, Clone, Serialize)]
pub struct FFmpegVersionInfo {
    /// Currently installed version (None if not installed)
    pub installed_version: Option<String>,
    /// Latest available version for download
    pub latest_version: Option<String>,
    /// Whether an update is available
    pub update_available: bool,
    /// Human-readable status message
    pub status: String,
}

/// Errors that can occur during FFmpeg download
#[derive(Error, Debug)]
#[allow(dead_code)] // Some variants are only used on specific platforms
pub enum DownloadError {
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Unsupported platform: {0}")]
    UnsupportedPlatform(String),

    #[error("Download cancelled")]
    Cancelled,

    #[error("Archive extraction failed: {0}")]
    ExtractionFailed(String),

    #[error("FFmpeg binary not found in archive")]
    BinaryNotFound,

    #[error("Elevation denied by user")]
    ElevationDenied,

    #[error("Elevation failed: {0}")]
    ElevationFailed(String),

    #[error("Installation failed: {0}")]
    InstallationFailed(String),

    #[error("Downloaded FFmpeg build is missing required hardware encoders: {0}")]
    UnsupportedBuild(String),
}

/// Platform-specific shared libs download information
/// Used for FFmpeg libs integration (Phase 0 of FFmpeg libs migration)
struct SharedLibsDownload {
    url: String,
    archive_type: ArchiveType,
    /// Top-level directory name inside the archive (e.g., "ffmpeg-n7.1-latest-win64-gpl-shared-7.1")
    archive_root: String,
}

#[allow(dead_code)] // Variants are used on specific platforms only
enum ArchiveType {
    Zip,
    TarXz,
}

/// FFmpeg Downloader Service
pub struct FFmpegDownloader {
    client: Client,
    cancel_token: Arc<AtomicBool>,
}

impl FFmpegDownloader {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            cancel_token: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Request cancellation of the current download
    pub fn cancel(&self) {
        self.cancel_token.store(true, Ordering::SeqCst);
    }

    /// Reset the cancel token for a new download
    pub fn reset_cancel(&self) {
        self.cancel_token.store(false, Ordering::SeqCst);
    }

    /// Check if cancellation was requested
    fn is_cancelled(&self) -> bool {
        self.cancel_token.load(Ordering::SeqCst)
    }


    fn supports_version_check() -> bool {
        #[cfg(target_os = "macos")]
        {
            true
        }

        #[cfg(any(target_os = "windows", target_os = "linux"))]
        {
            false
        }

        #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
        {
            false
        }
    }

    /// Get platform-specific shared libs download information
    /// These are pinned builds for FFmpeg libs integration (linking with ffmpeg-sys-next)
    fn get_shared_libs_download() -> Result<SharedLibsDownload, DownloadError> {
        #[cfg(target_os = "windows")]
        {
            // Windows: BtbN shared libs build with hardware encoders
            // Package contains: bin/ (DLLs), lib/ (import libs), include/ (headers)
            // Note: BtbN filenames include short version suffix (e.g., "-8.0.zip")
            Ok(SharedLibsDownload {
                url: format!(
                    "{}/ffmpeg-{}-latest-win64-gpl-shared-{}.zip",
                    BTBN_RELEASE_BASE, FFMPEG_LIBS_VERSION, FFMPEG_LIBS_SHORT_VERSION
                ),
                archive_type: ArchiveType::Zip,
                archive_root: format!("ffmpeg-{}-latest-win64-gpl-shared-{}", FFMPEG_LIBS_VERSION, FFMPEG_LIBS_SHORT_VERSION),
            })
        }

        #[cfg(target_os = "linux")]
        {
            // Linux: BtbN shared libs build
            // Note: BtbN filenames include short version suffix
            Ok(SharedLibsDownload {
                url: format!(
                    "{}/ffmpeg-{}-latest-linux64-gpl-shared-{}.tar.xz",
                    BTBN_RELEASE_BASE, FFMPEG_LIBS_VERSION, FFMPEG_LIBS_SHORT_VERSION
                ),
                archive_type: ArchiveType::TarXz,
                archive_root: format!("ffmpeg-{}-latest-linux64-gpl-shared-{}", FFMPEG_LIBS_VERSION, FFMPEG_LIBS_SHORT_VERSION),
            })
        }

        #[cfg(target_os = "macos")]
        {
            // macOS: evermeet.cx doesn't provide shared libs
            // Users need Homebrew FFmpeg or we need a different source
            // For now, return an error - macOS support for libs requires different approach
            Err(DownloadError::UnsupportedPlatform(
                "Shared libs download not yet supported on macOS. Use Homebrew: brew install ffmpeg".to_string()
            ))
        }

        #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
        {
            Err(DownloadError::UnsupportedPlatform(format!(
                "Shared libs not supported on {} {}",
                std::env::consts::OS,
                std::env::consts::ARCH
            )))
        }
    }

    /// Get the directory for FFmpeg shared libs (for linking/embedding)
    /// This is in app data directory - no elevation needed
    /// Structure: <app_data>/ffmpeg-libs/
    ///   ├── bin/     (DLLs on Windows, .so on Linux)
    ///   ├── lib/     (import libraries)
    ///   ├── include/ (headers for ffmpeg-sys-next)
    ///   └── version  (version marker file)
    pub fn get_ffmpeg_libs_dir() -> PathBuf {
        #[cfg(target_os = "windows")]
        {
            // Use %LOCALAPPDATA%/SpiritStream/ffmpeg-libs
            if let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") {
                return PathBuf::from(local_app_data)
                    .join("SpiritStream")
                    .join("ffmpeg-libs");
            }
            // Fallback to temp
            std::env::temp_dir().join("spiritstream-ffmpeg-libs")
        }

        #[cfg(target_os = "macos")]
        {
            // Use ~/Library/Application Support/SpiritStream/ffmpeg-libs
            if let Some(home) = std::env::var_os("HOME") {
                return PathBuf::from(home)
                    .join("Library")
                    .join("Application Support")
                    .join("SpiritStream")
                    .join("ffmpeg-libs");
            }
            std::env::temp_dir().join("spiritstream-ffmpeg-libs")
        }

        #[cfg(target_os = "linux")]
        {
            // Use ~/.local/share/spiritstream/ffmpeg-libs
            if let Some(home) = std::env::var_os("HOME") {
                return PathBuf::from(home)
                    .join(".local")
                    .join("share")
                    .join("spiritstream")
                    .join("ffmpeg-libs");
            }
            std::env::temp_dir().join("spiritstream-ffmpeg-libs")
        }

        #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
        {
            std::env::temp_dir().join("spiritstream-ffmpeg-libs")
        }
    }

    /// Check if FFmpeg shared libs are installed
    pub fn are_shared_libs_installed() -> bool {
        let libs_dir = Self::get_ffmpeg_libs_dir();
        let version_file = libs_dir.join("version");

        if !version_file.exists() {
            return false;
        }

        // Verify essential directories exist
        let has_bin = libs_dir.join("bin").exists();
        let has_lib = libs_dir.join("lib").exists();
        let has_include = libs_dir.join("include").exists();

        has_bin && has_lib && has_include
    }

    /// Get the installed shared libs version, if any
    pub fn get_installed_libs_version() -> Option<String> {
        let version_file = Self::get_ffmpeg_libs_dir().join("version");
        std::fs::read_to_string(version_file).ok()
            .map(|s| s.trim().to_string())
    }

    /// Download and extract FFmpeg shared libs.
    pub async fn download(&self, event_sink: &dyn EventSink) -> Result<PathBuf, DownloadError> {
        self.download_shared_libs(event_sink).await
    }

    /// Download and extract FFmpeg shared libs for linking/embedding
    /// Unlike the CLI binary, these go to app data directory (no elevation needed)
    /// This is Phase 0 of the FFmpeg libs migration - required for ffmpeg-sys-next linking
    pub async fn download_shared_libs(&self, event_sink: &dyn EventSink) -> Result<PathBuf, DownloadError> {
        self.reset_cancel();

        let libs_dir = Self::get_ffmpeg_libs_dir();

        // Check if already installed with correct version
        if Self::are_shared_libs_installed() {
            if let Some(installed_version) = Self::get_installed_libs_version() {
                if installed_version == FFMPEG_LIBS_VERSION {
                    log::info!("FFmpeg shared libs already installed at: {libs_dir:?} (version {installed_version})");
                    self.emit_progress(
                        event_sink, 100, 100, 100.0, "complete",
                        Some("FFmpeg shared libs are already installed")
                    );
                    return Ok(libs_dir);
                }
                log::info!("FFmpeg shared libs version mismatch: {installed_version} != {FFMPEG_LIBS_VERSION}, re-downloading");
            }
        }

        // Get platform-specific download info
        let platform = Self::get_shared_libs_download()?;

        // Create temp directory for download
        let temp_dir = tempfile::tempdir()?;
        let archive_path = temp_dir.path().join("ffmpeg_shared_libs_archive");

        // Emit starting phase
        self.emit_progress(event_sink, 0, 0, 0.0, "starting", Some("Downloading FFmpeg shared libraries..."));

        // Download the file
        self.download_file(&platform.url, &archive_path.clone(), event_sink).await?;

        if self.is_cancelled() {
            return Err(DownloadError::Cancelled);
        }

        // Extract the archive
        self.emit_progress(event_sink, 0, 0, 0.0, "extracting", Some("Extracting FFmpeg shared libraries..."));

        // Clean existing libs dir if present
        if libs_dir.exists() {
            log::info!("Removing existing libs directory: {libs_dir:?}");
            std::fs::remove_dir_all(&libs_dir)?;
        }
        std::fs::create_dir_all(&libs_dir)?;

        // Extract based on archive type
        match platform.archive_type {
            ArchiveType::Zip => {
                self.extract_shared_libs_zip(&archive_path, &libs_dir, &platform.archive_root)?;
            }
            ArchiveType::TarXz => {
                self.extract_shared_libs_tar_xz(&archive_path, &libs_dir, &platform.archive_root)?;
            }
        }

        // Verify essential directories exist
        let has_bin = libs_dir.join("bin").exists();
        let has_lib = libs_dir.join("lib").exists();
        let has_include = libs_dir.join("include").exists();

        if !has_bin || !has_lib || !has_include {
            return Err(DownloadError::ExtractionFailed(format!(
                "Missing directories after extraction: bin={has_bin}, lib={has_lib}, include={has_include}"
            )));
        }

        // Write version marker
        let version_file = libs_dir.join("version");
        std::fs::write(&version_file, FFMPEG_LIBS_VERSION)?;
        log::info!("Wrote version marker: {version_file:?} = {FFMPEG_LIBS_VERSION}");

        self.emit_progress(
            event_sink, 100, 100, 100.0, "complete",
            Some(&format!("FFmpeg shared libs {} installed successfully", FFMPEG_LIBS_VERSION))
        );

        log::info!("FFmpeg shared libs installed to: {libs_dir:?}");
        Ok(libs_dir)
    }

    /// Download file with progress reporting
    async fn download_file(
        &self,
        url: &str,
        dest: &PathBuf,
        event_sink: &dyn EventSink,
    ) -> Result<(), DownloadError> {
        log::info!("Downloading FFmpeg from: {url}");

        let response = self.client
            .get(url)
            .send()
            .await?
            .error_for_status()?;

        let total_size = response.content_length().unwrap_or(0);
        let mut downloaded: u64 = 0;

        let mut file = std::fs::File::create(dest)?;
        let mut stream = response.bytes_stream();

        while let Some(chunk) = stream.next().await {
            if self.is_cancelled() {
                return Err(DownloadError::Cancelled);
            }

            let chunk = chunk?;
            file.write_all(&chunk)?;

            downloaded += chunk.len() as u64;
            let percent = if total_size > 0 {
                (downloaded as f64 / total_size as f64) * 100.0
            } else {
                0.0
            };

            self.emit_progress(event_sink, downloaded, total_size, percent, "downloading", None);
        }

        file.flush()?;
        log::info!("Download complete: {downloaded} bytes");
        Ok(())
    }

    /// Extract shared libs ZIP archive (Windows)
    /// Extracts bin/, lib/, include/ directories to destination
    fn extract_shared_libs_zip(
        &self,
        archive_path: &Path,
        dest_dir: &Path,
        archive_root: &str,
    ) -> Result<(), DownloadError> {
        let file = std::fs::File::open(archive_path)?;
        let mut archive = zip::ZipArchive::new(file)
            .map_err(|e| DownloadError::ExtractionFailed(e.to_string()))?;

        log::info!("Extracting shared libs from ZIP to: {dest_dir:?}");

        // Create destination directories
        std::fs::create_dir_all(dest_dir.join("bin"))?;
        std::fs::create_dir_all(dest_dir.join("lib"))?;
        std::fs::create_dir_all(dest_dir.join("include"))?;

        let mut extracted_count = 0;

        for i in 0..archive.len() {
            let mut file = archive.by_index(i)
                .map_err(|e| DownloadError::ExtractionFailed(e.to_string()))?;

            let file_path = file.name();

            // Skip directories
            if file_path.ends_with('/') {
                continue;
            }

            // Strip the archive root prefix to get relative path
            let relative_path = file_path
                .strip_prefix(archive_root)
                .unwrap_or(file_path)
                .trim_start_matches('/');

            // Only extract files in bin/, lib/, include/
            let should_extract = relative_path.starts_with("bin/")
                || relative_path.starts_with("lib/")
                || relative_path.starts_with("include/");

            if should_extract {
                let dest_path = dest_dir.join(relative_path);

                // Create parent directories if needed
                if let Some(parent) = dest_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }

                let mut outfile = std::fs::File::create(&dest_path)?;
                std::io::copy(&mut file, &mut outfile)?;
                extracted_count += 1;
            }
        }

        log::info!("Extracted {extracted_count} files from shared libs archive");
        Ok(())
    }

    /// Extract shared libs TAR.XZ archive (Linux)
    fn extract_shared_libs_tar_xz(
        &self,
        archive_path: &Path,
        dest_dir: &Path,
        archive_root: &str,
    ) -> Result<(), DownloadError> {
        let file = std::fs::File::open(archive_path)?;
        let decompressor = xz2::read::XzDecoder::new(file);
        let mut archive = tar::Archive::new(decompressor);

        log::info!("Extracting shared libs from TAR.XZ to: {dest_dir:?}");

        // Create destination directories
        std::fs::create_dir_all(dest_dir.join("bin"))?;
        std::fs::create_dir_all(dest_dir.join("lib"))?;
        std::fs::create_dir_all(dest_dir.join("include"))?;

        let mut extracted_count = 0;

        for entry in archive.entries()? {
            let mut entry = entry?;
            let path = entry.path()?;
            let path_str = path.to_string_lossy();

            // Strip the archive root prefix
            let relative_path = path_str
                .strip_prefix(archive_root)
                .unwrap_or(&path_str)
                .trim_start_matches('/');

            // Only extract files in bin/, lib/, include/
            let should_extract = relative_path.starts_with("bin/")
                || relative_path.starts_with("lib/")
                || relative_path.starts_with("include/");

            if should_extract && entry.header().entry_type().is_file() {
                let dest_path = dest_dir.join(relative_path);

                // Create parent directories if needed
                if let Some(parent) = dest_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }

                let mut outfile = std::fs::File::create(&dest_path)?;
                std::io::copy(&mut entry, &mut outfile)?;
                extracted_count += 1;
            }
        }

        log::info!("Extracted {extracted_count} files from shared libs archive");
        Ok(())
    }

    /// Emit progress event to frontend
    fn emit_progress(
        &self,
        event_sink: &dyn EventSink,
        downloaded: u64,
        total: u64,
        percent: f64,
        phase: &str,
        message: Option<&str>,
    ) {
        let progress = DownloadProgress {
            downloaded,
            total,
            percent,
            phase: phase.to_string(),
            message: message.map(|s| s.to_string()),
        };

        emit_event(event_sink, "ffmpeg_download_progress", &progress);
    }

    /// Get the path where FFmpeg is installed
    /// Checks in order: 1) Custom path from settings, 2) System install location
    pub fn get_ffmpeg_path(settings_manager: Option<&SettingsManager>) -> Option<PathBuf> {
        let _ = settings_manager;
        if Self::are_shared_libs_installed() {
            Some(Self::get_ffmpeg_libs_dir())
        } else {
            None
        }
    }

    /// Delete FFmpeg shared libs from local app data.
    pub fn delete_ffmpeg(settings_manager: Option<&SettingsManager>) -> Result<(), DownloadError> {
        let _ = settings_manager;
        let libs_dir = Self::get_ffmpeg_libs_dir();

        if !libs_dir.exists() {
            log::info!("FFmpeg shared libs not found, nothing to delete");
            return Ok(());
        }

        log::info!("Deleting FFmpeg shared libs from: {libs_dir:?}");
        std::fs::remove_dir_all(&libs_dir)
            .map_err(|e| DownloadError::InstallationFailed(format!("Failed to delete FFmpeg shared libs: {e}")))?;
        log::info!("FFmpeg shared libs deleted successfully");
        Ok(())
    }

    /// Get the latest available FFmpeg version from the download source
    pub async fn get_latest_version(&self) -> Result<String, DownloadError> {
        #[cfg(target_os = "macos")]
        {
            // evermeet.cx provides a JSON API for version info
            #[derive(Deserialize)]
            struct EvermeetRelease {
                version: String,
            }

            let response = self.client
                .get("https://evermeet.cx/ffmpeg/info/ffmpeg/release")
                .send()
                .await?
                .error_for_status()?;

            let release: EvermeetRelease = response.json().await?;
            Ok(release.version)
        }

        #[cfg(target_os = "windows")]
        {
            Err(DownloadError::ExtractionFailed(
                "Version check not supported for hardware-enabled Windows builds".to_string(),
            ))
        }

        #[cfg(target_os = "linux")]
        {
            Err(DownloadError::ExtractionFailed(
                "Version check not supported for hardware-enabled Linux builds".to_string(),
            ))
        }

        #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
        {
            Err(DownloadError::UnsupportedPlatform("Version check not supported".to_string()))
        }
    }

    /// Extract version number from a URL string
    #[allow(dead_code)] // Used only on specific platforms
    fn extract_version_from_url(url: &str) -> Option<String> {
        // Look for patterns like "ffmpeg-7.1" or "ffmpeg-7.1.2"
        let re = regex::Regex::new(r"ffmpeg[_-](\d+\.\d+(?:\.\d+)?)").ok()?;
        re.captures(url)
            .and_then(|caps| caps.get(1))
            .map(|m| m.as_str().to_string())
    }

    /// Extract version number from text content
    #[allow(dead_code)] // Used only on specific platforms
    fn extract_version_from_text(text: &str) -> Option<String> {
        // Look for version patterns like "7.1" or "7.1.2" or "version 7.1"
        let re = regex::Regex::new(r"(?:version[:\s]+)?(\d+\.\d+(?:\.\d+)?)").ok()?;
        re.captures(text)
            .and_then(|caps| caps.get(1))
            .map(|m| m.as_str().to_string())
    }

    /// Parse version string to comparable tuple
    fn parse_version(version: &str) -> Option<(u32, u32, u32)> {
        let parts: Vec<&str> = version.split('.').collect();
        if parts.is_empty() {
            return None;
        }

        let major = parts.first()?.parse().ok()?;
        let minor = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);
        let patch = parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);

        Some((major, minor, patch))
    }

    /// Compare two version strings
    /// Returns true if `latest` is newer than `installed`
    pub fn is_newer_version(installed: &str, latest: &str) -> bool {
        let installed_parsed = Self::parse_version(installed);
        let latest_parsed = Self::parse_version(latest);

        match (installed_parsed, latest_parsed) {
            (Some(inst), Some(lat)) => lat > inst,
            _ => false,
        }
    }

    /// Check FFmpeg version status and determine if update is available
    pub async fn check_version_status(&self, installed_version: Option<&str>) -> FFmpegVersionInfo {
        let supports_version_check = Self::supports_version_check();
        let latest = if supports_version_check {
            self.get_latest_version().await.ok()
        } else {
            None
        };

        let update_available = match (&installed_version, &latest) {
            (Some(inst), Some(lat)) => Self::is_newer_version(inst, lat),
            (None, Some(_)) => true, // Not installed, latest available
            _ => false,
        };

        let status = if !supports_version_check {
            match installed_version {
                Some(_) => "Version check not supported for this build".to_string(),
                None => "FFmpeg not installed".to_string(),
            }
        } else {
            match (&installed_version, &latest, update_available) {
                (Some(v), _, false) => format!("FFmpeg {v} is up to date"),
                (Some(v), Some(l), true) => format!("Update available: {v} -> {l}"),
                (None, Some(l), _) => format!("FFmpeg not installed (latest: {l})"),
                (None, None, _) => "FFmpeg not installed".to_string(),
                (Some(v), None, _) => format!("FFmpeg {v} installed (unable to check for updates)"),
            }
        };

        FFmpegVersionInfo {
            installed_version: installed_version.map(|s| s.to_string()),
            latest_version: latest,
            update_available,
            status,
        }
    }
}

impl Default for FFmpegDownloader {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_shared_libs_download() {
        let result = FFmpegDownloader::get_shared_libs_download();

        #[cfg(any(target_os = "windows", target_os = "linux"))]
        assert!(result.is_ok(), "shared libs download should be configured");

        #[cfg(target_os = "macos")]
        assert!(result.is_err(), "shared libs download is intentionally unsupported on macOS");
    }
}
