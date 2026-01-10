// SpiritStream - Tauri Backend
// Multi-Destination Streaming Application

mod commands;
mod models;
mod services;

use std::sync::Arc;
use tauri::{image::Image, Manager, RunEvent};
use tauri_plugin_log::{Target, TargetKind};
use tokio::sync::Mutex;
use services::{ProfileManager, FFmpegHandler, FFmpegDownloader, SettingsManager, ThemeManager};
use commands::FFmpegDownloaderState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let mut targets = vec![
                Target::new(TargetKind::LogDir {
                    file_name: Some("spiritstream".to_string()),
                }),
                Target::new(TargetKind::Webview),
            ];
            if cfg!(debug_assertions) {
                targets.push(Target::new(TargetKind::Stdout));
            }
            app.handle().plugin(
                tauri_plugin_log::Builder::default()
                    .level(log::LevelFilter::Info)
                    .targets(targets)
                    .build(),
            )?;

            // Get app data directory for profile storage
            let app_data_dir = app.path().app_data_dir()
                .map_err(|e| format!("Failed to get app data directory: {e}"))?;
            std::fs::create_dir_all(&app_data_dir).ok();

            // Register ProfileManager as managed state
            let profile_manager = ProfileManager::new(app_data_dir.clone());
            app.manage(profile_manager);

            // Register SettingsManager as managed state (load early to get custom FFmpeg path)
            let settings_manager = SettingsManager::new(app_data_dir.clone());

            // Load settings to get custom ffmpeg_path if configured
            let settings = settings_manager.load().ok();
            let custom_ffmpeg_path = settings
                .as_ref()
                .and_then(|s| if s.ffmpeg_path.is_empty() { None } else { Some(s.ffmpeg_path.clone()) });

            if let Some(settings) = settings.as_ref() {
                if let Err(error) = services::prune_logs(&app.handle(), settings.log_retention_days) {
                    log::warn!("Failed to prune logs: {error}");
                }
            }

            // Register FFmpegHandler with custom path from settings (falls back to auto-discovery)
            let ffmpeg_handler = FFmpegHandler::new_with_custom_path(app_data_dir.clone(), custom_ffmpeg_path);
            app.manage(ffmpeg_handler);

            app.manage(settings_manager);

            let theme_manager = ThemeManager::new(app_data_dir.clone());
            // Sync project themes once on startup
            theme_manager.sync_project_themes(Some(&app.handle()));
            theme_manager.start_watcher(app.handle().clone());
            app.manage(theme_manager);

            // Register FFmpegDownloader as managed state
            let ffmpeg_downloader = FFmpegDownloaderState(Arc::new(Mutex::new(FFmpegDownloader::new())));
            app.manage(ffmpeg_downloader);

            log::info!("SpiritStream initialized. Data dir: {app_data_dir:?}");

            // Set window icon
            if let Some(window) = app.get_webview_window("main") {
                let icon_bytes = include_bytes!("../icons/icon.png").to_vec();
                match Image::from_bytes(&icon_bytes) {
                    Ok(icon) => {
                        if let Err(e) = window.set_icon(icon) {
                            log::warn!("Failed to set window icon: {e}");
                        }
                    }
                    Err(e) => {
                        log::warn!("Failed to load icon: {e}");
                    }
                }
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Profile commands
            commands::get_all_profiles,
            commands::get_profile_summaries,
            commands::load_profile,
            commands::save_profile,
            commands::delete_profile,
            commands::is_profile_encrypted,
            commands::validate_input,
            // Stream commands
            commands::start_stream,
            commands::stop_stream,
            commands::stop_all_streams,
            commands::get_active_stream_count,
            commands::is_group_streaming,
            commands::get_active_group_ids,
            commands::toggle_stream_target,
            commands::is_target_disabled,
            // System commands
            commands::get_encoders,
            commands::test_ffmpeg,
            commands::validate_ffmpeg_path,
            commands::get_recent_logs,
            commands::export_logs,
            // Settings commands
            commands::get_settings,
            commands::save_settings,
            commands::get_profiles_path,
            commands::export_data,
            commands::clear_data,
            commands::rotate_machine_key,
            // FFmpeg download commands
            commands::download_ffmpeg,
            commands::cancel_ffmpeg_download,
            commands::get_bundled_ffmpeg_path,
            commands::check_ffmpeg_update,
            // Theme commands
            commands::list_themes,
            commands::refresh_themes,
            commands::get_theme_tokens,
            commands::install_theme,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            // Clean up FFmpeg processes on app exit
            if let RunEvent::Exit = event {
                log::info!("Application exiting, stopping all FFmpeg processes...");
                if let Some(handler) = app_handle.try_state::<FFmpegHandler>() {
                    if let Err(e) = handler.stop_all() {
                        log::error!("Failed to stop FFmpeg processes: {e}");
                    } else {
                        log::info!("All FFmpeg processes stopped successfully");
                    }
                }
            }
        });
}
