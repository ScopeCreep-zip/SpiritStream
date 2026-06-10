use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, Runtime};

/// Slim shadow of `crates/core/src/models/Settings` — only the fields
/// the Tauri shell itself needs at launch time. Backend host/port/token
/// live in `ProfileSettings.backend` and are resolved server-side by
/// `crates/transport-http`; the shell never injects them as env vars
/// (it used to, which silently masked the profile-based resolution).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    #[serde(default)]
    pub start_minimized: bool,
}

pub fn load_settings<R: Runtime>(app: &AppHandle<R>) -> Option<Settings> {
    // LOCAL app data — the same directory the backend writes
    // settings.json into (`SPIRITSTREAM_DATA_DIR = app_local_data_dir`).
    // The old `app_data_dir()` read Roaming on Windows, where the file
    // never exists, so `start_minimized` was silently never honored.
    let app_data_dir = app.path().app_local_data_dir().ok()?;
    let settings_path = app_data_dir.join("settings.json");

    if !settings_path.exists() {
        return None;
    }

    let content = match std::fs::read_to_string(&settings_path) {
        Ok(c) => c,
        Err(e) => {
            log::warn!("settings: failed to read {settings_path:?}: {e}");
            return None;
        }
    };
    match serde_json::from_str(&content) {
        Ok(settings) => Some(settings),
        Err(e) => {
            // Don't silently reset the user's settings — if the JSON is
            // corrupt (truncated mid-write, hand-edited typo), surface
            // it loud so the operator can decide whether to repair the
            // file vs accept the defaults.
            log::warn!(
                "settings: failed to parse {settings_path:?}, falling back to defaults: {e}"
            );
            None
        }
    }
}
