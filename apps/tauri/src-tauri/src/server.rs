use std::{env, sync::Mutex, time::Duration};
use tauri::{image::Image, AppHandle, Emitter, Manager, Runtime};
use tauri_plugin_shell::{
    process::{CommandChild, CommandEvent},
    ShellExt,
};

use crate::settings::load_settings;

const DEFAULT_HOST: &str = "127.0.0.1";

/// Shell-side launcher failures. Typed (per the no-`Result<T, String>`
/// standard) so call sites can branch and logs stay greppable.
#[derive(Debug, thiserror::Error)]
pub enum ShellError {
    #[error("failed to resolve app data dir: {0}")]
    DataDir(String),
    #[error("failed to spawn backend sidecar: {0}")]
    Spawn(String),
    #[error("failed to create main window: {0}")]
    Window(String),
    #[error("backend port discovery failed: {0}")]
    PortFile(String),
}

/// Holds the server child process so we can kill it on exit. Managed
/// by Tauri as application state; `kill_on_exit` is invoked from the
/// `RunEvent::Exit` handler in `lib.rs`.
pub struct ServerProcess(pub Mutex<Option<CommandChild>>);

impl ServerProcess {
    pub fn new() -> Self {
        Self(Mutex::new(None))
    }

    pub fn kill_on_exit(&self) {
        match self.0.lock() {
            Ok(mut guard) => {
                if let Some(child) = guard.take() {
                    log::info!("Terminating backend server process");
                    if let Err(e) = child.kill() {
                        log::warn!("Failed to kill server process: {e}");
                    }
                }
            }
            Err(e) => log::error!(
                "ServerProcess lock poisoned — sidecar may not be killed and could leak as a zombie: {e}"
            ),
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

async fn run_launcher<R: Runtime>(app: &AppHandle<R>) -> Result<(), ShellError> {
    let settings = load_settings(app).unwrap_or_default();

    // Bind args (host/port/token) are resolved server-side from the
    // active profile's `settings.backend` — with Remote Access off the
    // server asks the OS for any free port (binds port 0). The shell
    // never guesses or mirrors that resolution: the server publishes
    // the REAL bound port to `run/server.port` after binding, and the
    // shell reads it back. The CSP / capability allow-lists use
    // wildcard loopback ports (`http://127.0.0.1:*`), so whatever port
    // comes up works without rebuilding the shell.
    let data_dir = crate::port_file::effective_data_dir(app)?;

    // The previous run's port file (if any) tells us which port to
    // verify released after reaping a stale sidecar. Read BEFORE
    // deleting; the delete guarantees the post-spawn poll can only be
    // satisfied by the new child's own write.
    let stale_port = crate::port_file::read(&data_dir).map(|record| record.port);
    kill_stale_sidecar(app, stale_port);
    crate::port_file::remove(&data_dir);

    // Desktop: spawn the sidecar, learn the negotiated port from the
    // discovery file (written by the server only after its listener is
    // bound), confirm the TCP socket accepts connections, and publish
    // the URL for the `backend_url` command — all BEFORE any webview
    // exists, so the frontend bootstrap can never race the discovery.
    //
    // The webview is NOT created yet — the React bundle is not running
    // and cannot fire any HTTP requests. Once this block completes, we
    // build the window programmatically; React mounts knowing the
    // backend is reachable, and `/api/v1/ready` either returns 200
    // immediately (fast path) or long-polls until services finish
    // initializing. This ordering is the entire reason there are no
    // console errors during boot: nothing in the webview exists to make
    // a failed request. Tauri's `visible: false` does not provide this
    // guarantee (issues #5583 / #7669 / #10950) — only deferred window
    // creation does.
    #[cfg(desktop)]
    {
        let server_pid = spawn_server(app)?;
        let port =
            crate::port_file::await_with_pid(&data_dir, server_pid, Duration::from_secs(10))
                .await?;
        log::info!("Backend negotiated port {port} (pid {server_pid})");
        wait_for_tcp_listening(DEFAULT_HOST, port).await;

        if let Some(discovered) = app.try_state::<crate::DiscoveredBackend>() {
            let url = format!("http://{DEFAULT_HOST}:{port}");
            match discovered.0.lock() {
                Ok(mut guard) => *guard = Some(url),
                Err(e) => log::error!("DiscoveredBackend lock poisoned: {e}"),
            }
        }
    }
    #[cfg(not(desktop))]
    {
        // Mobile: the server is linked in-process; nothing to spawn and
        // no discovery to run yet. A future in-process Axum bind must
        // write the same `run/server.port` file so the desktop discovery
        // path extends unchanged. The window still gets built below —
        // the webview's /ready overlay reports the missing backend.
        log::info!("Mobile build: skipping sidecar spawn (server linked in-process)");
    }

    // Build the main webview window. `WebviewUrl::default()` resolves
    // to `App("index.html".into())`, which Tauri swaps to the dev URL
    // (`http://localhost:1420`) in `tauri dev` and to the bundled SPA
    // in release. Same call site, both modes.
    let window = match tauri::WebviewWindowBuilder::new(app, "main", tauri::WebviewUrl::default())
        .title("SpiritStream")
        .inner_size(1500.0, 1000.0)
        .min_inner_size(1024.0, 600.0)
        .resizable(true)
        .center()
        .background_color(tauri::utils::config::Color(0x0F, 0x0A, 0x14, 0xFF))
        .build()
    {
        Ok(w) => w,
        Err(e) => return Err(ShellError::Window(e.to_string())),
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

/// Spawn the bundled `spiritstream-server` sidecar. Desktop only —
/// J1: iOS/Android Tauri 2 builds cannot host an out-of-process
/// sidecar (mobile sandbox + signing constraints), so the
/// `tauri-plugin-shell` `sidecar(...)` call is unavailable on those
/// targets. Mobile clients must link `spiritstream-core` directly and
/// bind Axum in-process (the mobile shell entry point handles this).
/// Compiling this for mobile would fail link-time on the missing
/// sidecar binary anyway.
#[cfg(desktop)]
fn spawn_server<R: Runtime>(app: &AppHandle<R>) -> Result<u32, ShellError> {
    // `SPIRITSTREAM_SERVER_PATH` lets the dev iteration loop point at a
    // freshly-rebuilt server binary outside the bundle. In RELEASE
    // builds this would be a privilege-escalation vector: anyone who
    // can set the user's env (compromised shell-rc, supply chain,
    // shared host) could replace the backend binary the next time
    // SpiritStream launched. Gate behind debug_assertions so release
    // builds always use the bundled, signed sidecar.
    #[cfg(debug_assertions)]
    let mut command = if let Ok(server_path) = env::var("SPIRITSTREAM_SERVER_PATH") {
        log::warn!(
            "SPIRITSTREAM_SERVER_PATH={server_path} overriding bundled sidecar — debug build only"
        );
        app.shell().command(server_path)
    } else {
        app.shell()
            .sidecar("spiritstream-server")
            .map_err(|e| ShellError::Spawn(e.to_string()))?
    };
    #[cfg(not(debug_assertions))]
    let mut command = app
        .shell()
        .sidecar("spiritstream-server")
        .map_err(|e| ShellError::Spawn(e.to_string()))?;

    // Local AppData (not Roaming) keeps everything in one machine-
    // specific location that doesn't sync; profiles / settings don't
    // need to follow the user across domain machines.
    let app_data_dir = app
        .path()
        .app_local_data_dir()
        .map_err(|e| ShellError::DataDir(e.to_string()))?;
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
        ShellError::Spawn(e.to_string())
    })?;

    // Persist the sidecar pid so the NEXT launch (after a shell crash /
    // SIGKILL that skipped RunEvent::Exit) can reap exactly this
    // process — by pid, after verifying its identity — instead of the
    // old `pkill -f spiritstream-server`, which killed any same-user
    // process whose command line merely mentioned the name (an editor
    // on the log file, a cargo build, a second instance's healthy
    // backend).
    let child_pid = child.pid();
    write_sidecar_pid_file(&app_data_dir, child_pid);

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

                    emit_server_error_when_deliverable(&app_handle, error_msg.clone());
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

    Ok(child_pid)
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
    if cwd_themes
        .as_ref()
        .map(|p| has_theme_files(p))
        .unwrap_or(false)
    {
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
        log::info!(
            "Using grandparent themes directory: {:?}",
            grandparent_themes
        );
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

    let bundled_name = if cfg!(windows) {
        "ffmpeg.exe"
    } else {
        "ffmpeg"
    };
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
        let Some(path) = candidate.as_ref() else {
            continue;
        };
        let Ok(meta) = std::fs::metadata(path) else {
            continue;
        };
        if meta.is_file() && meta.len() > 0 {
            return Some(path.clone());
        }
    }
    None
}

/// Reap a stale sidecar from a previous run (shell crash / SIGKILL
/// skipped the orderly RunEvent::Exit kill) so the port is free. The
/// pid comes from our own `sidecar.pid` file and is verified to still
/// be a `spiritstream-server` process before any signal is sent — a
/// recycled pid running someone else's program is left alone, loudly.
fn kill_stale_sidecar<R: Runtime>(app: &AppHandle<R>, stale_port: Option<u16>) {
    let Some(data_dir) = app.path().app_local_data_dir().ok() else {
        return;
    };
    let pid_path = data_dir.join("sidecar.pid");
    let Ok(text) = std::fs::read_to_string(&pid_path) else {
        return; // no previous run to clean up
    };
    let _ = std::fs::remove_file(&pid_path);
    let Ok(pid) = text.trim().parse::<u32>() else {
        log::warn!("sidecar.pid was unparseable; skipping stale-sidecar reap");
        return;
    };

    let mut system = sysinfo::System::new();
    system.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
    let Some(process) = system.process(sysinfo::Pid::from_u32(pid)) else {
        return; // already gone
    };
    let name = process.name().to_string_lossy().to_string();
    if !name.contains("spiritstream-server") {
        log::warn!(
            "sidecar.pid {pid} now belongs to {name:?} (pid reuse) — refusing to kill it"
        );
        return;
    }
    log::info!("Reaping stale sidecar from previous run (pid {pid})");
    // Two-phase: TERM, brief grace, then KILL if still alive.
    let _ = process.kill_with(sysinfo::Signal::Term);
    std::thread::sleep(Duration::from_millis(1500));
    system.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
    if let Some(process) = system.process(sysinfo::Pid::from_u32(pid)) {
        if process
            .name()
            .to_string_lossy()
            .contains("spiritstream-server")
        {
            process.kill();
        }
    }
    // Give the kernel a beat to release the previously bound port —
    // only knowable when the previous run left a port file behind. The
    // new server binds port 0 in the common case, so this is purely
    // about not surprising a fixed-port (Remote Access) setup.
    let Some(port) = stale_port else {
        return;
    };
    for attempt in 1..=3 {
        if std::net::TcpListener::bind(("127.0.0.1", port)).is_ok() {
            log::info!("Port {port} is available after {attempt} attempt(s)");
            break;
        }
        log::warn!("Port {port} still in use, waiting… (attempt {attempt})");
        std::thread::sleep(Duration::from_millis(1000));
    }
}

fn write_sidecar_pid_file(data_dir: &std::path::Path, pid: u32) {
    let path = data_dir.join("sidecar.pid");
    if let Err(e) = std::fs::write(&path, pid.to_string()) {
        log::warn!("failed to write sidecar.pid: {e}");
    }
}

/// Emit `server-error` once a window exists to receive it. The
/// terminated handler used to emit immediately — usually BEFORE the
/// main window was built — so the event evaporated and the user saw
/// only a generic "unreachable" overlay with no cause attached.
fn emit_server_error_when_deliverable<R: Runtime>(app: &AppHandle<R>, error_msg: String) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        for _ in 0..120 {
            if app.get_webview_window("main").is_some() {
                let _ = app.emit("server-error", &error_msg);
                return;
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
        log::error!("server-error never deliverable (no window after 30s): {error_msg}");
    });
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
async fn wait_for_tcp_listening(host: &str, port: u16) {
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
