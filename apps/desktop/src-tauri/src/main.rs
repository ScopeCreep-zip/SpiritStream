#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
// SpiritStream Desktop - Minimal Tauri wrapper
// Spawns the backend server and displays the UI

use serde::{Deserialize, Serialize};
use std::{env, fs, path::PathBuf, sync::Mutex, time::Duration};
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

/// Migrate user data from legacy locations to the new Tauri data directory.
/// This is a safety net in case the installer migration doesn't run (portable installs, dev builds).
/// Legacy locations checked (in order):
///   1. %APPDATA%\SpiritStream\
///   2. %APPDATA%\spirit-stream\
///   3. %LOCALAPPDATA%\SpiritStream\
fn migrate_legacy_data(new_data_dir: &PathBuf) {
    // Skip if new location already has profiles
    let new_profiles_dir = new_data_dir.join("profiles");
    if new_profiles_dir.exists()
        && fs::read_dir(&new_profiles_dir)
            .map(|mut entries| entries.next().is_some())
            .unwrap_or(false)
    {
        log::debug!("New data directory already has profiles, skipping migration");
        return;
    }

    // Skip if already migrated (marker file exists)
    let migration_marker = new_data_dir.join(".migrated_from_legacy");
    if migration_marker.exists() {
        log::debug!("Migration marker exists, skipping migration");
        return;
    }

    // Get base directories
    let roaming_appdata = dirs_next::data_dir(); // %APPDATA%
    let local_appdata = dirs_next::data_local_dir(); // %LOCALAPPDATA%

    // Check legacy locations in order of preference
    let legacy_locations: Vec<PathBuf> = [
        roaming_appdata.as_ref().map(|p| p.join("SpiritStream")),
        roaming_appdata.as_ref().map(|p| p.join("spirit-stream")),
        local_appdata.as_ref().map(|p| p.join("SpiritStream")),
    ]
    .into_iter()
    .flatten()
    .collect();

    let mut legacy_source: Option<PathBuf> = None;
    for location in &legacy_locations {
        let profiles_dir = location.join("profiles");
        if profiles_dir.exists()
            && fs::read_dir(&profiles_dir)
                .map(|mut entries| entries.next().is_some())
                .unwrap_or(false)
        {
            legacy_source = Some(location.clone());
            break;
        }
    }

    let Some(source) = legacy_source else {
        log::debug!("No legacy data found to migrate");
        return;
    };

    log::info!("Found legacy data at {:?}, migrating to {:?}", source, new_data_dir);

    // Create target directories
    fs::create_dir_all(&new_profiles_dir).ok();
    fs::create_dir_all(new_data_dir.join("themes")).ok();
    fs::create_dir_all(new_data_dir.join("logs")).ok();
    fs::create_dir_all(new_data_dir.join("indexes")).ok();

    // Copy profiles (critical user data)
    if let Ok(entries) = fs::read_dir(source.join("profiles")) {
        for entry in entries.flatten() {
            let target = new_profiles_dir.join(entry.file_name());
            if let Err(e) = fs::copy(entry.path(), &target) {
                log::warn!("Failed to migrate profile {:?}: {}", entry.file_name(), e);
            }
        }
        log::info!("Migrated profiles directory");
    }

    // Copy settings.json
    let settings_src = source.join("settings.json");
    if settings_src.exists() {
        let target = new_data_dir.join("settings.json");
        if let Err(e) = fs::copy(&settings_src, &target) {
            log::warn!("Failed to migrate settings.json: {}", e);
        } else {
            log::info!("Migrated settings.json");
        }
    }

    // Copy machine encryption key (critical for encrypted profiles)
    let key_src = source.join(".stream_key");
    if key_src.exists() {
        let target = new_data_dir.join(".stream_key");
        if let Err(e) = fs::copy(&key_src, &target) {
            log::warn!("Failed to migrate .stream_key: {}", e);
        } else {
            log::info!("Migrated encryption key");
        }
    }

    // Copy custom themes
    let themes_src = source.join("themes");
    if themes_src.exists() {
        if let Ok(entries) = fs::read_dir(&themes_src) {
            for entry in entries.flatten() {
                let target = new_data_dir.join("themes").join(entry.file_name());
                if let Err(e) = fs::copy(entry.path(), &target) {
                    log::warn!("Failed to migrate theme {:?}: {}", entry.file_name(), e);
                }
            }
            log::info!("Migrated themes directory");
        }
    }

    // Copy profile order indexes
    let indexes_src = source.join("indexes");
    if indexes_src.exists() {
        if let Ok(entries) = fs::read_dir(&indexes_src) {
            for entry in entries.flatten() {
                let target = new_data_dir.join("indexes").join(entry.file_name());
                if let Err(e) = fs::copy(entry.path(), &target) {
                    log::warn!("Failed to migrate index {:?}: {}", entry.file_name(), e);
                }
            }
            log::info!("Migrated indexes directory");
        }
    }

    // Create migration marker
    let marker_content = format!(
        "Migrated from: {:?}\nMigration date: {}\n",
        source,
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
    );
    if let Err(e) = fs::write(&migration_marker, marker_content) {
        log::warn!("Failed to write migration marker: {}", e);
    }

    log::info!("Legacy data migration completed successfully");
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_http::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(ServerProcess(Mutex::new(None)))
        .invoke_handler(tauri::generate_handler![])
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

    // Use Local AppData instead of Roaming AppData for all user data
    // This keeps everything in one machine-specific location that doesn't sync
    // For a streaming app, settings/profiles don't need to roam across domain machines
    let app_data_dir = app
        .path()
        .app_local_data_dir()
        .map_err(|e| format!("Failed to resolve app local data dir: {e}"))?;

    // Migrate legacy data from old locations (safety net for portable/dev installs)
    migrate_legacy_data(&app_data_dir);

    // Put logs in a subdirectory of the local data dir
    let log_dir = app_data_dir.join("logs");

    // Ensure directories exist before spawning server
    std::fs::create_dir_all(&app_data_dir).ok();
    std::fs::create_dir_all(&log_dir).ok();

    // Helper function to check if directory has theme files
    fn has_theme_files(dir: &std::path::Path) -> bool {
        std::fs::read_dir(dir)
            .map(|entries| {
                entries.flatten().any(|entry| {
                    entry
                        .path()
                        .extension()
                        .map(|ext| ext == "jsonc" || ext == "json")
                        .unwrap_or(false)
                })
            })
            .unwrap_or(false)
    }

    // Helper to count theme files (for logging)
    fn count_theme_files(dir: &std::path::Path) -> usize {
        std::fs::read_dir(dir)
            .map(|entries| {
                entries
                    .flatten()
                    .filter(|entry| {
                        entry
                            .path()
                            .extension()
                            .map(|ext| ext == "jsonc" || ext == "json")
                            .unwrap_or(false)
                    })
                    .count()
            })
            .unwrap_or(0)
    }

    // Find themes directory - check bundled resources first, then dev paths
    let themes_dir = {
        let bundled = app.path().resource_dir().ok().map(|dir| dir.join("themes"));

        // Log detailed info about bundled path for debugging production issues
        if let Some(ref path) = bundled {
            log::info!("Checking bundled themes at: {:?}", path);
            log::info!(
                "  exists: {}, is_dir: {}",
                path.exists(),
                path.is_dir()
            );

            // List contents if exists
            if path.exists() {
                let count = count_theme_files(path);
                log::info!("  theme files found: {}", count);

                if let Ok(entries) = std::fs::read_dir(path) {
                    let files: Vec<_> = entries
                        .flatten()
                        .map(|e| e.file_name().to_string_lossy().to_string())
                        .collect();
                    log::info!("  contents: {:?}", files);
                }
            }
        }

        // Use bundled if it exists AND has theme files
        if bundled
            .as_ref()
            .map(|p| p.exists() && has_theme_files(p))
            .unwrap_or(false)
        {
            log::info!("Using bundled themes directory");
            bundled
        } else {
            // Development fallback chain
            let cwd = std::env::current_dir().ok();

            // Option 1: themes/ in CWD (running from project root)
            let cwd_themes = cwd.as_ref().map(|d| d.join("themes"));
            if cwd_themes
                .as_ref()
                .map(|p| has_theme_files(p))
                .unwrap_or(false)
            {
                log::info!("Using CWD themes directory: {:?}", cwd_themes);
                cwd_themes
            } else {
                // Option 2: ../../themes from apps/desktop
                let parent_themes = cwd
                    .as_ref()
                    .and_then(|d| d.join("../../themes").canonicalize().ok());
                if parent_themes
                    .as_ref()
                    .map(|p| has_theme_files(p))
                    .unwrap_or(false)
                {
                    log::info!("Using parent themes directory: {:?}", parent_themes);
                    parent_themes
                } else {
                    // Option 3: ../../../themes from apps/desktop/src-tauri
                    let grandparent_themes = cwd
                        .as_ref()
                        .and_then(|d| d.join("../../../themes").canonicalize().ok());
                    if grandparent_themes
                        .as_ref()
                        .map(|p| has_theme_files(p))
                        .unwrap_or(false)
                    {
                        log::info!(
                            "Using grandparent themes directory: {:?}",
                            grandparent_themes
                        );
                        grandparent_themes
                    } else {
                        log::warn!("No themes directory found with theme files!");
                        log::warn!("  Tried bundled: {:?}", bundled);
                        log::warn!("  Tried CWD: {:?}", cwd_themes);
                        log::warn!("  Tried parent: {:?}", parent_themes);
                        log::warn!("  Tried grandparent: {:?}", grandparent_themes);
                        // Return bundled path anyway - server will handle missing
                        bundled
                    }
                }
            }
        }
    };

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

    let (mut rx, child) = command.spawn().map_err(|e| {
        log::error!("Failed to spawn server: {e}");
        format!("Failed to spawn server: {e}")
    })?;

    // Store the child process handle so we can kill it on exit
    if let Some(server_state) = app.try_state::<ServerProcess>() {
        if let Ok(mut guard) = server_state.0.lock() {
            *guard = Some(child);
        }
    }

    let app_handle = app.clone();
    tauri::async_runtime::spawn(async move {
        let mut startup_errors: Vec<String> = Vec::new();
        let mut terminated_early = false;

        while let Some(event) = rx.recv().await {
            match event {
                CommandEvent::Stdout(line) => {
                    let msg = String::from_utf8_lossy(&line);
                    log::info!("[server] {}", msg);
                }
                CommandEvent::Stderr(line) => {
                    let msg = String::from_utf8_lossy(&line).to_string();
                    log::warn!("[server] {}", msg);
                    // Capture stderr for potential error reporting
                    startup_errors.push(msg);
                }
                CommandEvent::Error(error) => {
                    log::error!("[server] {error}");
                    startup_errors.push(error.clone());
                }
                CommandEvent::Terminated(payload) => {
                    log::error!(
                        "[server] terminated unexpectedly (code: {:?}, signal: {:?})",
                        payload.code,
                        payload.signal
                    );
                    terminated_early = true;

                    // Emit event to frontend with error details
                    let error_msg = if startup_errors.is_empty() {
                        format!(
                            "Server process terminated with code {:?}",
                            payload.code.unwrap_or(-1)
                        )
                    } else {
                        startup_errors.join("\n")
                    };

                    let _ = app_handle.emit("server-error", &error_msg);
                    log::error!("Server startup failed: {}", error_msg);
                    break;
                }
                _ => {}
            }
        }

        if terminated_early {
            log::error!("Server terminated during startup - check logs for details");
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
        // Give processes time to terminate and release the port
        std::thread::sleep(Duration::from_millis(1000));
        log::info!("Killed any existing spiritstream-server processes");
    }
    #[cfg(windows)]
    {
        use std::process::Command;
        // Try taskkill first
        let result = Command::new("taskkill")
            .args(["/F", "/IM", "spiritstream-server.exe"])
            .output();

        if let Ok(output) = &result {
            if output.status.success() {
                log::info!("Taskkill succeeded for spiritstream-server.exe");
            }
        }

        // Wait for process to fully terminate and release the port
        std::thread::sleep(Duration::from_millis(1500));

        // Verify port 8008 is free, if not wait a bit more
        for attempt in 1..=3 {
            if is_port_available(8008) {
                log::info!("Port 8008 is available after {} attempt(s)", attempt);
                break;
            }
            log::warn!("Port 8008 still in use, waiting... (attempt {})", attempt);
            std::thread::sleep(Duration::from_millis(1000));
        }

        log::info!("Killed any existing spiritstream-server processes");
    }
}

/// Check if a port is available for binding
#[cfg(windows)]
fn is_port_available(port: u16) -> bool {
    use std::net::TcpListener;
    TcpListener::bind(format!("127.0.0.1:{}", port)).is_ok()
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
