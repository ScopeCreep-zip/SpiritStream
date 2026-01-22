// FFmpeg Auto-Download Service
// Downloads and extracts FFmpeg for the current platform

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

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
use tauri::{AppHandle, Emitter, Manager};
use crate::services::SettingsManager;
use thiserror::Error;

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

/// Platform-specific download information
struct PlatformDownload {
    url: &'static str,
    archive_type: ArchiveType,
    binary_path: &'static str,
}

#[allow(dead_code)] // Variants are used on specific platforms only
enum ArchiveType {
    Zip,
    TarXz,
    TarGz,
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

    fn required_hardware_encoders() -> &'static [&'static str] {
        #[cfg(target_os = "windows")]
        {
            &["h264_nvenc", "h264_qsv", "h264_amf"]
        }

        #[cfg(target_os = "linux")]
        {
            &["h264_nvenc", "h264_qsv", "h264_vaapi"]
        }

        #[cfg(target_os = "macos")]
        {
            &["h264_videotoolbox"]
        }

        #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
        {
            &[]
        }
    }

    fn validate_hardware_support(ffmpeg_path: &Path) -> Result<(), DownloadError> {
        let required = Self::required_hardware_encoders();
        if required.is_empty() {
            return Ok(());
        }

        #[cfg(unix)]
        {
            if let Ok(metadata) = std::fs::metadata(ffmpeg_path) {
                let mut perms = metadata.permissions();
                // 0o111 => any execute bit (owner/group/other); if none are set, make it user/group/world executable.
                if (perms.mode() & 0o111) == 0 {
                    perms.set_mode(0o755);
                    let _ = std::fs::set_permissions(ffmpeg_path, perms);
                }
            }
        }

        let output = Command::new(ffmpeg_path)
            .args(["-encoders", "-hide_banner"])
            .output()
            .map_err(|e| DownloadError::InstallationFailed(format!("Failed to run downloaded FFmpeg: {e}")))?;

        if !output.status.success() {
            return Err(DownloadError::InstallationFailed(
                "Downloaded FFmpeg returned an error while listing encoders".to_string(),
            ));
        }

        let encoder_list = String::from_utf8_lossy(&output.stdout);
        let missing: Vec<&str> = required
            .iter()
            .copied()
            .filter(|encoder| !encoder_list.contains(encoder))
            .collect();

        if !missing.is_empty() {
            return Err(DownloadError::UnsupportedBuild(missing.join(", ")));
        }

        Ok(())
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

    /// Get platform-specific download information
    fn get_platform_download() -> Result<PlatformDownload, DownloadError> {
        #[cfg(target_os = "windows")]
        {
            Ok(PlatformDownload {
                // Windows: hardware-enabled build (NVENC/QSV/AMF)
                // BtbN Windows ZIPs currently expose `ffmpeg.exe` at the archive
                // root (no additional subdirectory), so we only need the bare
                // filename here. If the upstream archive layout changes (e.g. a
                // top-level directory is added), this binary_path will need to be
                // updated to match the new internal path.
                url: "https://github.com/BtbN/FFmpeg-Builds/releases/latest/download/ffmpeg-master-latest-win64-gpl.zip",
                archive_type: ArchiveType::Zip,
                binary_path: "ffmpeg.exe",
            })
        }

        #[cfg(target_os = "linux")]
        {
            Ok(PlatformDownload {
                // Linux: hardware-enabled static build (NVENC/QSV/VAAPI)
                url: "https://github.com/BtbN/FFmpeg-Builds/releases/latest/download/ffmpeg-master-latest-linux64-gpl.tar.xz",
                archive_type: ArchiveType::TarXz,
                binary_path: "ffmpeg",
            })
        }

        #[cfg(target_os = "macos")]
        {
            // macOS: evermeet.cx is the official FFmpeg-recommended source
            // Provides Intel builds that run on ARM Macs via Rosetta 2
            Ok(PlatformDownload {
                url: "https://evermeet.cx/ffmpeg/getrelease/zip",
                archive_type: ArchiveType::Zip,
                binary_path: "ffmpeg", // ZIP contains just the binary
            })
        }

        #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
        {
            Err(DownloadError::UnsupportedPlatform(format!(
                "{} {}",
                std::env::consts::OS,
                std::env::consts::ARCH
            )))
        }
    }

    /// Get the system installation path for FFmpeg
    pub fn get_system_install_path() -> PathBuf {
        #[cfg(target_os = "windows")]
        {
            PathBuf::from(r"C:\Program Files\ffmpeg\bin\ffmpeg.exe")
        }

        #[cfg(target_os = "macos")]
        {
            #[cfg(target_arch = "aarch64")]
            {
                PathBuf::from("/opt/homebrew/bin/ffmpeg")
            }
            #[cfg(not(target_arch = "aarch64"))]
            {
                PathBuf::from("/usr/local/bin/ffmpeg")
            }
        }

        #[cfg(target_os = "linux")]
        {
            PathBuf::from("/usr/local/bin/ffmpeg")
        }

        #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
        {
            PathBuf::from("ffmpeg")
        }
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
    pub async fn download(&self, app_handle: &AppHandle) -> Result<PathBuf, DownloadError> {
        self.reset_cancel();

        // Get system install path
        let target_path = Self::get_system_install_path();

        // Check if already installed at system location
        if target_path.exists() {
            log::info!("FFmpeg already installed at: {target_path:?}");
            self.emit_progress(app_handle, 100, 100, 100.0, "complete", Some("FFmpeg is already installed"));
            return Ok(target_path);
        }

        // Get platform-specific download info
        let platform = Self::get_platform_download()?;

        // Create temp directory for download and extraction
        let temp_dir = tempfile::tempdir()?;
        let archive_path = temp_dir.path().join("ffmpeg_archive");
        let temp_binary = temp_dir.path().join(
            if cfg!(windows) { "ffmpeg.exe" } else { "ffmpeg" }
        );

        // Emit starting phase
        self.emit_progress(app_handle, 0, 0, 0.0, "starting", None);

        // Download the file
        self.download_file(platform.url, &archive_path, app_handle).await?;

        if self.is_cancelled() {
            return Err(DownloadError::Cancelled);
        }

        // Extract the archive to temp location
        self.emit_progress(app_handle, 0, 0, 0.0, "extracting", None);

        self.extract_archive(
            &archive_path,
            temp_dir.path(),
            &platform.archive_type,
            platform.binary_path,
        )?;

        // Verify extracted binary
        self.emit_progress(app_handle, 0, 0, 0.0, "verifying", None);

        if !temp_binary.exists() {
            return Err(DownloadError::BinaryNotFound);
        }

        Self::validate_hardware_support(&temp_binary)?;

        // Request elevation and install to system location
        self.emit_progress(
            app_handle,
            0, 0, 0.0,
            "requesting_permission",
            Some("Administrator permission required to install FFmpeg")
        );

        match Self::install_with_elevation(&temp_binary, &target_path) {
            Ok(()) => {
                self.emit_progress(
                    app_handle,
                    100, 100, 100.0,
                    "complete",
                    Some("FFmpeg installed successfully")
                );
                log::info!("FFmpeg installed successfully to: {target_path:?}");
                Ok(target_path)
            }
            Err(DownloadError::ElevationDenied) => {
                self.emit_progress(
                    app_handle,
                    0, 0, 0.0,
                    "elevation_denied",
                    Some("Permission denied - FFmpeg was not installed")
                );
                Err(DownloadError::ElevationDenied)
            }
            Err(e) => {
                self.emit_progress(
                    app_handle,
                    0, 0, 0.0,
                    "error",
                    Some(&format!("Installation failed: {e}"))
                );
                Err(e)
            }
        }
        // temp_dir is automatically cleaned up when dropped
    }

    /// Download file with progress reporting
    async fn download_file(
        &self,
        url: &str,
        dest: &PathBuf,
        app_handle: &AppHandle,
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

            self.emit_progress(app_handle, downloaded, total_size, percent, "downloading", None);
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

    /// Emit progress event to frontend
    fn emit_progress(
        &self,
        app_handle: &AppHandle,
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

        let _ = app_handle.emit("ffmpeg_download_progress", &progress);
    }

    /// Get the path where FFmpeg is installed
    /// Checks in order: 1) Custom path from settings, 2) System install location
    pub fn get_ffmpeg_path(app_handle: &AppHandle) -> Option<PathBuf> {
        // First, check for custom path in settings (set via browse functionality)
        if let Some(settings_manager) = app_handle.try_state::<SettingsManager>() {
            if let Ok(settings) = settings_manager.load() {
                if !settings.ffmpeg_path.is_empty() {
                    let custom_path = PathBuf::from(&settings.ffmpeg_path);
                    if custom_path.exists() {
                        log::debug!("Using custom FFmpeg path from settings: {custom_path:?}");
                        return Some(custom_path);
                    } else {
                        log::warn!("Custom FFmpeg path in settings does not exist: {custom_path:?}");
                    }
                }
            }
        }

        // Fall back to system install location
        let system_path = Self::get_system_install_path();
        if system_path.exists() {
            log::debug!("Using system FFmpeg path: {system_path:?}");
            Some(system_path)
        } else {
            None
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
