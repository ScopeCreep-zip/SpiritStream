// MagillaStream - Tauri Backend
// Multi-Destination Streaming Application

mod commands;
mod models;
mod services;

use std::sync::Arc;
use tauri::Manager;
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

            // Register FFmpegHandler as managed state
            let ffmpeg_handler = FFmpegHandler::new();
            app.manage(ffmpeg_handler);

            // Register SettingsManager as managed state
            let settings_manager = SettingsManager::new(app_data_dir.clone());
            app.manage(settings_manager);

            // Register FFmpegDownloader as managed state
            let ffmpeg_downloader = FFmpegDownloaderState(Arc::new(Mutex::new(FFmpegDownloader::new())));
            app.manage(ffmpeg_downloader);

            log::info!("MagillaStream initialized. Data dir: {:?}", app_data_dir);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::greet,
            // Profile commands
            commands::get_all_profiles,
            commands::load_profile,
            commands::save_profile,
            commands::delete_profile,
            commands::is_profile_encrypted,
            commands::create_profile,
            commands::create_stream_target,
            // Stream commands
            commands::start_stream,
            commands::stop_stream,
            commands::stop_all_streams,
            commands::get_active_stream_count,
            commands::is_group_streaming,
            commands::start_stream_simple,
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
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
