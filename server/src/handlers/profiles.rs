use serde_json::{json, Value};

use crate::state::{set_active_profile, AppState};
use crate::util::{get_arg, get_opt_arg};
use spiritstream_server::services::EventSink;
use spiritstream_server::models::{Profile, RtmpInput};

pub(crate) async fn handle(state: &AppState, command: &str, payload: &Value) -> Result<Value, String> {
    match command {
        "get_all_profiles" => {
            let names = state.profile_manager.get_all_names().await?;
            Ok(json!(names))
        }
        "get_profile_summaries" => {
            let summaries = state.profile_manager.get_all_summaries().await?;
            Ok(json!(summaries))
        }
        "load_profile" => {
            let name: String = get_arg(payload, "name")?;
            let password: Option<String> = get_opt_arg(payload, "password")?;
            let set_active_flag: bool = get_opt_arg(payload, "setActive")?.unwrap_or(true);
            let mut profile = state
                .profile_manager
                .load_with_key_decryption(&name, password.as_deref())
                .await?;

            if set_active_flag {
                // Migration: If legacy settings were found, merge them into this profile
                if let Some(legacy_settings) = state.settings_manager.take_legacy_profile_settings() {
                    let migrated = profile.settings.merge_missing(&legacy_settings);
                    if migrated {
                        log::info!("Migrating legacy settings to profile: {}", name);
                        state
                            .profile_manager
                            .save_with_key_encryption(&profile, password.as_deref())
                            .await?;
                        log::info!("Profile migrated successfully: {}", name);
                    }
                }

                set_active_profile(state, &profile).await;
            }

            Ok(json!(profile))
        }
        "save_profile" => {
            let profile: Profile = get_arg(payload, "profile")?;
            let password: Option<String> = get_opt_arg(payload, "password")?;
            // encrypt_stream_keys is now per-profile (profile.settings.encrypt_stream_keys)
            state
                .profile_manager
                .save_with_key_encryption(&profile, password.as_deref())
                .await?;
            let is_active = {
                let guard = state.active_profile_name.lock().await;
                guard.as_deref() == Some(profile.name.as_str())
            };
            if is_active {
                set_active_profile(state, &profile).await;
            }
            state.event_bus.emit("profile_changed", json!({ "action": "saved", "name": profile.name }));
            Ok(Value::Null)
        }
        "delete_profile" => {
            let name: String = get_arg(payload, "name")?;
            state.profile_manager.delete(&name).await?;
            let was_active = {
                let guard = state.active_profile_name.lock().await;
                guard.as_deref() == Some(name.as_str())
            };
            if was_active {
                {
                    let mut guard = state.active_profile_name.lock().await;
                    *guard = None;
                }
                {
                    let mut guard = state.active_profile_settings.lock().await;
                    *guard = None;
                }
                state
                    .chat_manager
                    .update_profile_chat_settings(spiritstream_server::models::ChatSettings::default())
                    .await;
            }
            state.event_bus.emit("profile_changed", json!({ "action": "deleted", "name": name }));
            Ok(Value::Null)
        }
        "is_profile_encrypted" => {
            let name: String = get_arg(payload, "name")?;
            Ok(json!(state.profile_manager.is_encrypted(&name)))
        }
        "validate_input" => {
            let profile_id: String = get_arg(payload, "profileId")?;
            let input: RtmpInput = get_arg(payload, "input")?;
            state.profile_manager.validate_input_conflict(&profile_id, &input).await?;
            Ok(Value::Null)
        }
        "set_profile_order" => {
            let ordered_names: Vec<String> = get_arg(payload, "orderedNames")?;
            let mut map = state.profile_manager.read_order_index_map()?;
            let existing = state.profile_manager.get_all_names().await?;

            let mut idx = 0;
            for name in ordered_names {
                if !existing.contains(&name) {
                    return Err(format!("Unknown profile: {name}"));
                }
                idx += 10;
                map.insert(name, idx);
            }

            state.profile_manager.write_order_index_map(&map)?;
            Ok(Value::Null)
        }
        "get_order_index_map" => {
            let map = state.profile_manager.read_order_index_map()?;
            Ok(json!(map))
        }
        "ensure_order_indexes" => {
            let map = state.profile_manager.ensure_order_indexes().await?;
            Ok(json!(map))
        }
        _ => Err(format!("Unknown profile command: {command}")),
    }
}
