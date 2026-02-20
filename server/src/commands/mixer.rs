// Audio Mixer Commands
// Handles audio mixer operations: track volume/mute/solo, master volume/mute

use crate::app_state::{AppState, get_arg, get_opt_arg};
use crate::services::EventSink;
use serde_json::{json, Value};

/// Handle audio mixer commands.
///
/// Returns `None` if the command is not recognized by this module,
/// or `Some(result)` if it was handled.
pub async fn handle(state: &AppState, command: &str, payload: &Value) -> Option<Result<Value, String>> {
    match command {
        "set_track_volume" => Some(set_track_volume(state, payload).await),
        "set_track_muted" => Some(set_track_muted(state, payload).await),
        "set_track_solo" => Some(set_track_solo(state, payload).await),
        "set_master_volume" => Some(set_master_volume(state, payload).await),
        "set_master_muted" => Some(set_master_muted(state, payload).await),
        _ => None,
    }
}

async fn set_track_volume(state: &AppState, payload: &Value) -> Result<Value, String> {
    let profile_name: String = get_arg(payload, "profileName")?;
    let scene_id: String = get_arg(payload, "sceneId")?;
    let source_id: String = get_arg(payload, "sourceId")?;
    let volume: f32 = get_arg(payload, "volume")?;
    let password: Option<String> = get_opt_arg(payload, "password")?;

    let mut profile = state
        .profile_manager
        .load_with_key_decryption(&profile_name, password.as_deref())
        .await?;

    let scene = profile.scenes.iter_mut().find(|s| s.id == scene_id)
        .ok_or_else(|| format!("Scene {} not found", scene_id))?;

    let track = scene.audio_mixer.tracks.iter_mut().find(|t| t.source_id == source_id)
        .ok_or_else(|| format!("Audio track for source {} not found", source_id))?;

    track.volume = volume.clamp(0.0, 2.0);

    let settings = state.settings_manager.load()?;
    state
        .profile_manager
        .save_with_key_encryption(&profile, password.as_deref(), settings.encrypt_stream_keys)
        .await?;

    state.event_bus.emit("track_volume_changed", json!({ "profileName": profile_name, "sceneId": scene_id, "sourceId": source_id, "volume": volume }));
    Ok(Value::Null)
}

async fn set_track_muted(state: &AppState, payload: &Value) -> Result<Value, String> {
    let profile_name: String = get_arg(payload, "profileName")?;
    let scene_id: String = get_arg(payload, "sceneId")?;
    let source_id: String = get_arg(payload, "sourceId")?;
    let muted: bool = get_arg(payload, "muted")?;
    let password: Option<String> = get_opt_arg(payload, "password")?;

    let mut profile = state
        .profile_manager
        .load_with_key_decryption(&profile_name, password.as_deref())
        .await?;

    let scene = profile.scenes.iter_mut().find(|s| s.id == scene_id)
        .ok_or_else(|| format!("Scene {} not found", scene_id))?;

    let track = scene.audio_mixer.tracks.iter_mut().find(|t| t.source_id == source_id)
        .ok_or_else(|| format!("Audio track for source {} not found", source_id))?;

    track.muted = muted;

    let settings = state.settings_manager.load()?;
    state
        .profile_manager
        .save_with_key_encryption(&profile, password.as_deref(), settings.encrypt_stream_keys)
        .await?;

    // Mute = gain multiplier in AudioLevelService. Decode keeps running.
    // register_audio_source()'s background task checks muted_sources and reports zeros.
    // Unmute = instant gain restore, no process respawn needed.
    state.audio_level_service.set_source_mute(&source_id, muted);

    state.event_bus.emit("track_muted_changed", json!({ "profileName": profile_name, "sceneId": scene_id, "sourceId": source_id, "muted": muted }));
    Ok(Value::Null)
}

async fn set_track_solo(state: &AppState, payload: &Value) -> Result<Value, String> {
    let profile_name: String = get_arg(payload, "profileName")?;
    let scene_id: String = get_arg(payload, "sceneId")?;
    let source_id: String = get_arg(payload, "sourceId")?;
    let solo: bool = get_arg(payload, "solo")?;
    let password: Option<String> = get_opt_arg(payload, "password")?;

    let mut profile = state
        .profile_manager
        .load_with_key_decryption(&profile_name, password.as_deref())
        .await?;

    let scene = profile.scenes.iter_mut().find(|s| s.id == scene_id)
        .ok_or_else(|| format!("Scene {} not found", scene_id))?;

    let track = scene.audio_mixer.tracks.iter_mut().find(|t| t.source_id == source_id)
        .ok_or_else(|| format!("Audio track for source {} not found", source_id))?;

    track.solo = solo;

    let settings = state.settings_manager.load()?;
    state
        .profile_manager
        .save_with_key_encryption(&profile, password.as_deref(), settings.encrypt_stream_keys)
        .await?;

    state.event_bus.emit("track_solo_changed", json!({ "profileName": profile_name, "sceneId": scene_id, "sourceId": source_id, "solo": solo }));
    Ok(Value::Null)
}

async fn set_master_volume(state: &AppState, payload: &Value) -> Result<Value, String> {
    let profile_name: String = get_arg(payload, "profileName")?;
    let scene_id: String = get_arg(payload, "sceneId")?;
    let volume: f32 = get_arg(payload, "volume")?;
    let password: Option<String> = get_opt_arg(payload, "password")?;

    let mut profile = state
        .profile_manager
        .load_with_key_decryption(&profile_name, password.as_deref())
        .await?;

    let scene = profile.scenes.iter_mut().find(|s| s.id == scene_id)
        .ok_or_else(|| format!("Scene {} not found", scene_id))?;

    scene.audio_mixer.master_volume = volume.clamp(0.0, 2.0);

    let settings = state.settings_manager.load()?;
    state
        .profile_manager
        .save_with_key_encryption(&profile, password.as_deref(), settings.encrypt_stream_keys)
        .await?;

    state.event_bus.emit("master_volume_changed", json!({ "profileName": profile_name, "sceneId": scene_id, "volume": volume }));
    Ok(Value::Null)
}

async fn set_master_muted(state: &AppState, payload: &Value) -> Result<Value, String> {
    let profile_name: String = get_arg(payload, "profileName")?;
    let scene_id: String = get_arg(payload, "sceneId")?;
    let muted: bool = get_arg(payload, "muted")?;
    let password: Option<String> = get_opt_arg(payload, "password")?;

    let mut profile = state
        .profile_manager
        .load_with_key_decryption(&profile_name, password.as_deref())
        .await?;

    let scene = profile.scenes.iter_mut().find(|s| s.id == scene_id)
        .ok_or_else(|| format!("Scene {} not found", scene_id))?;

    scene.audio_mixer.master_muted = muted;

    let settings = state.settings_manager.load()?;
    state
        .profile_manager
        .save_with_key_encryption(&profile, password.as_deref(), settings.encrypt_stream_keys)
        .await?;

    state.event_bus.emit("master_muted_changed", json!({ "profileName": profile_name, "sceneId": scene_id, "muted": muted }));
    Ok(Value::Null)
}
