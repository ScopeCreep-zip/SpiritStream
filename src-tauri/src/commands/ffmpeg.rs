// FFmpeg Download Commands
// Handles FFmpeg auto-download functionality

use std::sync::Arc;
use tauri::{AppHandle, Manager, State};
use tokio::sync::Mutex;

use crate::services::{FFmpegDownloader, FFmpegVersionInfo, SettingsManager, TauriEventSink};

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

    let event_sink = TauriEventSink::new(app.clone());
    match downloader.download(&event_sink).await {
        Ok(path) => {
            log::info!("FFmpeg downloaded to: {path:?}");
            Ok(path.to_string_lossy().to_string())
        }
        Err(e) => {
            log::error!("FFmpeg download failed: {e}");
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
    let settings_manager = app.try_state::<SettingsManager>();
    match FFmpegDownloader::get_ffmpeg_path(settings_manager.as_deref()) {
        Some(path) => Ok(Some(path.to_string_lossy().to_string())),
        None => Ok(None),
    }
}

/// Check for FFmpeg updates
/// Compares installed version against latest available version
#[tauri::command]
pub async fn check_ffmpeg_update(
    installed_version: Option<String>,
    state: State<'_, FFmpegDownloaderState>,
) -> Result<FFmpegVersionInfo, String> {
    log::info!("Checking for FFmpeg updates...");

    let downloader = state.0.lock().await;
    let version_info = downloader
        .check_version_status(installed_version.as_deref())
        .await;

    log::info!(
        "FFmpeg version check: installed={:?}, latest={:?}, update_available={}",
        version_info.installed_version,
        version_info.latest_version,
        version_info.update_available
    );

    Ok(version_info)
}
