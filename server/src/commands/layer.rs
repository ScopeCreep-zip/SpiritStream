// Layer Commands
// Handles layer management operations: add, update, remove, reorder

use serde_json::{json, Value};

use crate::app_state::{AppState, get_arg, get_opt_arg};
use crate::models::{AudioDeviceSource, Source, SourceAudioConfig, SourceLayer, Transform};
use crate::services::{EventSink, SourceTransition};

/// Handle layer management commands.
///
/// Returns `None` for unrecognized commands, `Some(result)` for handled ones.
pub async fn handle(state: &AppState, command: &str, payload: &Value) -> Option<Result<Value, String>> {
    match command {
        "add_layer_to_scene" => Some(add_layer_to_scene(state, payload).await),
        "update_layer" => Some(update_layer(state, payload).await),
        "remove_layer" => Some(remove_layer(state, payload).await),
        "reorder_layers" => Some(reorder_layers(state, payload).await),
        _ => None,
    }
}

async fn add_layer_to_scene(state: &AppState, payload: &Value) -> Result<Value, String> {
    let profile_name: String = get_arg(payload, "profileName")?;
    let scene_id: String = get_arg(payload, "sceneId")?;
    let source_id: String = get_arg(payload, "sourceId")?;
    let transform: Option<Transform> = get_opt_arg(payload, "transform")?;
    let password: Option<String> = get_opt_arg(payload, "password")?;

    let mut profile = state
        .profile_manager
        .load_with_key_decryption(&profile_name, password.as_deref())
        .await?;

    // Verify source exists
    if !profile.sources.iter().any(|s| s.id() == source_id) {
        return Err(format!("Source {} not found", source_id));
    }

    // Get scene index for later use
    let scene_idx = profile
        .scenes
        .iter()
        .position(|s| s.id == scene_id)
        .ok_or_else(|| format!("Scene {} not found", scene_id))?;

    // Get canvas dimensions and max z-index
    let (canvas_width, canvas_height, max_z) = {
        let scene = &profile.scenes[scene_idx];
        let max_z = scene.layers.iter().map(|l| l.z_index).max().unwrap_or(0);
        (scene.canvas_width, scene.canvas_height, max_z)
    };

    let layer = SourceLayer {
        id: uuid::Uuid::new_v4().to_string(),
        source_id: source_id.clone(),
        visible: true,
        locked: false,
        transform: transform.unwrap_or_else(|| Transform {
            x: 0,
            y: 0,
            width: canvas_width,
            height: canvas_height,
            rotation: 0.0,
            crop: None,
        }),
        z_index: max_z + 1,
    };

    let layer_id = layer.id.clone();
    profile.scenes[scene_idx].layers.push(layer);

    // Check if source is a Camera with captureAudio enabled and linked audio device
    // If so, auto-create a linked AudioDeviceSource
    let mut linked_audio_source_id: Option<String> = None;
    if let Some(Source::Camera(camera)) = profile.sources.iter().find(|s| s.id() == &source_id) {
        if camera.capture_audio {
            if let Some(ref audio_device_id) = camera.linked_audio_device_id {
                // Check if linked audio source already exists for this camera
                let linked_audio_exists = profile.sources.iter().any(|s| {
                    if let Source::AudioDevice(ad) = s {
                        ad.linked_to_source_id.as_ref() == Some(&source_id)
                    } else {
                        false
                    }
                });

                if !linked_audio_exists {
                    // Get the audio device name (use camera name + Audio as fallback)
                    let audio_device_name = format!("{} (Audio)", camera.name);

                    // Create linked AudioDeviceSource
                    let linked_audio_id = uuid::Uuid::new_v4().to_string();
                    let linked_audio = Source::AudioDevice(AudioDeviceSource {
                        id: linked_audio_id.clone(),
                        name: audio_device_name,
                        device_id: audio_device_id.clone(),
                        channels: None,
                        sample_rate: None,
                        linked_to_source_id: Some(source_id.clone()),
                    });

                    // Add to profile sources
                    profile.sources.push(linked_audio);

                    // Create source-level audio config for linked audio source
                    profile.source_audio_configs.insert(
                        linked_audio_id.clone(),
                        SourceAudioConfig::default(),
                    );

                    linked_audio_source_id = Some(linked_audio_id);
                }
            }
        }
    }

    // Ensure source-level audio config exists
    profile.source_audio_configs
        .entry(source_id.clone())
        .or_insert_with(SourceAudioConfig::default);

    // Track source lifecycle: if adding to the active scene, register a ref
    if profile.active_scene_id.as_deref() == Some(&scene_id) {
        let transition = state.source_lifecycle.add_ref(&source_id, true);
        if transition == SourceTransition::Activated {
            state.power_budget.acquire_power("video source active");
        }
    }

    let settings = state.settings_manager.load()?;
    state
        .profile_manager
        .save_with_key_encryption(&profile, password.as_deref(), settings.encrypt_stream_keys)
        .await?;

    state.event_bus.emit(
        "layer_added",
        json!({
            "profileName": profile_name,
            "sceneId": scene_id,
            "layerId": layer_id,
            "linkedAudioSourceId": linked_audio_source_id
        }),
    );
    Ok(json!({
        "layerId": layer_id,
        "linkedAudioSourceId": linked_audio_source_id,
        "sourceAudioConfigs": profile.source_audio_configs,
    }))
}

