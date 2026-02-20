// Profile Commands
// Handles profile CRUD operations (load, save, delete, ordering, validation)

use serde_json::{json, Value};

use crate::app_state::{AppState, get_arg, get_opt_arg};
use crate::models::{Profile, RtmpInput};
use crate::services::EventSink;

/// Handle profile-related commands.
///
/// Returns `None` for unrecognized commands, `Some(result)` for handled ones.
pub async fn handle(state: &AppState, command: &str, payload: &Value) -> Option<Result<Value, String>> {
    match command {
        "get_all_profiles" => Some(handle_get_all_profiles(state).await),
        "get_profile_summaries" => Some(handle_get_profile_summaries(state).await),
        "load_profile" => Some(handle_load_profile(state, payload).await),
        "save_profile" => Some(handle_save_profile(state, payload).await),
        "delete_profile" => Some(handle_delete_profile(state, payload).await),
        "is_profile_encrypted" => Some(handle_is_profile_encrypted(state, payload)),
        "validate_input" => Some(handle_validate_input(state, payload).await),
        "set_profile_order" => Some(handle_set_profile_order(state, payload).await),
        "get_order_index_map" => Some(handle_get_order_index_map(state)),
        "ensure_order_indexes" => Some(handle_ensure_order_indexes(state).await),
        _ => None,
    }
}

async fn handle_get_all_profiles(state: &AppState) -> Result<Value, String> {
    let names = state.profile_manager.get_all_names().await?;
    Ok(json!(names))
}

async fn handle_get_profile_summaries(state: &AppState) -> Result<Value, String> {
    let summaries = state.profile_manager.get_all_summaries().await?;
    Ok(json!(summaries))
}

async fn handle_load_profile(state: &AppState, payload: &Value) -> Result<Value, String> {
    let name: String = get_arg(payload, "name")?;
    let password: Option<String> = get_opt_arg(payload, "password")?;
    let profile = state
        .profile_manager
        .load_with_key_decryption(&name, password.as_deref())
        .await?;

    // Update last_profile in settings so preview handler knows which profile is active
    if let Ok(mut settings) = state.settings_manager.load() {
        settings.last_profile = Some(name.clone());
        let _ = state.settings_manager.save(&settings);
    }

    Ok(json!(profile))
}

async fn handle_save_profile(state: &AppState, payload: &Value) -> Result<Value, String> {
    let profile: Profile = get_arg(payload, "profile")?;
    let password: Option<String> = get_opt_arg(payload, "password")?;
    let settings = state.settings_manager.load()?;
    state
        .profile_manager
        .save_with_key_encryption(&profile, password.as_deref(), settings.encrypt_stream_keys)
        .await?;
    state.event_bus.emit("profile_changed", json!({ "action": "saved", "name": profile.name }));
    Ok(Value::Null)
}

async fn handle_delete_profile(state: &AppState, payload: &Value) -> Result<Value, String> {
    let name: String = get_arg(payload, "name")?;
    state.profile_manager.delete(&name).await?;
    state.event_bus.emit("profile_changed", json!({ "action": "deleted", "name": name }));
    Ok(Value::Null)
}

fn handle_is_profile_encrypted(state: &AppState, payload: &Value) -> Result<Value, String> {
    let name: String = get_arg(payload, "name")?;
    Ok(json!(state.profile_manager.is_encrypted(&name)))
}

async fn handle_validate_input(state: &AppState, payload: &Value) -> Result<Value, String> {
    let profile_id: String = get_arg(payload, "profileId")?;
    let input: RtmpInput = get_arg(payload, "input")?;
    state.profile_manager.validate_input_conflict(&profile_id, &input).await?;
    Ok(Value::Null)
}

async fn handle_set_profile_order(state: &AppState, payload: &Value) -> Result<Value, String> {
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

fn handle_get_order_index_map(state: &AppState) -> Result<Value, String> {
    let map = state.profile_manager.read_order_index_map()?;
    Ok(json!(map))
}

async fn handle_ensure_order_indexes(state: &AppState) -> Result<Value, String> {
    let map = state.profile_manager.ensure_order_indexes().await?;
    Ok(json!(map))
}
