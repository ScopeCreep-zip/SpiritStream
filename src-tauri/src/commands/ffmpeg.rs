// FFmpeg Download Commands
// Handles FFmpeg auto-download functionality

use std::sync::Arc;
use tauri::{AppHandle, State};
use tokio::sync::Mutex;

use crate::services::FFmpegDownloader;

/// State wrapper for the FFmpeg downloader
pub struct FFmpegDownloaderState(pub Arc<Mutex<FFmpegDownloader>>);

/// Download FFmpeg for the current platform
#[tauri::command]
pub async fn download_ffmpeg(
    app: AppHandle,
    state: State<'_, FFmpegDownloaderState>,
) -> Result<String, String> {
    log::info!("Starting FFmpeg download...");

    let downloader = state.0.lock().await;

    match downloader.download(&app).await {
        Ok(path) => {
            log::info!("FFmpeg downloaded to: {:?}", path);
            Ok(path.to_string_lossy().to_string())
        }
        Err(e) => {
            log::error!("FFmpeg download failed: {}", e);
            Err(e.to_string())
        }
    }
}

/// Cancel an in-progress FFmpeg download
#[tauri::command]
pub async fn cancel_ffmpeg_download(
    state: State<'_, FFmpegDownloaderState>,
) -> Result<(), String> {
    log::info!("Cancelling FFmpeg download...");

    let downloader = state.0.lock().await;
    downloader.cancel();

    Ok(())
}

/// Get the path to the bundled/downloaded FFmpeg binary
#[tauri::command]
pub fn get_bundled_ffmpeg_path(app: AppHandle) -> Result<Option<String>, String> {
    match FFmpegDownloader::get_ffmpeg_path(&app) {
        Some(path) => Ok(Some(path.to_string_lossy().to_string())),
        None => Ok(None),
    }
}
