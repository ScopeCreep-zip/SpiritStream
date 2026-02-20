use crate::app_state::{AppState, get_arg, get_opt_arg};
use crate::models::Settings;
use crate::services::EventSink;
use crate::services::{prune_logs, read_recent_logs, validate_path_within_any, Encryption, FFmpegDownloader};
use serde_json::{json, Value};
use std::path::PathBuf;

pub async fn handle(state: &AppState, command: &str, payload: &Value) -> Option<Result<Value, String>> {
    match command {
        "get_recent_logs" => Some(get_recent_logs(state, payload)),
        "export_logs" => Some(export_logs(state, payload)),
        "get_settings" => Some(get_settings(state)),
        "save_settings" => Some(save_settings(state, payload)),
        "get_profiles_path" => Some(get_profiles_path(state)),
        "export_data" => Some(export_data(state, payload)),
        "clear_data" => Some(clear_data(state)),
        "rotate_machine_key" => Some(rotate_machine_key(state)),
        "download_ffmpeg" => Some(download_ffmpeg(state).await),
        "cancel_ffmpeg_download" => Some(cancel_ffmpeg_download(state).await),
        "get_bundled_ffmpeg_path" => Some(get_bundled_ffmpeg_path(state)),
        "check_ffmpeg_update" => Some(check_ffmpeg_update(state, payload).await),
        _ => None,
    }
}

fn get_recent_logs(state: &AppState, payload: &Value) -> Result<Value, String> {
    let max_lines: Option<usize> = get_opt_arg(payload, "maxLines")?;
    Ok(json!(read_recent_logs(
        &state.log_dir,
        max_lines.unwrap_or(500)
    )?))
}

fn export_logs(state: &AppState, payload: &Value) -> Result<Value, String> {
    let path: String = get_arg(payload, "path")?;
    let content: String = get_arg(payload, "content")?;

    // Security: Validate export path is within allowed directories
    let export_path = PathBuf::from(&path);

    // Build list of allowed directories
    let mut allowed_dirs: Vec<&std::path::Path> = vec![state.app_data_dir.as_path()];
    if let Some(ref home) = state.home_dir {
        allowed_dirs.push(home.as_path());
    }

    // Validate the path
    validate_path_within_any(&export_path, &allowed_dirs)?;

    std::fs::write(&path, content).map_err(|e| format!("Failed to write log file: {e}"))?;
    Ok(Value::Null)
}

fn get_settings(state: &AppState) -> Result<Value, String> {
    Ok(json!(state.settings_manager.load()?))
}

fn save_settings(state: &AppState, payload: &Value) -> Result<Value, String> {
    let settings: Settings = get_arg(payload, "settings")?;
    state.settings_manager.save(&settings)?;
    let _ = prune_logs(&state.log_dir, settings.log_retention_days);
    state.event_bus.emit("settings_changed", json!({}));
    Ok(Value::Null)
}

fn get_profiles_path(state: &AppState) -> Result<Value, String> {
    let path = state.settings_manager.get_profiles_path();
    Ok(json!(path.to_string_lossy().to_string()))
}

fn export_data(state: &AppState, payload: &Value) -> Result<Value, String> {
    let export_path: String = get_arg(payload, "exportPath")?;
    let path = PathBuf::from(&export_path);

    // Security: Validate export path is within allowed directories
    let mut allowed_dirs: Vec<&std::path::Path> = vec![state.app_data_dir.as_path()];
    if let Some(ref home) = state.home_dir {
        allowed_dirs.push(home.as_path());
    }

    validate_path_within_any(&path, &allowed_dirs)?;

    state.settings_manager.export_data(&path)?;
    Ok(Value::Null)
}

fn clear_data(state: &AppState) -> Result<Value, String> {
    state.settings_manager.clear_data()?;
    Ok(Value::Null)
}

fn rotate_machine_key(state: &AppState) -> Result<Value, String> {
    let profiles_dir = state.app_data_dir.join("profiles");
    let report = Encryption::rotate_machine_key(&state.app_data_dir, &profiles_dir)?;
    Ok(json!(report))
}

async fn download_ffmpeg(state: &AppState) -> Result<Value, String> {
    let downloader = state.ffmpeg_downloader.lock().await;
    let path = downloader
        .download(&state.event_bus)
        .await
        .map_err(|e| e.to_string())?;
    Ok(json!(path.to_string_lossy().to_string()))
}

async fn cancel_ffmpeg_download(state: &AppState) -> Result<Value, String> {
    let downloader = state.ffmpeg_downloader.lock().await;
    downloader.cancel();
    Ok(Value::Null)
}

fn get_bundled_ffmpeg_path(state: &AppState) -> Result<Value, String> {
    let path = FFmpegDownloader::get_ffmpeg_path(Some(&state.settings_manager));
    Ok(json!(path.map(|p| p.to_string_lossy().to_string())))
}

async fn check_ffmpeg_update(state: &AppState, payload: &Value) -> Result<Value, String> {
    let installed_version: Option<String> = get_opt_arg(payload, "installedVersion")?;
    let downloader = state.ffmpeg_downloader.lock().await;
    let info = downloader
        .check_version_status(installed_version.as_deref())
        .await;
    Ok(json!(info))
}
