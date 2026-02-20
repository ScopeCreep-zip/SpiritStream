// Scene Commands
// Handles scene management operations: create, update, delete, duplicate, set active

use crate::app_state::{AppState, get_arg, get_opt_arg};
use crate::models::Scene;
use crate::services::{EventSink, SourceTransition};
use serde_json::{json, Value};

/// Handle scene-related commands.
///
/// Returns `None` if the command is not recognized, allowing the caller
/// to try other command modules.
pub async fn handle(state: &AppState, command: &str, payload: &Value) -> Option<Result<Value, String>> {
    match command {
        "create_scene" => Some(create_scene(state, payload).await),
        "update_scene" => Some(update_scene(state, payload).await),
        "delete_scene" => Some(delete_scene(state, payload).await),
        "set_active_scene" => Some(set_active_scene(state, payload).await),
        "duplicate_scene" => Some(duplicate_scene(state, payload).await),
        _ => None,
    }
}

async fn create_scene(state: &AppState, payload: &Value) -> Result<Value, String> {
    let profile_name: String = get_arg(payload, "profileName")?;
    let name: String = get_arg(payload, "name")?;
    let width: Option<u32> = get_opt_arg(payload, "width")?;
    let height: Option<u32> = get_opt_arg(payload, "height")?;
    let password: Option<String> = get_opt_arg(payload, "password")?;

    let mut profile = state
        .profile_manager
        .load_with_key_decryption(&profile_name, password.as_deref())
        .await?;

    let scene = Scene {
        id: uuid::Uuid::new_v4().to_string(),
        name,
        canvas_width: width.unwrap_or(1920),
        canvas_height: height.unwrap_or(1080),
        layers: Vec::new(),
        audio_mixer: Default::default(),
        transition_in: None,
    };

    let scene_id = scene.id.clone();
    profile.scenes.push(scene);

    // Set as active if it's the first scene
    if profile.active_scene_id.is_none() {
        profile.active_scene_id = Some(scene_id.clone());
    }

    let settings = state.settings_manager.load()?;
    state
        .profile_manager
        .save_with_key_encryption(&profile, password.as_deref(), settings.encrypt_stream_keys)
        .await?;

    state.event_bus.emit("scene_created", json!({ "profileName": profile_name, "sceneId": scene_id }));
    Ok(json!({ "sceneId": scene_id }))
}

async fn update_scene(state: &AppState, payload: &Value) -> Result<Value, String> {
    let profile_name: String = get_arg(payload, "profileName")?;
    let scene_id: String = get_arg(payload, "sceneId")?;
    let updates: Value = get_arg(payload, "updates")?;
    let password: Option<String> = get_opt_arg(payload, "password")?;

    let mut profile = state
        .profile_manager
        .load_with_key_decryption(&profile_name, password.as_deref())
        .await?;

    let scene_idx = profile.scenes.iter().position(|s| s.id == scene_id)
        .ok_or_else(|| format!("Scene {} not found", scene_id))?;

    // Merge updates into existing scene
    let mut scene_json = serde_json::to_value(&profile.scenes[scene_idx])
        .map_err(|e| e.to_string())?;
    if let (Some(obj), Some(upd)) = (scene_json.as_object_mut(), updates.as_object()) {
        for (k, v) in upd {
            obj.insert(k.clone(), v.clone());
        }
    }
    profile.scenes[scene_idx] = serde_json::from_value(scene_json)
        .map_err(|e| format!("Failed to update scene: {}", e))?;

    let settings = state.settings_manager.load()?;
    state
        .profile_manager
        .save_with_key_encryption(&profile, password.as_deref(), settings.encrypt_stream_keys)
        .await?;

    state.event_bus.emit("scene_updated", json!({ "profileName": profile_name, "sceneId": scene_id }));
    Ok(json!(profile.scenes[scene_idx]))
}

