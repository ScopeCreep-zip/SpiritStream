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
    let app_data_dir = app.path().app_data_dir().ok()?;
    let settings_path = app_data_dir.join("settings.json");

    if !settings_path.exists() {
        return None;
    }

    let content = std::fs::read_to_string(&settings_path).ok()?;
    serde_json::from_str(&content).ok()
}
