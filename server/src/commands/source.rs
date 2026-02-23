// Source Commands
// Handles source CRUD operations (add, update, remove, reorder)

use serde_json::{json, Value};

use crate::app_state::{AppState, get_arg, get_opt_arg};
use crate::models::{Source, SourceAudioConfig};
use crate::services::EventSink;

/// Handle source-related commands.
///
/// Returns `None` for unrecognized commands, `Some(result)` for handled ones.
pub async fn handle(
    state: &AppState,
    command: &str,
    payload: &Value,
) -> Option<Result<Value, String>> {
    match command {
        "add_source" => Some(handle_add_source(state, payload).await),
        "update_source" => Some(handle_update_source(state, payload).await),
        "remove_source" => Some(handle_remove_source(state, payload).await),
        "reorder_sources" => Some(handle_reorder_sources(state, payload).await),
        _ => None,
    }
}

async fn handle_add_source(state: &AppState, payload: &Value) -> Result<Value, String> {
    let profile_name: String = get_arg(payload, "profileName")?;
    let source: Source = get_arg(payload, "source")?;
    let password: Option<String> = get_opt_arg(payload, "password")?;

    let mut profile = state
        .profile_manager
        .load_with_key_decryption(&profile_name, password.as_deref())
        .await?;

    // Check for duplicate source ID
    if profile.sources.iter().any(|s| s.id() == source.id()) {
        return Err(format!("Source with ID {} already exists", source.id()));
    }

    // Create default source-level audio config (OBS pattern)
    let source_id = source.id().to_string();
    profile.source_audio_configs.insert(source_id, SourceAudioConfig::default());

    profile.sources.push(source);

    let settings = state.settings_manager.load()?;
    state
        .profile_manager
        .save_with_key_encryption(&profile, password.as_deref(), settings.encrypt_stream_keys)
        .await?;

    state
        .event_bus
        .emit("source_added", json!({ "profileName": profile_name }));
    Ok(json!(profile.sources))
}

async fn handle_update_source(state: &AppState, payload: &Value) -> Result<Value, String> {
    let profile_name: String = get_arg(payload, "profileName")?;
    let source_id: String = get_arg(payload, "sourceId")?;
    let updates: Value = get_arg(payload, "updates")?;
    let password: Option<String> = get_opt_arg(payload, "password")?;

    let mut profile = state
        .profile_manager
        .load_with_key_decryption(&profile_name, password.as_deref())
        .await?;

    // Find and update the source
    let source_idx = profile
        .sources
        .iter()
        .position(|s| s.id() == source_id)
        .ok_or_else(|| format!("Source {} not found", source_id))?;

    // Merge updates into existing source
    let mut source_json =
        serde_json::to_value(&profile.sources[source_idx]).map_err(|e| e.to_string())?;
    if let (Some(obj), Some(upd)) = (source_json.as_object_mut(), updates.as_object()) {
        for (k, v) in upd {
            obj.insert(k.clone(), v.clone());
        }
    }
    profile.sources[source_idx] =
        serde_json::from_value(source_json).map_err(|e| format!("Failed to update source: {}", e))?;

    // Ensure source has an audio config entry (may be missing from old profiles)
    profile.source_audio_configs
        .entry(source_id.clone())
        .or_insert_with(SourceAudioConfig::default);

    let settings = state.settings_manager.load()?;
    state
        .profile_manager
        .save_with_key_encryption(&profile, password.as_deref(), settings.encrypt_stream_keys)
        .await?;

    state.event_bus.emit(
        "source_updated",
        json!({ "profileName": profile_name, "sourceId": source_id }),
    );
    Ok(json!(profile.sources[source_idx]))
}