async fn delete_scene(state: &AppState, payload: &Value) -> Result<Value, String> {
    let profile_name: String = get_arg(payload, "profileName")?;
    let scene_id: String = get_arg(payload, "sceneId")?;
    let password: Option<String> = get_opt_arg(payload, "password")?;

    let mut profile = state
        .profile_manager
        .load_with_key_decryption(&profile_name, password.as_deref())
        .await?;

    // If deleting the active scene, remove lifecycle refs for its sources
    let is_active = profile.active_scene_id.as_deref() == Some(&scene_id);
    if is_active {
        if let Some(scene) = profile.scenes.iter().find(|s| s.id == scene_id) {
            for layer in &scene.layers {
                let transition = state.source_lifecycle.remove_ref(&layer.source_id, layer.visible);
                if transition == SourceTransition::Deactivated {
                    log::info!("Source {} deactivated (scene deleted), stopping captures", layer.source_id);
                    let _ = state.h264_capture.stop_capture(&layer.source_id);
                    state.native_preview.stop_preview(&layer.source_id);
                    state.media_audio_decoder.stop(&layer.source_id);
                        state.audio_level_service.unregister_audio_source(&layer.source_id);
                    state.power_budget.release_power();
                }
            }
        }
    }

    let initial_len = profile.scenes.len();
    profile.scenes.retain(|s| s.id != scene_id);

    if profile.scenes.len() == initial_len {
        return Err(format!("Scene {} not found", scene_id));
    }

    // Update active scene if deleted
    if is_active {
        let new_active = profile.scenes.first().map(|s| s.id.clone());
        // Add refs for new active scene sources
        if let Some(ref new_id) = new_active {
            if let Some(new_scene) = profile.scenes.iter().find(|s| &s.id == new_id) {
                for layer in &new_scene.layers {
                    let transition = state.source_lifecycle.add_ref(&layer.source_id, layer.visible);
                    if transition == SourceTransition::Activated {
                        state.power_budget.acquire_power("video source active");
                    }
                }
            }
        }
        profile.active_scene_id = new_active;
    }

    let settings = state.settings_manager.load()?;
    state
        .profile_manager
        .save_with_key_encryption(&profile, password.as_deref(), settings.encrypt_stream_keys)
        .await?;

    state.event_bus.emit("scene_deleted", json!({ "profileName": profile_name, "sceneId": scene_id }));
    Ok(Value::Null)
}

async fn set_active_scene(state: &AppState, payload: &Value) -> Result<Value, String> {
    let profile_name: String = get_arg(payload, "profileName")?;
    let scene_id: String = get_arg(payload, "sceneId")?;
    let password: Option<String> = get_opt_arg(payload, "password")?;

    let mut profile = state
        .profile_manager
        .load_with_key_decryption(&profile_name, password.as_deref())
        .await?;

    // Verify scene exists
    if !profile.scenes.iter().any(|s| s.id == scene_id) {
        return Err(format!("Scene {} not found", scene_id));
    }

    // Track source lifecycle transitions: remove old scene refs, add new scene refs
    let old_scene_id = profile.active_scene_id.clone();
    if old_scene_id.as_deref() != Some(&scene_id) {
        // Remove refs for old scene sources
        if let Some(ref old_id) = old_scene_id {
            if let Some(old_scene) = profile.scenes.iter().find(|s| &s.id == old_id) {
                for layer in &old_scene.layers {
                    let transition = state.source_lifecycle.remove_ref(&layer.source_id, layer.visible);
                    if transition == SourceTransition::Deactivated {
                        log::info!("Source {} deactivated (left all scenes), stopping captures", layer.source_id);
                        let _ = state.h264_capture.stop_capture(&layer.source_id);
                        state.native_preview.stop_preview(&layer.source_id);
                        state.media_audio_decoder.stop(&layer.source_id);
                        state.audio_level_service.unregister_audio_source(&layer.source_id);
                        state.power_budget.release_power();
                    }
                }
            }
        }

        // Add refs for new scene sources
        if let Some(new_scene) = profile.scenes.iter().find(|s| s.id == scene_id) {
            for layer in &new_scene.layers {
                let transition = state.source_lifecycle.add_ref(&layer.source_id, layer.visible);
                if transition == SourceTransition::Activated {
                    state.power_budget.acquire_power("video source active");
                }
            }
        }
    }

    profile.active_scene_id = Some(scene_id.clone());

    let settings = state.settings_manager.load()?;
    state
        .profile_manager
        .save_with_key_encryption(&profile, password.as_deref(), settings.encrypt_stream_keys)
        .await?;

    state.event_bus.emit("active_scene_changed", json!({ "profileName": profile_name, "sceneId": scene_id }));
    Ok(Value::Null)
}

async fn duplicate_scene(state: &AppState, payload: &Value) -> Result<Value, String> {
    let profile_name: String = get_arg(payload, "profileName")?;
    let scene_id: String = get_arg(payload, "sceneId")?;
    let new_name: Option<String> = get_opt_arg(payload, "newName")?;
    let password: Option<String> = get_opt_arg(payload, "password")?;

    let mut profile = state
        .profile_manager
        .load_with_key_decryption(&profile_name, password.as_deref())
        .await?;

    let original = profile.scenes.iter().find(|s| s.id == scene_id)
        .ok_or_else(|| format!("Scene {} not found", scene_id))?
        .clone();

    let mut new_scene = original;
    new_scene.id = uuid::Uuid::new_v4().to_string();
    new_scene.name = new_name.unwrap_or_else(|| format!("{} (Copy)", new_scene.name));

    // Generate new layer IDs
    for layer in &mut new_scene.layers {
        layer.id = uuid::Uuid::new_v4().to_string();
    }

    let new_scene_id = new_scene.id.clone();
    profile.scenes.push(new_scene);

    let settings = state.settings_manager.load()?;
    state
        .profile_manager
        .save_with_key_encryption(&profile, password.as_deref(), settings.encrypt_stream_keys)
        .await?;

    state.event_bus.emit("scene_duplicated", json!({ "profileName": profile_name, "sceneId": new_scene_id }));
    Ok(json!({ "sceneId": new_scene_id }))
}
