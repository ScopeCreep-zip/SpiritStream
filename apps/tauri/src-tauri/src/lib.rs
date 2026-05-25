// SpiritStream Desktop — Tauri 2 shell.
//
// `main.rs` is a 3-line entrypoint; everything wireable lives here so
// the iOS / Android shells (which can't have a `main` of their own) can
// call `run()` from their `mobile_entry_point`-marked function.

mod server;
mod settings;
mod updater;

use tauri::{AppHandle, Manager, RunEvent};
use tauri_plugin_log::{Target, TargetKind};

use server::{launch, ServerProcess};
use updater::updater_supported;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_http::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        // App self-update via GitHub Releases. The plugin verifies each
        // `.sig` file (locally signed by the maintainer) against the
        // `pubkey` embedded in `tauri.conf.json`. Linux .deb / .rpm
        // installs opt out at the UI layer via `updater_supported`;
        // the plugin itself is always registered so any platform can
        // call into it.
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        // Global system-wide hotkeys (panic shortcut). The frontend
        // registers / unregisters bindings via the JS side of the
        // plugin; this initializer just exposes the OS hook so the
        // shortcut fires even when SpiritStream isn't focused.
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .manage(ServerProcess::new())
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
            // programmatically in `server::launch`, AFTER the backend's
            // TCP socket is accepting connections. This is the only
            // architecture in Tauri 2 that prevents the React bundle
            // from executing before the backend exists: `visible: false`
            // doesn't stop JS execution (per Tauri issues #5583, #7669,
            // #10950), and there is no `navigate_later` API. The
            // webview must not exist at all until the gate passes.
            launch(app.handle());

            log::info!("SpiritStream Desktop initialized");

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(handle_run_event);
}

fn handle_run_event(app_handle: &AppHandle, event: RunEvent) {
    if let RunEvent::Exit = event {
        log::info!("SpiritStream Desktop exiting");
        if let Some(server_state) = app_handle.try_state::<ServerProcess>() {
            server_state.kill_on_exit();
        }
    }
}
