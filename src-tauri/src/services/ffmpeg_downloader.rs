// FFmpeg Auto-Download Service
// Downloads and extracts FFmpeg for the current platform

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use futures_util::StreamExt;
use reqwest::Client;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use thiserror::Error;

/// Progress information emitted during download
#[derive(Debug, Clone, Serialize)]
pub struct DownloadProgress {
    pub downloaded: u64,
    pub total: u64,
    pub percent: f64,
    pub phase: String,
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

    /// Get platform-specific download information
    fn get_platform_download() -> Result<PlatformDownload, DownloadError> {
        #[cfg(target_os = "windows")]
        {
            Ok(PlatformDownload {
                // Windows: gyan.dev essentials build (smaller, ~80MB)
                url: "https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-essentials.zip",
                archive_type: ArchiveType::Zip,
                binary_path: "ffmpeg-7.1-essentials_build/bin/ffmpeg.exe",
            })
        }

        #[cfg(target_os = "linux")]
        {
            Ok(PlatformDownload {
                // Linux: johnvansickle static build
                url: "https://johnvansickle.com/ffmpeg/releases/ffmpeg-release-amd64-static.tar.xz",
                archive_type: ArchiveType::TarXz,
                binary_path: "ffmpeg-7.1-amd64-static/ffmpeg",
            })
        }

        #[cfg(target_os = "macos")]
        {
            // macOS: Use BtbN GitHub releases which provide tar.xz
            #[cfg(target_arch = "aarch64")]
            {
                Ok(PlatformDownload {
                    url: "https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-macosarm64-gpl.tar.xz",
                    archive_type: ArchiveType::TarXz,
                    binary_path: "ffmpeg-master-latest-macosarm64-gpl/bin/ffmpeg",
                })
            }
            #[cfg(target_arch = "x86_64")]
            {
                Ok(PlatformDownload {
                    url: "https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-macos64-gpl.tar.xz",
                    archive_type: ArchiveType::TarXz,
                    binary_path: "ffmpeg-master-latest-macos64-gpl/bin/ffmpeg",
                })
            }
            #[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
            {
                Err(DownloadError::UnsupportedPlatform(format!(
                    "macOS {}",
                    std::env::consts::ARCH
                )))
            }
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

    /// Download and extract FFmpeg
    pub async fn download(&self, app_handle: &AppHandle) -> Result<PathBuf, DownloadError> {
        self.reset_cancel();

        // Get platform-specific download info
        let platform = Self::get_platform_download()?;

        // Determine destination directory
        let app_data_dir = app_handle.path().app_data_dir()
            .map_err(|e| DownloadError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                e.to_string()
            )))?;
        let ffmpeg_dir = app_data_dir.join("ffmpeg");
        std::fs::create_dir_all(&ffmpeg_dir)?;

        // Create temp file for download
        let temp_file = tempfile::NamedTempFile::new_in(&ffmpeg_dir)?;
        let temp_path = temp_file.path().to_path_buf();

        // Emit starting phase
        self.emit_progress(app_handle, 0, 0, 0.0, "starting");

        // Download the file
        self.download_file(platform.url, &temp_path, app_handle).await?;

        if self.is_cancelled() {
            return Err(DownloadError::Cancelled);
        }

        // Extract the archive
        self.emit_progress(app_handle, 0, 0, 0.0, "extracting");

        let ffmpeg_path = self.extract_archive(
            &temp_path,
            &ffmpeg_dir,
            &platform.archive_type,
            platform.binary_path,
        )?;

        // Verify the binary exists and is executable
        self.emit_progress(app_handle, 0, 0, 0.0, "verifying");

        if !ffmpeg_path.exists() {
            return Err(DownloadError::BinaryNotFound);
        }

        // On Unix, make executable
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&ffmpeg_path)?.permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&ffmpeg_path, perms)?;
        }

        // Cleanup temp file
        let _ = std::fs::remove_file(&temp_path);

        // Emit completion
        self.emit_progress(app_handle, 100, 100, 100.0, "complete");

        log::info!("FFmpeg downloaded successfully to: {ffmpeg_path:?}");
        Ok(ffmpeg_path)
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

            self.emit_progress(app_handle, downloaded, total_size, percent, "downloading");
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

            // Check if this is the ffmpeg binary (handle wildcards in path)
            if file_path.ends_with("ffmpeg.exe") || file_path.ends_with("/ffmpeg") {
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
    ) {
        let progress = DownloadProgress {
            downloaded,
            total,
            percent,
            phase: phase.to_string(),
        };

        let _ = app_handle.emit("ffmpeg_download_progress", &progress);
    }

    /// Get the path where FFmpeg would be installed
    pub fn get_ffmpeg_path(app_handle: &AppHandle) -> Option<PathBuf> {
        let app_data_dir = app_handle.path().app_data_dir().ok()?;
        let ffmpeg_dir = app_data_dir.join("ffmpeg");

        let binary_name = if cfg!(windows) { "ffmpeg.exe" } else { "ffmpeg" };
        let path = ffmpeg_dir.join(binary_name);

        if path.exists() {
            Some(path)
        } else {
            None
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
