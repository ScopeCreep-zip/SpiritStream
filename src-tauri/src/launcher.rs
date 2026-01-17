use std::{
    env,
    path::PathBuf,
    time::Duration,
};

use reqwest::Client;
use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_shell::{process::CommandEvent, ShellExt};
use crate::services::SettingsManager;

const DEFAULT_HOST: &str = "127.0.0.1";
const DEFAULT_PORT: &str = "8008";
const DEFAULT_UI_URL_DEV: &str = "http://localhost:1420";

pub fn launch<R: Runtime>(app: &AppHandle<R>) {
    let app_handle = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(error) = run_launcher(&app_handle).await {
            log::error!("Launcher failed: {error}");
        }
    });
}

async fn run_launcher<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    let settings = load_settings(app).unwrap_or_default();
    let settings_host = if settings.backend_remote_enabled {
        if settings.backend_host.trim().is_empty() {
            DEFAULT_HOST.to_string()
        } else {
            settings.backend_host.clone()
        }
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

    if env::var("SPIRITSTREAM_LAUNCHER_OPEN_EXTERNAL").ok().as_deref() == Some("1") {
        let ui_url = resolve_ui_url(&host, &port, app);
        open_ui(app, &ui_url)?;
    }

    if env::var("SPIRITSTREAM_LAUNCHER_HIDE_WINDOW").ok().as_deref() == Some("1") {
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.hide();
        }
    }

    Ok(())
}

fn resolve_ui_url<R: Runtime>(host: &str, port: &str, app: &AppHandle<R>) -> String {
    if let Ok(url) = env::var("SPIRITSTREAM_UI_URL") {
        return url;
    }

    if cfg!(debug_assertions) {
        return DEFAULT_UI_URL_DEV.to_string();
    }

    if let Ok(resource_dir) = app.path().resource_dir() {
        let dist_dir = resource_dir.join("dist");
        if dist_dir.exists() {
            return format!("http://{host}:{port}");
        }
    }

    format!("http://{host}:{port}")
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
            .sidecar("server")
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
        .unwrap_or_else(|| PathBuf::from("../themes"));

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
                    log::info!("[launcher:server] {}", String::from_utf8_lossy(&line));
                }
                CommandEvent::Stderr(line) => {
                    log::warn!("[launcher:server] {}", String::from_utf8_lossy(&line));
                }
                CommandEvent::Error(error) => {
                    log::error!("[launcher:server] {error}");
                }
                CommandEvent::Terminated(payload) => {
                    log::warn!(
                        "[launcher:server] terminated (code: {:?}, signal: {:?})",
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

fn load_settings<R: Runtime>(app: &AppHandle<R>) -> Option<crate::models::Settings> {
    let app_data_dir = app.path().app_data_dir().ok()?;
    let settings_manager = SettingsManager::new(app_data_dir);
    settings_manager.load().ok()
}

async fn wait_for_health(host: &str, port: &str) {
    let client = Client::new();
    let url = format!("http://{host}:{port}/health");

    for _ in 0..25 {
        if let Ok(response) = client.get(&url).send().await {
            if response.status().is_success() {
                return;
            }
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }

    log::warn!("Launcher could not confirm backend health at {url}");
}

fn open_ui<R: Runtime>(app: &AppHandle<R>, url: &str) -> Result<(), String> {
    #[allow(deprecated)]
    {
        app.shell()
            .open(url.to_string(), None)
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}
