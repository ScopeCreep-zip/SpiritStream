// FFmpeg Auto-Download Service
// Downloads and extracts FFmpeg for the current platform

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;


// Windows: Hide console windows for spawned processes
#[cfg(windows)]
use std::os::windows::process::CommandExt;
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

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

    /// Install FFmpeg to system location with privilege elevation
    fn install_with_elevation(temp_path: &Path, target_path: &Path) -> Result<(), DownloadError> {
        #[cfg(target_os = "windows")]
        {
            Self::install_windows(temp_path, target_path)
        }

        #[cfg(target_os = "macos")]
        {
            Self::install_macos(temp_path, target_path)
        }

        #[cfg(target_os = "linux")]
        {
            Self::install_linux(temp_path, target_path)
        }

        #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
        {
            Err(DownloadError::UnsupportedPlatform("elevation not supported".to_string()))
        }
    }

    /// Install FFmpeg on Windows using UAC elevation
    #[cfg(target_os = "windows")]
    fn install_windows(temp_path: &Path, target_path: &Path) -> Result<(), DownloadError> {
        let target_dir = target_path.parent()
            .ok_or_else(|| DownloadError::InstallationFailed("Invalid target path".to_string()))?;

        // Create a temporary batch script with a descriptive name
        // This makes UAC show "Install-FFmpeg-MagillaStream.bat" instead of generic "powershell"
        let temp_script_dir = std::env::temp_dir();
        let batch_path = temp_script_dir.join("Install-FFmpeg-MagillaStream.bat");

        let batch_content = format!(
            r#"@echo off
:: MagillaStream FFmpeg Installer
:: This script installs FFmpeg to Program Files
if not exist "{}" mkdir "{}"
copy /Y "{}" "{}"
exit /b %errorlevel%
"#,
            target_dir.display(),
            target_dir.display(),
            temp_path.display(),
            target_path.display()
        );

        std::fs::write(&batch_path, &batch_content)
            .map_err(|e| DownloadError::InstallationFailed(format!("Failed to create installer script: {e}")))?;

        log::info!("Requesting Windows UAC elevation to install FFmpeg");

        // Use cmd to run the batch file with elevation - UAC will show the batch file name
        let mut cmd = Command::new("powershell");
        cmd.args([
            "-NoProfile",
            "-Command",
            &format!(
                "Start-Process cmd -Verb RunAs -Wait -WindowStyle Hidden -ArgumentList '/c \"{}\"'",
                batch_path.display()
            )
        ]);
        cmd.creation_flags(CREATE_NO_WINDOW);
        let output = cmd.output()
            .map_err(|e| DownloadError::ElevationFailed(e.to_string()))?;

        // Clean up the batch file
        let _ = std::fs::remove_file(&batch_path);

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            // Check if user cancelled UAC
            if stderr.contains("canceled") || stderr.contains("denied") || stderr.contains("The operation was canceled") {
                return Err(DownloadError::ElevationDenied);
            }
            return Err(DownloadError::InstallationFailed(stderr.to_string()));
        }

        // Verify installation
        if !target_path.exists() {
            // UAC might have been cancelled without error message
            return Err(DownloadError::ElevationDenied);
        }

        log::info!("FFmpeg installed to: {target_path:?}");
        Ok(())
    }

    /// Install FFmpeg on macOS using AppleScript elevation
    #[cfg(target_os = "macos")]
    fn install_macos(temp_path: &Path, target_path: &Path) -> Result<(), DownloadError> {
        let target_dir = target_path.parent()
            .ok_or_else(|| DownloadError::InstallationFailed("Invalid target path".to_string()))?;

        // Use osascript with administrator privileges to show native macOS auth dialog
        let script = format!(
            r#"do shell script "mkdir -p '{}' && cp '{}' '{}' && chmod +x '{}'" with administrator privileges"#,
            target_dir.display(),
            temp_path.display(),
            target_path.display(),
            target_path.display()
        );

        log::info!("Requesting macOS administrator privileges to install FFmpeg");

        let output = Command::new("osascript")
            .args(["-e", &script])
            .output()
            .map_err(|e| DownloadError::ElevationFailed(e.to_string()))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            // User clicked Cancel in the auth dialog
            if stderr.contains("User canceled") || stderr.contains("-128") {
                return Err(DownloadError::ElevationDenied);
            }
            return Err(DownloadError::InstallationFailed(stderr.to_string()));
        }

        // Verify installation
        if !target_path.exists() {
            return Err(DownloadError::InstallationFailed("File not found after installation".to_string()));
        }

        log::info!("FFmpeg installed to: {target_path:?}");
        Ok(())
    }

    /// Install FFmpeg on Linux using pkexec (PolicyKit) elevation
    #[cfg(target_os = "linux")]
    fn install_linux(temp_path: &Path, target_path: &Path) -> Result<(), DownloadError> {
        let target_dir = target_path.parent()
            .ok_or_else(|| DownloadError::InstallationFailed("Invalid target path".to_string()))?;

        // Use pkexec (PolicyKit) for graphical sudo prompt
        let script = format!(
            "mkdir -p '{}' && cp '{}' '{}' && chmod +x '{}'",
            target_dir.display(),
            temp_path.display(),
            target_path.display(),
            target_path.display()
        );

        log::info!("Requesting Linux administrator privileges via pkexec to install FFmpeg");

        // Try pkexec first (graphical)
        let output = Command::new("pkexec")
            .args(["sh", "-c", &script])
            .output();

        match output {
            Ok(output) => {
                if output.status.success() {
                    // Verify installation
                    if target_path.exists() {
                        log::info!("FFmpeg installed to: {target_path:?}");
                        return Ok(());
                    }
                    return Err(DownloadError::InstallationFailed("File not found after installation".to_string()));
                }

                let exit_code = output.status.code().unwrap_or(-1);
                // pkexec exit code 126 = auth dismissed/denied
                if exit_code == 126 {
                    return Err(DownloadError::ElevationDenied);
                }

                let stderr = String::from_utf8_lossy(&output.stderr);
                Err(DownloadError::InstallationFailed(stderr.to_string()))
            }
            Err(e) => {
                // pkexec not available - try fallbacks
                log::warn!("pkexec not available: {e}. Trying gksudo/kdesudo...");

                // Try gksudo (GNOME) or kdesudo (KDE) as fallbacks
                for cmd in ["gksudo", "kdesudo"] {
                    if let Ok(output) = Command::new(cmd)
                        .args(["--", "sh", "-c", &script])
                        .output()
                    {
                        if output.status.success() && target_path.exists() {
                            log::info!("FFmpeg installed to: {target_path:?}");
                            return Ok(());
                        }
                    }
                }

                Err(DownloadError::ElevationFailed(
                    "No graphical sudo available (pkexec, gksudo, or kdesudo)".to_string()
                ))
            }
        }
    }

    /// Download and extract FFmpeg to system location with elevation
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
            ArchiveType::TarGz => {
                // Reuse tar.xz logic for tar.gz (similar structure)
                let file = std::fs::File::open(&archive_path)?;
                let decompressor = flate2::read::GzDecoder::new(file);
                let mut archive = tar::Archive::new(decompressor);

                std::fs::create_dir_all(libs_dir.join("bin"))?;
                std::fs::create_dir_all(libs_dir.join("lib"))?;
                std::fs::create_dir_all(libs_dir.join("include"))?;

                for entry in archive.entries()? {
                    let mut entry = entry?;
                    let path = entry.path()?;
                    let path_str = path.to_string_lossy();

                    let relative_path = if path_str.starts_with(&platform.archive_root) {
                        path_str[platform.archive_root.len()..].trim_start_matches('/')
                    } else {
                        &path_str
                    };

                    let should_extract = relative_path.starts_with("bin/")
                        || relative_path.starts_with("lib/")
                        || relative_path.starts_with("include/");

                    if should_extract && entry.header().entry_type().is_file() {
                        let dest_path = libs_dir.join(relative_path);
                        if let Some(parent) = dest_path.parent() {
                            std::fs::create_dir_all(parent)?;
                        }
                        let mut outfile = std::fs::File::create(&dest_path)?;
                        std::io::copy(&mut entry, &mut outfile)?;
                    }
                }
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

    /// Extract archive and return path to FFmpeg binary
    fn extract_archive(
        &self,
        archive_path: &Path,
        dest_dir: &Path,
        archive_type: &ArchiveType,
        binary_path: &str,
    ) -> Result<PathBuf, DownloadError> {
        log::info!("Extracting archive to: {dest_dir:?}");

        match archive_type {
            ArchiveType::Zip => self.extract_zip(archive_path, dest_dir, binary_path),
            ArchiveType::TarXz => self.extract_tar_xz(archive_path, dest_dir, binary_path),
            ArchiveType::TarGz => self.extract_tar_gz(archive_path, dest_dir, binary_path),
        }
    }

    /// Extract ZIP archive
    fn extract_zip(
        &self,
        archive_path: &Path,
        dest_dir: &Path,
        _binary_path: &str,
    ) -> Result<PathBuf, DownloadError> {
        let file = std::fs::File::open(archive_path)?;
        let mut archive = zip::ZipArchive::new(file)
            .map_err(|e| DownloadError::ExtractionFailed(e.to_string()))?;

        // Find and extract FFmpeg binary
        for i in 0..archive.len() {
            let mut file = archive.by_index(i)
                .map_err(|e| DownloadError::ExtractionFailed(e.to_string()))?;

            let file_path = file.name();

            // Check if this is the ffmpeg binary (handle various path formats)
            if file_path.ends_with("ffmpeg.exe") || file_path.ends_with("/ffmpeg") || file_path == "ffmpeg" {
                let dest_path = dest_dir.join(
                    if cfg!(windows) { "ffmpeg.exe" } else { "ffmpeg" }
                );

                let mut outfile = std::fs::File::create(&dest_path)?;
                std::io::copy(&mut file, &mut outfile)?;

                log::info!("Extracted FFmpeg binary to: {dest_path:?}");
                return Ok(dest_path);
            }
        }

        // Fallback: try the exact path
        let dest_path = dest_dir.join(if cfg!(windows) { "ffmpeg.exe" } else { "ffmpeg" });
        if dest_path.exists() {
            return Ok(dest_path);
        }

        Err(DownloadError::BinaryNotFound)
    }

    /// Extract TAR.XZ archive
    fn extract_tar_xz(
        &self,
        archive_path: &Path,
        dest_dir: &Path,
        _binary_path: &str,
    ) -> Result<PathBuf, DownloadError> {
        let file = std::fs::File::open(archive_path)?;
        let decompressor = xz2::read::XzDecoder::new(file);
        let mut archive = tar::Archive::new(decompressor);

        // Extract all files first
        for entry in archive.entries()? {
            let mut entry = entry?;
            let path = entry.path()?;
            let path_str = path.to_string_lossy();

            // Only extract the ffmpeg binary
            if path_str.ends_with("/ffmpeg") || path_str.ends_with("/bin/ffmpeg") {
                let dest_path = dest_dir.join("ffmpeg");
                let mut outfile = std::fs::File::create(&dest_path)?;
                std::io::copy(&mut entry, &mut outfile)?;

                log::info!("Extracted FFmpeg binary to: {dest_path:?}");
                return Ok(dest_path);
            }
        }

        Err(DownloadError::BinaryNotFound)
    }

    /// Extract TAR.GZ archive
    fn extract_tar_gz(
        &self,
        archive_path: &Path,
        dest_dir: &Path,
        _binary_path: &str,
    ) -> Result<PathBuf, DownloadError> {
        let file = std::fs::File::open(archive_path)?;
        let decompressor = flate2::read::GzDecoder::new(file);
        let mut archive = tar::Archive::new(decompressor);

        // Extract all files first
        for entry in archive.entries()? {
            let mut entry = entry?;
            let path = entry.path()?;
            let path_str = path.to_string_lossy();

            // Only extract the ffmpeg binary
            if path_str.ends_with("/ffmpeg") || path_str.ends_with("/bin/ffmpeg") {
                let dest_path = dest_dir.join("ffmpeg");
                let mut outfile = std::fs::File::create(&dest_path)?;
                std::io::copy(&mut entry, &mut outfile)?;

                log::info!("Extracted FFmpeg binary to: {dest_path:?}");
                return Ok(dest_path);
            }
        }

        Err(DownloadError::BinaryNotFound)
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

    /// Delete FFmpeg from the system with privilege elevation
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

    /// Check if a path requires elevation to modify
    fn path_needs_elevation(path: &Path) -> bool {
        #[cfg(target_os = "windows")]
        {
            // Windows: Program Files and system directories need elevation
            let path_str = path.to_string_lossy().to_lowercase();
            path_str.contains("program files") || path_str.contains("windows")
        }

        #[cfg(any(target_os = "macos", target_os = "linux"))]
        {
            // Unix: /usr, /opt, /bin, /sbin need elevation
            let path_str = path.to_string_lossy();
            path_str.starts_with("/usr") ||
            path_str.starts_with("/opt") ||
            path_str.starts_with("/bin") ||
            path_str.starts_with("/sbin")
        }

        #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
        {
            let _ = path;
            false
        }
    }

    /// Delete FFmpeg with privilege elevation
    fn delete_with_elevation(target_path: &Path) -> Result<(), DownloadError> {
        #[cfg(target_os = "windows")]
        {
            Self::delete_windows(target_path)
        }

        #[cfg(target_os = "macos")]
        {
            Self::delete_macos(target_path)
        }

        #[cfg(target_os = "linux")]
        {
            Self::delete_linux(target_path)
        }

        #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
        {
            let _ = target_path;
            Err(DownloadError::UnsupportedPlatform("elevation not supported".to_string()))
        }
    }

    /// Delete FFmpeg on Windows using UAC elevation
    #[cfg(target_os = "windows")]
    fn delete_windows(target_path: &Path) -> Result<(), DownloadError> {
        let temp_script_dir = std::env::temp_dir();
        let batch_path = temp_script_dir.join("Uninstall-FFmpeg-SpiritStream.bat");

        let batch_content = format!(
            r#"@echo off
:: SpiritStream FFmpeg Uninstaller
:: This script removes FFmpeg from the system
if exist "{}" del /F /Q "{}"
exit /b %errorlevel%
"#,
            target_path.display(),
            target_path.display()
        );

        std::fs::write(&batch_path, &batch_content)
            .map_err(|e| DownloadError::InstallationFailed(format!("Failed to create uninstaller script: {e}")))?;

        log::info!("Requesting Windows UAC elevation to delete FFmpeg");

        let mut cmd = Command::new("powershell");
        cmd.args([
            "-NoProfile",
            "-Command",
            &format!(
                "Start-Process cmd -Verb RunAs -Wait -WindowStyle Hidden -ArgumentList '/c \"{}\"'",
                batch_path.display()
            )
        ]);
        cmd.creation_flags(CREATE_NO_WINDOW);
        let output = cmd.output()
            .map_err(|e| DownloadError::ElevationFailed(e.to_string()))?;

        // Clean up the batch file
        let _ = std::fs::remove_file(&batch_path);

        // Check both stdout and stderr for cancellation messages
        // UAC cancellation errors can appear in either stream
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let combined_output = format!("{} {}", stdout, stderr).to_lowercase();

        log::debug!("UAC delete stdout: {}", stdout);
        log::debug!("UAC delete stderr: {}", stderr);
        log::debug!("UAC delete exit code: {:?}", output.status.code());

        // Check for cancellation indicators
        if combined_output.contains("canceled")
            || combined_output.contains("cancelled")
            || combined_output.contains("denied")
            || combined_output.contains("the operation was canceled")
            || combined_output.contains("user account control")
            || combined_output.contains("access is denied") {
            log::warn!("UAC elevation was denied or cancelled");
            return Err(DownloadError::ElevationDenied);
        }

        // Primary verification: check if file still exists
        // This is the most reliable way to know if deletion succeeded
        if target_path.exists() {
            log::warn!("FFmpeg file still exists after delete attempt - UAC likely cancelled");
            return Err(DownloadError::ElevationDenied);
        }

        log::info!("FFmpeg deleted from: {target_path:?}");
        Ok(())
    }

    /// Delete FFmpeg on macOS using AppleScript elevation
    #[cfg(target_os = "macos")]
    fn delete_macos(target_path: &Path) -> Result<(), DownloadError> {
        let script = format!(
            r#"do shell script "rm -f '{}'" with administrator privileges"#,
            target_path.display()
        );

        log::info!("Requesting macOS administrator privileges to delete FFmpeg");

        let output = Command::new("osascript")
            .args(["-e", &script])
            .output()
            .map_err(|e| DownloadError::ElevationFailed(e.to_string()))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("User canceled") || stderr.contains("-128") {
                return Err(DownloadError::ElevationDenied);
            }
            return Err(DownloadError::InstallationFailed(stderr.to_string()));
        }

        // Verify deletion
        if target_path.exists() {
            return Err(DownloadError::InstallationFailed("File still exists after deletion".to_string()));
        }

        log::info!("FFmpeg deleted from: {target_path:?}");
        Ok(())
    }

    /// Delete FFmpeg on Linux using pkexec elevation
    #[cfg(target_os = "linux")]
    fn delete_linux(target_path: &Path) -> Result<(), DownloadError> {
        log::info!("Requesting Linux administrator privileges via pkexec to delete FFmpeg");

        let output = Command::new("pkexec")
            .args(["rm", "-f", &target_path.to_string_lossy()])
            .output();

        match output {
            Ok(output) => {
                if output.status.success() {
                    if !target_path.exists() {
                        log::info!("FFmpeg deleted from: {target_path:?}");
                        return Ok(());
                    }
                    return Err(DownloadError::InstallationFailed("File still exists after deletion".to_string()));
                }

                let exit_code = output.status.code().unwrap_or(-1);
                if exit_code == 126 {
                    return Err(DownloadError::ElevationDenied);
                }

                let stderr = String::from_utf8_lossy(&output.stderr);
                Err(DownloadError::InstallationFailed(stderr.to_string()))
            }
            Err(e) => {
                log::warn!("pkexec not available: {e}. Trying gksudo/kdesudo...");

                for cmd in ["gksudo", "kdesudo"] {
                    if let Ok(output) = Command::new(cmd)
                        .args(["--", "rm", "-f", &target_path.to_string_lossy()])
                        .output()
                    {
                        if output.status.success() && !target_path.exists() {
                            log::info!("FFmpeg deleted from: {target_path:?}");
                            return Ok(());
                        }
                    }
                }

                Err(DownloadError::ElevationFailed(
                    "No graphical sudo available (pkexec, gksudo, or kdesudo)".to_string()
                ))
            }
        }
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
    fn test_get_platform_download() {
        // Should not error on supported platforms
        let result = FFmpegDownloader::get_platform_download();
        assert!(result.is_ok(), "Platform should be supported");
    }
}
