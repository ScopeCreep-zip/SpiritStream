use std::{env, sync::Mutex, time::Duration};
use tauri::{image::Image, AppHandle, Emitter, Manager, Runtime};
use tauri_plugin_shell::{
    process::{CommandChild, CommandEvent},
    ShellExt,
};

use crate::settings::load_settings;

const DEFAULT_HOST: &str = "127.0.0.1";
const DEFAULT_PORT: &str = "8008";

/// Holds the server child process so we can kill it on exit. Managed
/// by Tauri as application state; `kill_on_exit` is invoked from the
/// `RunEvent::Exit` handler in `lib.rs`.
pub struct ServerProcess(pub Mutex<Option<CommandChild>>);

impl ServerProcess {
    pub fn new() -> Self {
        Self(Mutex::new(None))
    }

    pub fn kill_on_exit(&self) {
        if let Ok(mut guard) = self.0.lock() {
            if let Some(child) = guard.take() {
                log::info!("Terminating backend server process");
                if let Err(e) = child.kill() {
                    log::warn!("Failed to kill server process: {e}");
                }
            }
        }
    }
}

/// Spawn the backend launcher off the Tauri runtime. Caller-side fire-
/// and-forget; errors during boot are logged and surfaced to the
/// webview via the `server-error` event.
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

    // Local AppData (not Roaming) keeps everything in one machine-
    // specific location that doesn't sync; profiles / settings don't
    // need to follow the user across domain machines.
    let app_data_dir = app
        .path()
        .app_local_data_dir()
        .map_err(|e| format!("Failed to resolve app local data dir: {e}"))?;
    let log_dir = app_data_dir.join("logs");

    std::fs::create_dir_all(&app_data_dir).ok();
    std::fs::create_dir_all(&log_dir).ok();

    let themes_dir = resolve_themes_dir(app);

    if env::var("SPIRITSTREAM_DATA_DIR").is_err() {
        command = command.env("SPIRITSTREAM_DATA_DIR", &app_data_dir);
    }
    if env::var("SPIRITSTREAM_LOG_DIR").is_err() {
        command = command.env("SPIRITSTREAM_LOG_DIR", &log_dir);
    }
    if env::var("SPIRITSTREAM_THEMES_DIR").is_err() {
        if let Some(themes) = &themes_dir {
            command = command.env("SPIRITSTREAM_THEMES_DIR", themes);
        }
    }
    // SPIRITSTREAM_HOST / SPIRITSTREAM_PORT / SPIRITSTREAM_API_TOKEN
    // are deliberately NOT set here. The server resolves bind args
    // from the active profile's `settings.backend` — injecting env-var
    // defaults from the shell would silently override that resolution
    // and was the root cause of remote-access being silently broken
    // pre-fix. Operator overrides in the parent launcher's environment
    // still propagate via standard env-var inheritance to the child.

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
    // the .app / install dir alongside `spiritstream-server`.
    //
    // On Linux the placeholder file is empty — the .deb / .rpm
    // dependency on `ffmpeg` delivers the binary, and the server falls
    // through to `$PATH` lookup. We detect the placeholder by file size
    // in `resolve_ffmpeg_sidecar` and skip the env injection.
    if env::var("SPIRITSTREAM_FFMPEG_PATH").is_err() {
        match resolve_ffmpeg_sidecar() {
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

/// Locate the themes directory. Bundled resources first; on dev a chain
/// of relative-path candidates anchored at the current working dir
/// covers `pnpm dev:desktop` (apps/tauri) and `cargo run -p
/// spiritstream-desktop` (apps/tauri/src-tauri) launch contexts.
fn resolve_themes_dir<R: Runtime>(app: &AppHandle<R>) -> Option<std::path::PathBuf> {
    let bundled = app.path().resource_dir().ok().map(|dir| dir.join("themes"));

    if let Some(ref path) = bundled {
        log::info!("Checking bundled themes at: {:?}", path);
        log::info!("  exists: {}, is_dir: {}", path.exists(), path.is_dir());

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

    if bundled
        .as_ref()
        .map(|p| p.exists() && has_theme_files(p))
        .unwrap_or(false)
    {
        log::info!("Using bundled themes directory");
        return bundled;
    }

    let cwd = std::env::current_dir().ok();
    let cwd_themes = cwd.as_ref().map(|d| d.join("themes"));
    if cwd_themes.as_ref().map(|p| has_theme_files(p)).unwrap_or(false) {
        log::info!("Using CWD themes directory: {:?}", cwd_themes);
        return cwd_themes;
    }

    let parent_themes = cwd
        .as_ref()
        .and_then(|d| d.join("../../themes").canonicalize().ok());
    if parent_themes
        .as_ref()
        .map(|p| has_theme_files(p))
        .unwrap_or(false)
    {
        log::info!("Using parent themes directory: {:?}", parent_themes);
        return parent_themes;
    }

    let grandparent_themes = cwd
        .as_ref()
        .and_then(|d| d.join("../../../themes").canonicalize().ok());
    if grandparent_themes
        .as_ref()
        .map(|p| has_theme_files(p))
        .unwrap_or(false)
    {
        log::info!("Using grandparent themes directory: {:?}", grandparent_themes);
        return grandparent_themes;
    }

    log::warn!("No themes directory found with theme files!");
    log::warn!("  Tried bundled: {:?}", bundled);
    log::warn!("  Tried CWD: {:?}", cwd_themes);
    log::warn!("  Tried parent: {:?}", parent_themes);
    log::warn!("  Tried grandparent: {:?}", grandparent_themes);
    bundled
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

/// Kill any zombie spiritstream-server processes from previous runs to
/// avoid port conflicts. Unix uses `pkill -f`; Windows uses `taskkill`
/// + a port-availability re-check loop because taskkill returns before
/// the kernel actually frees the bound port.
fn kill_existing_servers() {
    #[cfg(unix)]
    {
        use std::process::Command;
        let _ = Command::new("pkill")
            .args(["-f", "spiritstream-server"])
            .output();
        std::thread::sleep(Duration::from_millis(1000));
        log::info!("Killed any existing spiritstream-server processes");
    }
    #[cfg(windows)]
    {
        use std::process::Command;
        let result = Command::new("taskkill")
            .args(["/F", "/IM", "spiritstream-server.exe"])
            .output();

        if let Ok(output) = &result {
            if output.status.success() {
                log::info!("Taskkill succeeded for spiritstream-server.exe");
            }
        }

        std::thread::sleep(Duration::from_millis(1500));

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