async fn handle_remove_source(state: &AppState, payload: &Value) -> Result<Value, String> {
    let profile_name: String = get_arg(payload, "profileName")?;
    let source_id: String = get_arg(payload, "sourceId")?;
    let password: Option<String> = get_opt_arg(payload, "password")?;
    // If true, also remove linked audio sources. If false and linked sources exist,
    // return a confirmation request instead of deleting.
    let remove_linked: Option<bool> = get_opt_arg(payload, "removeLinked")?;

    let mut profile = state
        .profile_manager
        .load_with_key_decryption(&profile_name, password.as_deref())
        .await?;

    // Find the source first (before any deletion)
    if !profile.sources.iter().any(|s| s.id() == source_id) {
        return Err(format!("Source {} not found", source_id));
    }

    // Find any linked audio sources
    // (AudioDeviceSources that have linked_to_source_id pointing to this source)
    let linked_audio_ids: Vec<String> = profile
        .sources
        .iter()
        .filter_map(|s| {
            if let Source::AudioDevice(ad) = s {
                if ad.linked_to_source_id.as_ref() == Some(&source_id) {
                    return Some(ad.id.clone());
                }
            }
            None
        })
        .collect();

    // If linked sources exist and removeLinked is explicitly false, return confirmation request
    if !linked_audio_ids.is_empty() && remove_linked == Some(false) {
        // Get linked source names for the confirmation dialog
        let linked_names: Vec<String> = profile
            .sources
            .iter()
            .filter(|s| linked_audio_ids.contains(&s.id().to_string()))
            .map(|s| s.name().to_string())
            .collect();

        return Ok(json!({
            "requiresConfirmation": true,
            "linkedSourceIds": linked_audio_ids,
            "linkedSourceNames": linked_names,
            "message": "This source has linked audio sources. Remove both?"
        }));
    }

    // Proceed with deletion
    profile.sources.retain(|s| s.id() != source_id);

    // Remove source-level audio config
    profile.source_audio_configs.remove(&source_id);

    // Stop any running preview for this source
    state.preview_handler.stop_source_preview(&source_id);

    // Stop audio capture if running for this source
    let _ = state.audio_capture.stop_capture_for_source(&source_id);

    // Stop in-process audio decoders and unregister from audio bus
    state.media_audio_decoder.stop(&source_id);
    state.audio_level_service.unregister_audio_source(&source_id);

    // Stop ScreenCaptureKit audio capture if running (macOS only)
    #[cfg(target_os = "macos")]
    let _ = state.sck_audio_capture.stop_capture(&source_id);

    // Determine if we should delete linked sources
    // Default to true (backward compatible) unless explicitly set to false
    let should_remove_linked = remove_linked.unwrap_or(true);

    // Delete linked audio sources if requested
    let removed_linked_ids: Vec<String> =
        if should_remove_linked && !linked_audio_ids.is_empty() {
            profile
                .sources
                .retain(|s| !linked_audio_ids.contains(&s.id().to_string()));

            // Stop previews and audio captures for linked audio sources
            for linked_id in &linked_audio_ids {
                state.preview_handler.stop_source_preview(linked_id);
                // Also stop audio capture and decoders for linked sources
                let _ = state.audio_capture.stop_capture_for_source(linked_id);
                state.media_audio_decoder.stop(linked_id);
                state.audio_level_service.unregister_audio_source(linked_id);
                // Stop ScreenCaptureKit audio capture (macOS only)
                #[cfg(target_os = "macos")]
                let _ = state.sck_audio_capture.stop_capture(linked_id);
                // Remove source-level audio config
                profile.source_audio_configs.remove(linked_id);
            }
            linked_audio_ids.clone()
        } else {
            Vec::new()
        };

    // Also remove from all scenes
    let ids_to_remove: Vec<&String> = std::iter::once(&source_id)
        .chain(removed_linked_ids.iter())
        .collect();

    for scene in &mut profile.scenes {
        scene
            .layers
            .retain(|l| !ids_to_remove.contains(&&l.source_id));
        scene
            .audio_mixer
            .tracks
            .retain(|t| !ids_to_remove.contains(&&t.source_id));
    }

    let settings = state.settings_manager.load()?;
    state
        .profile_manager
        .save_with_key_encryption(&profile, password.as_deref(), settings.encrypt_stream_keys)
        .await?;

    state.event_bus.emit(
        "source_removed",
        json!({
            "profileName": profile_name,
            "sourceId": source_id,
            "linkedAudioSourceIds": removed_linked_ids
        }),
    );

    Ok(json!({
        "removed": true,
        "linkedRemoved": removed_linked_ids
    }))
}

async fn handle_reorder_sources(state: &AppState, payload: &Value) -> Result<Value, String> {
    let profile_name: String = get_arg(payload, "profileName")?;
    let source_ids: Vec<String> = get_arg(payload, "sourceIds")?;
    let password: Option<String> = get_opt_arg(payload, "password")?;

    let mut profile = state
        .profile_manager
        .load_with_key_decryption(&profile_name, password.as_deref())
        .await?;

    // Validate that all IDs exist
    for id in &source_ids {
        if !profile.sources.iter().any(|s| s.id() == *id) {
            return Err(format!("Source {} not found", id));
        }
    }

    // Reorder sources based on the new order
    let mut reordered: Vec<Source> = Vec::with_capacity(source_ids.len());
    for id in &source_ids {
        if let Some(source) = profile.sources.iter().find(|s| s.id() == *id) {
            reordered.push(source.clone());
        }
    }
    // Add any sources not in the list at the end (safety measure)
    for source in &profile.sources {
        if !source_ids.iter().any(|id| id == source.id()) {
            reordered.push(source.clone());
        }
    }
    profile.sources = reordered;

    let settings = state.settings_manager.load()?;
    state
        .profile_manager
        .save_with_key_encryption(&profile, password.as_deref(), settings.encrypt_stream_keys)
        .await?;

    state
        .event_bus
        .emit("sources_reordered", json!({ "profileName": profile_name }));
    Ok(json!(profile.sources))
}
