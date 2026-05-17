#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
// SpiritStream Desktop - Minimal Tauri wrapper
// Spawns the backend server and displays the UI

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

/// Slim shadow of `crates/core/src/models/Settings` — only the fields
/// the Tauri shell itself needs at launch time. Backend host/port/token
/// live in `ProfileSettings.backend` and are resolved server-side by
/// `crates/transport-http`; the shell never injects them as env vars
/// (it used to, which silently masked the profile-based resolution).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct Settings {
    #[serde(default)]
    start_minimized: bool,
}

/// Whether the running build supports the self-updater. Returns:
/// - `true` on macOS, Windows, and Linux AppImage installs.
/// - `false` on Linux `.deb` / `.rpm` installs (system package manager
///   owns updates — running our updater there fights the distro and
///   leaves files orphaned in `/usr/...` outside `apt`/`dnf`'s tracking).
///
/// AppImage detection is per appimage.org's runtime contract: the
/// AppImage runtime sets `APPIMAGE` to the mounted .AppImage's absolute
/// path before launching the embedded binary. `.deb`/`.rpm` users never
/// see this env var, so `is_some()` cleanly separates the two install
/// types without heuristics on `argv[0]` paths.
///
/// docs: https://docs.appimage.org/packaging-guide/environment-variables.html
#[tauri::command]
fn updater_supported() -> bool {
    #[cfg(target_os = "linux")]
    {
        std::env::var_os("APPIMAGE").is_some()
    }
    #[cfg(not(target_os = "linux"))]
    {
        true
    }
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_http::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        // App self-update via GitHub Releases. The plugin
        // verifies each `.sig` file (locally signed by the maintainer)
        // against the `pubkey` embedded in `tauri.conf.json`. Linux
        // .deb/.rpm installs opt out at the UI layer via the
        // `updater_supported` command below; the plugin itself is
        // always registered so any platform can call into it if
        // present.
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        // Global system-wide hotkeys (panic shortcut). The frontend
        // registers / unregisters bindings via the JS side of the
        // plugin; this initializer just exposes the OS hook so the
        // shortcut fires even when SpiritStream isn't focused.
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .manage(ServerProcess(Mutex::new(None)))
        .invoke_handler(tauri::generate_handler![updater_supported])
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

            // No window declared in `tauri.conf.json` — we build it
            // programmatically below, AFTER the backend's TCP socket is
            // accepting connections. This is the only architecture in
            // Tauri 2 that prevents the React bundle from executing
            // before the backend exists: `visible: false` doesn't stop
            // JS execution (per Tauri issues #5583, #7669, #10950), and
            // there is no `navigate_later` API. The webview must not
            // exist at all until the gate passes.
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

    // Bind args (host/port/token) are resolved server-side from the
    // active profile's `settings.backend` (`crates/transport-http/src/lib.rs`
    // ~2403). The shell never invents host/port/token defaults; the
    // server owns that decision exclusively. Operator overrides via
    // `SPIRITSTREAM_HOST` / `SPIRITSTREAM_PORT` / `SPIRITSTREAM_API_TOKEN`
    // in the launcher's parent environment are inherited by the
    // spawned child automatically (process-default env-var inheritance).
    //
    // The webview always loads from `127.0.0.1:8008` regardless of
    // server bind: the CSP allow-list in `tauri.conf.json` hardcodes
    // that origin, and the server always answers on localhost too —
    // toggling "Allow remote web access" widens the bind from
    // `127.0.0.1:8008` to `0.0.0.0:8008`, never moves the port.
    let host = DEFAULT_HOST;
    let port = DEFAULT_PORT;

    // Kill any zombie server processes from previous runs to avoid port conflicts
    kill_existing_servers();

    spawn_server(app)?;

    // Wait until the server's TCP socket is accepting connections. The
    // webview is NOT created yet — the React bundle is not running and
    // cannot fire any HTTP requests. Once this returns, we build the
    // window programmatically; React mounts knowing the backend is
    // reachable, and `/api/v1/ready` either returns 200 immediately
    // (fast path) or long-polls until services finish initializing.
    //
    // This ordering is the entire reason there are no console errors
    // during boot: nothing in the webview exists to make a failed
    // request. Tauri's `visible: false` does not provide this guarantee
    // (issues #5583 / #7669 / #10950) — only deferred window creation
    // does.
    wait_for_tcp_listening(host, port).await;

    // Build the main webview window. `WebviewUrl::default()` resolves
    // to `App("index.html".into())`, which Tauri swaps to the dev URL
    // (`http://localhost:1420`) in `tauri dev` and to the bundled SPA
    // in release. Same call site, both modes.
    let window = match tauri::WebviewWindowBuilder::new(
        app,
        "main",
        tauri::WebviewUrl::default(),
    )
    .title("SpiritStream")
    .inner_size(1500.0, 1000.0)
    .min_inner_size(1024.0, 600.0)
    .resizable(true)
    .center()
    .background_color(tauri::utils::config::Color(0x0F, 0x0A, 0x14, 0xFF))
    .build()
    {
        Ok(w) => w,
        Err(e) => return Err(format!("failed to create main window: {e}")),
    };

    // Apply the embedded icon. Failure is non-fatal — the platform
    // default icon is acceptable.
    let icon_bytes = include_bytes!("../icons/icon.png").to_vec();
    if let Ok(icon) = Image::from_bytes(&icon_bytes) {
        if let Err(e) = window.set_icon(icon) {
            log::warn!("Failed to set window icon: {e}");
        }
    }

    if settings.start_minimized {
        if let Err(e) = window.minimize() {
            log::warn!("Failed to minimize window: {e}");
        }
        log::info!("Window minimized per user settings");
    }

    log::info!("Main window created — backend was reachable when bundle loaded");
    Ok(())
}

