use serde_json::{json, Value};
use std::path::PathBuf;

use crate::state::AppState;
use crate::util::get_arg;
use spiritstream_server::models::Settings;
use spiritstream_server::services::{prune_logs, validate_path_within_any, Encryption, EventSink};

pub(crate) async fn handle(state: &AppState, command: &str, payload: &Value) -> Result<Value, String> {
    match command {
        "get_settings" => Ok(json!(state.settings_manager.load()?)),
        "save_settings" => {
            let new_settings: Settings = get_arg(payload, "settings")?;

            // Check if encryption was just enabled
            let old_settings = state.settings_manager.load().ok();
            let encryption_just_enabled = new_settings.encrypt_stream_keys
                && old_settings.as_ref().is_some_and(|s| !s.encrypt_stream_keys);

            // Save the new settings
            state.settings_manager.save(&new_settings)?;

            // If encryption was just enabled, re-encrypt all profiles
            if encryption_just_enabled {
                log::info!("Stream key encryption enabled, encrypting existing profiles");
                match state.profile_manager.encrypt_all_profiles().await {
                    Ok(count) => log::info!("Encrypted stream keys in {count} profiles"),
                    Err(e) => log::error!("Failed to encrypt profiles: {e}"),
                }
            }

            let _ = prune_logs(&state.log_dir, new_settings.log_retention_days);
            state.event_bus.emit("settings_changed", json!({}));
            Ok(Value::Null)
        }
        "get_profiles_path" => {
            let path = state.settings_manager.get_profiles_path();
            Ok(json!(path.to_string_lossy().to_string()))
        }
        "export_data" => {
            let export_path: String = get_arg(payload, "exportPath")?;
            let path = PathBuf::from(&export_path);

            // Security: Validate export path is within allowed directories
            let mut allowed_dirs: Vec<&std::path::Path> = vec![state.app_data_dir.as_path()];
            if let Some(ref home) = state.home_dir {
                allowed_dirs.push(home.as_path());
            }

            validate_path_within_any(&path, &allowed_dirs)?;

            state.settings_manager.export_data(&path)?;
            Ok(Value::Null)
        }
        "clear_data" => {
            state.settings_manager.clear_data()?;
            Ok(Value::Null)
        }
        "rotate_machine_key" => {
            let profiles_dir = state.app_data_dir.join("profiles");
            let report = Encryption::rotate_machine_key(&state.app_data_dir, &profiles_dir)?;
            Ok(json!(report))
        }
        _ => Err(format!("Unknown settings command: {command}")),
    }
}
