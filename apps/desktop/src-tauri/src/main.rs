// SpiritStream Desktop - Minimal Tauri wrapper
// Spawns the backend server and displays the UI

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::{Deserialize, Serialize};
use std::{env, sync::Mutex, time::Duration};
use tauri::{image::Image, AppHandle, Emitter, Manager, RunEvent, Runtime};
use tauri_plugin_log::{Target, TargetKind};
use tauri_plugin_shell::{
    process::{CommandChild, CommandEvent},
    ShellExt,
};

/// Holds the server child process so we can kill it on exit
struct ServerProcess(Mutex<Option<CommandChild>>);

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
        .plugin(tauri_plugin_http::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(ServerProcess(Mutex::new(None)))
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

            // Set window icon (window starts hidden, shown after server is ready)
            if let Some(window) = app.get_webview_window("main") {
                let icon_bytes = include_bytes!("../icons/icon.png").to_vec();
                if let Ok(icon) = Image::from_bytes(&icon_bytes) {
                    if let Err(e) = window.set_icon(icon) {
                        log::warn!("Failed to set window icon: {e}");
                    }
                }
            }

            // Launch the backend server
            launch(app.handle());

            log::info!("SpiritStream Desktop initialized");

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            if let RunEvent::Exit = event {
                log::info!("SpiritStream Desktop exiting");
                // Kill the server process on exit
                if let Some(server_state) = app_handle.try_state::<ServerProcess>() {
                    if let Ok(mut guard) = server_state.0.lock() {
                        if let Some(child) = guard.take() {
                            log::info!("Terminating backend server process");
                            if let Err(e) = child.kill() {
                                log::warn!("Failed to kill server process: {e}");
                            }
                        }
                    }
                }
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

    let settings_host =
        if settings.backend_remote_enabled && !settings.backend_host.trim().is_empty() {
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

    // Kill any zombie server processes from previous runs to avoid port conflicts
    kill_existing_servers();

    spawn_server(app, &host, &port, settings_token.as_deref())?;

    wait_for_health(&host, &port).await;

    // Emit event BEFORE showing window so frontend knows server is ready
    // This prevents race condition where React fails before window shows
    app.emit("server-ready", ()).ok();
    log::info!("Emitted server-ready event to frontend");

    // Show the main window now that the server is ready
    if let Some(window) = app.get_webview_window("main") {
        if let Err(e) = window.show() {
            log::warn!("Failed to show main window: {e}");
        } else {
            log::info!("Main window shown - server is ready");
        }

        // If user wants to start minimized, minimize after showing
        if settings.start_minimized {
            if let Err(e) = window.minimize() {
                log::warn!("Failed to minimize window: {e}");
            }
            log::info!("Window minimized per user settings");
        }
    }

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

    // Ensure directories exist before spawning server
    std::fs::create_dir_all(&app_data_dir).ok();
    std::fs::create_dir_all(&log_dir).ok();

    // Find themes directory - check bundled resources first, then dev path
    let themes_dir = app
        .path()
        .resource_dir()
        .ok()
        .map(|dir| dir.join("themes"))
        .filter(|dir| dir.exists())
        .or_else(|| {
            // In development, check relative to current working directory
            // (pnpm dev runs from project root or apps/desktop)
            let cwd = std::env::current_dir().ok()?;
            // Try project root first
            let root_themes = cwd.join("themes");
            if root_themes.exists() {
                return Some(root_themes);
            }
            // Try from apps/desktop
            let parent_themes = cwd.join("../../themes");
            parent_themes.canonicalize().ok().filter(|p| p.exists())
        });

    if env::var("SPIRITSTREAM_DATA_DIR").is_err() {
        command = command.env("SPIRITSTREAM_DATA_DIR", &app_data_dir);
    }
    if env::var("SPIRITSTREAM_LOG_DIR").is_err() {
        command = command.env("SPIRITSTREAM_LOG_DIR", &log_dir);
    }
    // Only set SPIRITSTREAM_THEMES_DIR if bundled themes exist
    if env::var("SPIRITSTREAM_THEMES_DIR").is_err() {
        if let Some(themes) = &themes_dir {
            command = command.env("SPIRITSTREAM_THEMES_DIR", themes);
        }
        // If themes don't exist, let server use its default handling
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

    let (mut rx, child) = command.spawn().map_err(|e| e.to_string())?;

    // Store the child process handle so we can kill it on exit
    if let Some(server_state) = app.try_state::<ServerProcess>() {
        if let Ok(mut guard) = server_state.0.lock() {
            *guard = Some(child);
        }
    }

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

/// Kill any existing spiritstream-server processes to avoid port conflicts
fn kill_existing_servers() {
    #[cfg(unix)]
    {
        use std::process::Command;
        // Kill any existing spiritstream-server processes
        let _ = Command::new("pkill")
            .args(["-f", "spiritstream-server"])
            .output();
        // Give processes time to terminate
        std::thread::sleep(Duration::from_millis(500));
        log::info!("Killed any existing spiritstream-server processes");
    }
    #[cfg(windows)]
    {
        use std::process::Command;
        let _ = Command::new("taskkill")
            .args(["/F", "/IM", "spiritstream-server.exe"])
            .output();
        std::thread::sleep(Duration::from_millis(500));
        log::info!("Killed any existing spiritstream-server processes");
    }
}

async fn wait_for_health(host: &str, port: &str) {
    let health_url = format!("http://{host}:{port}/health");
    let ready_url = format!("http://{host}:{port}/ready");

    // Phase 1: Wait for server to be alive (health check)
    for _ in 0..25 {
        if let Ok(response) = reqwest::get(&health_url).await {
            if response.status().is_success() {
                log::info!("Backend server is alive at {health_url}");
                break;
            }
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }

    // Phase 2: Wait for server to be ready (services initialized)
    for attempt in 1..=15 {
        if let Ok(response) = reqwest::get(&ready_url).await {
            if response.status().is_success() {
                if let Ok(data) = response.json::<serde_json::Value>().await {
                    if data.get("ready").and_then(|v| v.as_bool()).unwrap_or(false) {
                        log::info!("Backend server is ready");
                        return;
                    }
                }
            }
        }

        if attempt % 5 == 0 {
            log::info!("Waiting for backend readiness ({attempt}/15)...");
        }
        tokio::time::sleep(Duration::from_millis(300)).await;
    }

    log::warn!("Could not confirm backend readiness at {ready_url}");
}
