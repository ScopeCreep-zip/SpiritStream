// SpiritStream - Tauri Backend
// Multi-Destination Streaming Application

mod commands;
mod models;
mod services;

use std::sync::Arc;
use tauri::{Manager, RunEvent};
use tokio::sync::Mutex;
use services::{ProfileManager, FFmpegHandler, FFmpegDownloader, SettingsManager};
use commands::FFmpegDownloaderState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            // Initialize logging in debug mode
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }

            // Get app data directory for profile storage
            let app_data_dir = app.path().app_data_dir().expect("Failed to get app data dir");
            std::fs::create_dir_all(&app_data_dir).ok();

            // Register ProfileManager as managed state
            let profile_manager = ProfileManager::new(app_data_dir.clone());
            app.manage(profile_manager);

            // Register SettingsManager as managed state (load early to get custom FFmpeg path)
            let settings_manager = SettingsManager::new(app_data_dir.clone());

            // Load settings to get custom ffmpeg_path if configured
            let custom_ffmpeg_path = settings_manager.load()
                .ok()
                .and_then(|s| if s.ffmpeg_path.is_empty() { None } else { Some(s.ffmpeg_path) });

            // Register FFmpegHandler with custom path from settings (falls back to auto-discovery)
            let ffmpeg_handler = FFmpegHandler::new_with_custom_path(app_data_dir.clone(), custom_ffmpeg_path);
            app.manage(ffmpeg_handler);

            app.manage(settings_manager);

            // Register FFmpegDownloader as managed state
            let ffmpeg_downloader = FFmpegDownloaderState(Arc::new(Mutex::new(FFmpegDownloader::new())));
            app.manage(ffmpeg_downloader);

            log::info!("SpiritStream initialized. Data dir: {:?}", app_data_dir);

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
            // System commands
            commands::get_encoders,
            commands::test_ffmpeg,
            // Settings commands
            commands::get_settings,
            commands::save_settings,
            commands::get_profiles_path,
            commands::export_data,
            commands::clear_data,
            // FFmpeg download commands
            commands::download_ffmpeg,
            commands::cancel_ffmpeg_download,
            commands::get_bundled_ffmpeg_path,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            // Clean up FFmpeg processes on app exit
            if let RunEvent::Exit = event {
                log::info!("Application exiting, stopping all FFmpeg processes...");
                if let Some(handler) = app_handle.try_state::<FFmpegHandler>() {
                    if let Err(e) = handler.stop_all() {
                        log::error!("Failed to stop FFmpeg processes: {}", e);
                    } else {
                        log::info!("All FFmpeg processes stopped successfully");
                    }
                }
            }
        });
}
