// SpiritStream Desktop - Minimal Tauri wrapper
// Spawns the backend server and displays the UI

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{env, time::Duration, path::PathBuf};
use serde::{Deserialize, Serialize};
use tauri::{image::Image, Manager, RunEvent, Runtime, AppHandle};
use tauri_plugin_log::{Target, TargetKind};
use tauri_plugin_shell::{process::CommandEvent, ShellExt};

const DEFAULT_HOST: &str = "127.0.0.1";
const DEFAULT_PORT: &str = "8008";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct Settings {
    #[serde(default)]
    start_minimized: bool,
    #[serde(default)]
    backend_remote_enabled: bool,
    #[serde(default)]
    backend_host: String,
    #[serde(default)]
    backend_port: u16,
    #[serde(default)]
    backend_token: String,
}

fn main() {
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

            // Load settings early to check start_minimized
            let settings = load_settings(app.handle()).unwrap_or_default();

            // Set window icon and handle start minimized
            if let Some(window) = app.get_webview_window("main") {
                let icon_bytes = include_bytes!("../icons/icon.png").to_vec();
                if let Ok(icon) = Image::from_bytes(&icon_bytes) {
                    if let Err(e) = window.set_icon(icon) {
                        log::warn!("Failed to set window icon: {e}");
                    }
                }

                // Start minimized if setting is enabled
                if settings.start_minimized {
                    if let Err(e) = window.minimize() {
                        log::warn!("Failed to minimize window: {e}");
                    }
                    log::info!("Starting minimized per user settings");
                }
            }

            // Launch the backend server
            launch(app.handle());

            log::info!("SpiritStream Desktop initialized");

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_app_handle, event| {
            if let RunEvent::Exit = event {
                log::info!("SpiritStream Desktop exiting");
            }
        });
}

fn launch<R: Runtime>(app: &AppHandle<R>) {
    let app_handle = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(error) = run_launcher(&app_handle).await {
            log::error!("Launcher failed: {error}");
        }
    });
}

async fn run_launcher<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    let settings = load_settings(app).unwrap_or_default();

    let settings_host = if settings.backend_remote_enabled && !settings.backend_host.trim().is_empty() {
        settings.backend_host.clone()
    } else {
        DEFAULT_HOST.to_string()
    };

    let settings_port = if settings.backend_port == 0 {
        DEFAULT_PORT.to_string()
    } else {
        settings.backend_port.to_string()
    };

    let settings_token = if settings.backend_token.trim().is_empty() {
        None
    } else {
        Some(settings.backend_token.clone())
    };

    let host = env::var("SPIRITSTREAM_HOST").unwrap_or(settings_host);
    let port = env::var("SPIRITSTREAM_PORT").unwrap_or(settings_port);

    spawn_server(app, &host, &port, settings_token.as_deref())?;

    wait_for_health(&host, &port).await;

    Ok(())
}

fn spawn_server<R: Runtime>(
    app: &AppHandle<R>,
    host: &str,
    port: &str,
    auth_token: Option<&str>,
) -> Result<(), String> {
    let mut command = if let Ok(server_path) = env::var("SPIRITSTREAM_SERVER_PATH") {
        app.shell().command(server_path)
    } else {
        app.shell()
            .sidecar("spiritstream-server")
            .map_err(|e| e.to_string())?
    };

    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to resolve app data dir: {e}"))?;

    let log_dir = app
        .path()
        .app_log_dir()
        .map_err(|e| format!("Failed to resolve log dir: {e}"))?;

    let themes_dir = app
        .path()
        .resource_dir()
        .ok()
        .map(|dir| dir.join("themes"))
        .filter(|dir| dir.exists())
        .unwrap_or_else(|| PathBuf::from("../../../themes"));

    if env::var("SPIRITSTREAM_DATA_DIR").is_err() {
        command = command.env("SPIRITSTREAM_DATA_DIR", app_data_dir);
    }
    if env::var("SPIRITSTREAM_LOG_DIR").is_err() {
        command = command.env("SPIRITSTREAM_LOG_DIR", log_dir);
    }
    if env::var("SPIRITSTREAM_THEMES_DIR").is_err() {
        command = command.env("SPIRITSTREAM_THEMES_DIR", themes_dir);
    }
    if env::var("SPIRITSTREAM_HOST").is_err() {
        command = command.env("SPIRITSTREAM_HOST", host);
    }
    if env::var("SPIRITSTREAM_PORT").is_err() {
        command = command.env("SPIRITSTREAM_PORT", port);
    }
    if env::var("SPIRITSTREAM_API_TOKEN").is_err() && env::var("SPIRITSTREAM_DEV_TOKEN").is_err() {
        if let Some(token) = auth_token {
            command = command.env("SPIRITSTREAM_API_TOKEN", token);
        }
    }

    if env::var("SPIRITSTREAM_UI_DIR").is_err() {
        if let Ok(resource_dir) = app.path().resource_dir() {
            let dist_dir = resource_dir.join("dist");
            if dist_dir.exists() {
                command = command.env("SPIRITSTREAM_UI_DIR", dist_dir);
            }
        }
    }

    let (mut rx, _child) = command.spawn().map_err(|e| e.to_string())?;

    tauri::async_runtime::spawn(async move {
        while let Some(event) = rx.recv().await {
            match event {
                CommandEvent::Stdout(line) => {
                    log::info!("[server] {}", String::from_utf8_lossy(&line));
                }
                CommandEvent::Stderr(line) => {
                    log::warn!("[server] {}", String::from_utf8_lossy(&line));
                }
                CommandEvent::Error(error) => {
                    log::error!("[server] {error}");
                }
                CommandEvent::Terminated(payload) => {
                    log::warn!(
                        "[server] terminated (code: {:?}, signal: {:?})",
                        payload.code,
                        payload.signal
                    );
                    break;
                }
                _ => {}
            }
        }
    });

    Ok(())
}

fn load_settings<R: Runtime>(app: &AppHandle<R>) -> Option<Settings> {
    let app_data_dir = app.path().app_data_dir().ok()?;
    let settings_path = app_data_dir.join("settings.json");

    if !settings_path.exists() {
        return None;
    }

    let content = std::fs::read_to_string(&settings_path).ok()?;
    serde_json::from_str(&content).ok()
}

async fn wait_for_health(host: &str, port: &str) {
    let url = format!("http://{host}:{port}/health");

    for _ in 0..25 {
        if let Ok(response) = reqwest::get(&url).await {
            if response.status().is_success() {
                log::info!("Backend server is healthy at {url}");
                return;
            }
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }

    log::warn!("Launcher could not confirm backend health at {url}");
}