fn spawn_server<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
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
            log::info!("  exists: {}, is_dir: {}", path.exists(), path.is_dir());

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
                // Option 2: ../../themes from apps/tauri (Tauri dev launch)
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
                    // Option 3: ../../../themes from apps/tauri/src-tauri (cargo run -p spiritstream-desktop)
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
    // SPIRITSTREAM_HOST / SPIRITSTREAM_PORT / SPIRITSTREAM_API_TOKEN
    // are deliberately NOT set here. The server resolves bind args
    // from the active profile's `settings.backend` (`crates/transport-http/src/lib.rs`
    // ~2403) — injecting env-var defaults from the shell would silently
    // override that resolution and was the root cause of remote-access
    // being silently broken pre-fix. Operator overrides in the parent
    // launcher's environment still propagate via standard env-var
    // inheritance to the spawned child.

    if env::var("SPIRITSTREAM_UI_DIR").is_err() {
        if let Ok(resource_dir) = app.path().resource_dir() {
            let dist_dir = resource_dir.join("dist");
            if dist_dir.exists() {
                command = command.env("SPIRITSTREAM_UI_DIR", dist_dir);
            }
        }
    }

    // Tauri 2 sidecar FFmpeg (Option A — no runtime download).
    //
    // On macOS / Windows the build pipeline fetches FFmpeg from the
    // ffmpeg.org-recommended source (evermeet.cx / BtbN) and places it
    // at `binaries/ffmpeg-<TARGET>(.exe)`. The bundler copies that into
    // the .app / install dir alongside `spiritstream-server`. We
    // resolve the path here and inject `SPIRITSTREAM_FFMPEG_PATH` so
    // the server picks it up without doing its own discovery.
    //
    // On Linux the placeholder file is empty (~0 bytes) — the .deb /
    // .rpm dependency on `ffmpeg` is what actually delivers the
    // binary, and the server falls through to `$PATH` lookup. We
    // detect the placeholder by file size and skip the env injection.
    if env::var("SPIRITSTREAM_FFMPEG_PATH").is_err() {
        // Tauri 2's `app.shell().sidecar(name)` resolves the sidecar
        // path internally during spawn but doesn't expose it. We need
        // the resolved path explicitly so the server (spawned as a
        // separate process) can pick it up via the env var. So we
        // recompute the same path Tauri would use:
        //
        // - **prod**: bundled next to the main executable, with the
        //   target-triple suffix stripped during packaging
        //   (`<bundle>/Contents/MacOS/ffmpeg`, etc.).
        // - **dev**: `apps/tauri/src-tauri/binaries/ffmpeg-<TARGET>`
        //   relative to the workspace. `BUILD_TARGET` is set in
        //   `build.rs` from Cargo's `TARGET` env var.
        //
        // Linux distro packaging leaves a zero-byte placeholder (the
        // .deb/.rpm dep on `ffmpeg` delivers the real binary), which
        // we detect by file size and skip — the server then falls
        // through to `$PATH` lookup.
        let resolved = resolve_ffmpeg_sidecar();
        match resolved {
            Some(path) => {
                log::info!("Injecting bundled FFmpeg sidecar path: {path:?}");
                command = command.env("SPIRITSTREAM_FFMPEG_PATH", &path);
            }
            None => {
                log::info!(
                    "No bundled FFmpeg sidecar resolved (placeholder, missing, or unknown target); \
                     server will look up `ffmpeg` on $PATH (Linux distro dep or `brew install`)."
                );
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

/// Resolve the FFmpeg sidecar path for both `tauri dev` and packaged builds.
/// Returns `None` when the resolved path is missing OR a zero-byte placeholder
/// (Linux `.deb` / `.rpm` ships placeholders because the distro `ffmpeg` package
/// supplies the real binary).
///
/// **Debug builds prefer the dev path first.** `tauri build` leaves a stale
/// `target/debug/ffmpeg` next to the dev exe when run on the same workspace
/// (the bundler renames sidecars in place during packaging, and `cargo clean`
/// is the only thing that removes them). If the dev resolver checked
/// `target/debug/ffmpeg` first, it would resolve to that stale copy — which
/// might be the wrong architecture, an outdated version, or otherwise
/// mismatched against the suffixed sidecar in `binaries/`. So in debug we
/// check the target-triple-suffixed sidecar first.
///
/// **Release builds prefer the bundled path** next to the exe — that's where
/// Tauri's installer lands the renamed sidecar. `BUILD_TARGET` is injected by
/// `build.rs` from Cargo's `TARGET` env var (still used for fallback in case
/// a release-build user manually places a suffixed binary).
fn resolve_ffmpeg_sidecar() -> Option<std::path::PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let exe_dir = exe.parent()?;

    let bundled_name = if cfg!(windows) { "ffmpeg.exe" } else { "ffmpeg" };
    let target_triple: &str = env!("BUILD_TARGET");
    let dev_name = if cfg!(windows) {
        format!("ffmpeg-{target_triple}.exe")
    } else {
        format!("ffmpeg-{target_triple}")
    };

    let workspace_root_candidate = exe_dir.parent().and_then(|p| p.parent());
    let dev_candidate = workspace_root_candidate.map(|root| {
        root.join("apps")
            .join("tauri")
            .join("src-tauri")
            .join("binaries")
            .join(&dev_name)
    });

    let prod_candidate = exe_dir.join(bundled_name);

    // Probe order: debug → dev-first then prod-fallback; release → prod-first
    // then dev-fallback. The `cfg!(debug_assertions)` branch is a runtime
    // check rather than a `#[cfg]` so the dev-mode fix applies to any build
    // that compiled with debug symbols, even when launched outside `cargo run`.
    let order: [&Option<std::path::PathBuf>; 2] = if cfg!(debug_assertions) {
        [&dev_candidate, &Some(prod_candidate.clone())]
    } else {
        [&Some(prod_candidate.clone()), &dev_candidate]
    };
    for candidate in order {
        let Some(path) = candidate.as_ref() else { continue };
        let Ok(meta) = std::fs::metadata(path) else { continue };
        if meta.is_file() && meta.len() > 0 {
            return Some(path.clone());
        }
    }
    None
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

/// Wait for the server's TCP socket to be accepting connections.
///
/// Fast (~10–200ms typical, since spawn_server only returns after the
/// child has been fork-execed and the server binds immediately on startup).
/// 5-second worst-case bound; on giveup, the webview will surface the
/// `/api/v1/ready` failure path itself via the React `unreachable` overlay.
///
/// This is the entire shell-side readiness coordination — services
/// initialization is observed *through the server*, not the shell, via the
/// long-poll on `/api/v1/ready`. Single source of readiness truth.
async fn wait_for_tcp_listening(host: &str, port: &str) {
    let addr = format!("{host}:{port}");
    for attempt in 0..50 {
        if tokio::net::TcpStream::connect(&addr).await.is_ok() {
            log::info!("Backend listening at {addr} (attempt {})", attempt + 1);
            return;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    log::warn!(
        "Backend did not begin listening at {addr} within 5s — \
         the webview's /ready long-poll will surface the failure to the user"
    );
}