async fn update_layer(state: &AppState, payload: &Value) -> Result<Value, String> {
    let profile_name: String = get_arg(payload, "profileName")?;
    let scene_id: String = get_arg(payload, "sceneId")?;
    let layer_id: String = get_arg(payload, "layerId")?;
    let updates: Value = get_arg(payload, "updates")?;
    let password: Option<String> = get_opt_arg(payload, "password")?;

    let mut profile = state
        .profile_manager
        .load_with_key_decryption(&profile_name, password.as_deref())
        .await?;

    let scene_idx = profile
        .scenes
        .iter()
        .position(|s| s.id == scene_id)
        .ok_or_else(|| format!("Scene {} not found", scene_id))?;

    let layer_idx = profile.scenes[scene_idx]
        .layers
        .iter()
        .position(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer {} not found", layer_id))?;

    // Capture old visibility before merge for lifecycle tracking
    let old_visible = profile.scenes[scene_idx].layers[layer_idx].visible;

    // Merge updates into existing layer
    let mut layer_json = serde_json::to_value(&profile.scenes[scene_idx].layers[layer_idx])
        .map_err(|e| e.to_string())?;
    if let (Some(obj), Some(upd)) = (layer_json.as_object_mut(), updates.as_object()) {
        for (k, v) in upd {
            obj.insert(k.clone(), v.clone());
        }
    }
    profile.scenes[scene_idx].layers[layer_idx] = serde_json::from_value(layer_json)
        .map_err(|e| format!("Failed to update layer: {}", e))?;

    let updated_layer = profile.scenes[scene_idx].layers[layer_idx].clone();

    // Track visibility changes for source lifecycle (only for active scene)
    let new_visible = updated_layer.visible;
    if old_visible != new_visible && profile.active_scene_id.as_deref() == Some(&scene_id) {
        let source_id = &updated_layer.source_id;
        let transition = state.source_lifecycle.set_visibility(source_id, new_visible);
        match transition {
            SourceTransition::Hidden => {
                log::info!("Source {} hidden (eye toggle), stopping video capture", source_id);
                let _ = state.h264_capture.stop_capture(source_id);
                state.native_preview.stop_preview(source_id);
                state.power_budget.release_power();
            }
            SourceTransition::Shown => {
                log::info!("Source {} shown (eye toggle), resuming video capture", source_id);
                state.power_budget.acquire_power("layer shown");
            }
            _ => {}
        }
    }

    let settings = state.settings_manager.load()?;
    state
        .profile_manager
        .save_with_key_encryption(&profile, password.as_deref(), settings.encrypt_stream_keys)
        .await?;

    state.event_bus.emit(
        "layer_updated",
        json!({ "profileName": profile_name, "sceneId": scene_id, "layerId": layer_id }),
    );
    Ok(json!(updated_layer))
}

async fn remove_layer(state: &AppState, payload: &Value) -> Result<Value, String> {
    let profile_name: String = get_arg(payload, "profileName")?;
    let scene_id: String = get_arg(payload, "sceneId")?;
    let layer_id: String = get_arg(payload, "layerId")?;
    let password: Option<String> = get_opt_arg(payload, "password")?;

    let mut profile = state
        .profile_manager
        .load_with_key_decryption(&profile_name, password.as_deref())
        .await?;

    let scene = profile
        .scenes
        .iter_mut()
        .find(|s| s.id == scene_id)
        .ok_or_else(|| format!("Scene {} not found", scene_id))?;

    // Find layer info before removing (for lifecycle tracking)
    let removed_layer = scene.layers.iter().find(|l| l.id == layer_id).cloned();

    let initial_len = scene.layers.len();
    scene.layers.retain(|l| l.id != layer_id);

    if scene.layers.len() == initial_len {
        return Err(format!("Layer {} not found", layer_id));
    }

    // Track source lifecycle: if removing from the active scene, release a ref
    if let Some(ref layer) = removed_layer {
        if profile.active_scene_id.as_deref() == Some(&scene_id) {
            let transition = state.source_lifecycle.remove_ref(&layer.source_id, layer.visible);
            if transition == SourceTransition::Deactivated {
                log::info!("Source {} deactivated (removed from last scene), stopping captures", layer.source_id);
                let _ = state.h264_capture.stop_capture(&layer.source_id);
                state.native_preview.stop_preview(&layer.source_id);
                state.media_audio_decoder.stop(&layer.source_id);
                state.audio_level_service.unregister_audio_source(&layer.source_id);
                state.power_budget.release_power();
            }
        }
    }

    let settings = state.settings_manager.load()?;
    state
        .profile_manager
        .save_with_key_encryption(&profile, password.as_deref(), settings.encrypt_stream_keys)
        .await?;

    state.event_bus.emit(
        "layer_removed",
        json!({ "profileName": profile_name, "sceneId": scene_id, "layerId": layer_id }),
    );
    Ok(Value::Null)
}

async fn reorder_layers(state: &AppState, payload: &Value) -> Result<Value, String> {
    let profile_name: String = get_arg(payload, "profileName")?;
    let scene_id: String = get_arg(payload, "sceneId")?;
    let layer_ids: Vec<String> = get_arg(payload, "layerIds")?;
    let password: Option<String> = get_opt_arg(payload, "password")?;

    let mut profile = state
        .profile_manager
        .load_with_key_decryption(&profile_name, password.as_deref())
        .await?;

    let scene = profile
        .scenes
        .iter_mut()
        .find(|s| s.id == scene_id)
        .ok_or_else(|| format!("Scene {} not found", scene_id))?;

    // Assign z-index based on provided order
    for (idx, layer_id) in layer_ids.iter().enumerate() {
        if let Some(layer) = scene.layers.iter_mut().find(|l| &l.id == layer_id) {
            layer.z_index = idx as i32;
        }
    }

    let settings = state.settings_manager.load()?;
    state
        .profile_manager
        .save_with_key_encryption(&profile, password.as_deref(), settings.encrypt_stream_keys)
        .await?;

    state.event_bus.emit(
        "layers_reordered",
        json!({ "profileName": profile_name, "sceneId": scene_id }),
    );
    Ok(Value::Null)
}
