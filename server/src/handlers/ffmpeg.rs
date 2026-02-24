use serde_json::{json, Value};
use std::path::PathBuf;

use crate::state::AppState;
use crate::util::{get_arg, get_opt_arg};
use spiritstream_server::commands::{get_encoders, test_ffmpeg, test_rtmp_target, validate_ffmpeg_path};
use spiritstream_server::services::{read_recent_logs, validate_path_within_any, FFmpegDownloader};

pub(crate) async fn handle(state: &AppState, command: &str, payload: &Value) -> Result<Value, String> {
    match command {
        "get_encoders" => Ok(json!(get_encoders()?)),
        "test_ffmpeg" => Ok(json!(test_ffmpeg()?)),
        "validate_ffmpeg_path" => {
            let path: String = get_arg(payload, "path")?;
            Ok(json!(validate_ffmpeg_path(path)?))
        }
        "test_rtmp_target" => {
            let url: String = get_arg(payload, "url")?;
            let stream_key: String = get_arg(payload, "streamKey")?;
            Ok(json!(test_rtmp_target(url, stream_key)?))
        }
        "get_recent_logs" => {
            let max_lines: Option<usize> = get_opt_arg(payload, "maxLines")?;
            Ok(json!(read_recent_logs(&state.log_dir, max_lines.unwrap_or(500))?))
        }
        "export_logs" => {
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
        "download_ffmpeg" => {
            let downloader = state.ffmpeg_downloader.lock().await;
            let path = downloader
                .download(&state.event_bus)
                .await
                .map_err(|e| e.to_string())?;
            Ok(json!(path.to_string_lossy().to_string()))
        }
        "cancel_ffmpeg_download" => {
            let downloader = state.ffmpeg_downloader.lock().await;
            downloader.cancel();
            Ok(Value::Null)
        }
        "get_bundled_ffmpeg_path" => {
            let path = FFmpegDownloader::get_ffmpeg_path(Some(&state.settings_manager));
            Ok(json!(path.map(|p| p.to_string_lossy().to_string())))
        }
        "check_ffmpeg_update" => {
            let installed_version: Option<String> = get_opt_arg(payload, "installedVersion")?;
            let downloader = state.ffmpeg_downloader.lock().await;
            let info = downloader
                .check_version_status(installed_version.as_deref())
                .await;
            Ok(json!(info))
        }
        "delete_ffmpeg" => {
            FFmpegDownloader::delete_ffmpeg(Some(&state.settings_manager))
                .map_err(|e| e.to_string())?;
            Ok(Value::Null)
        }
        _ => Err(format!("Unknown ffmpeg command: {command}")),
    }
}
