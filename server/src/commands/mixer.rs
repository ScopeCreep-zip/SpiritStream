// Audio Mixer Commands
// Handles audio mixer operations: per-source volume/mute/solo/balance/monitoring/filters,
// plus scene-level master volume/mute.
//
// OBS pattern: per-source audio config lives on Profile.source_audio_configs (source-level),
// not per-scene. Master volume/mute remains scene-level.

use crate::app_state::{AppState, get_arg, get_opt_arg};
use crate::models::SourceAudioConfig;
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
        "set_source_balance" => Some(set_source_balance(state, payload).await),
        "set_source_monitoring" => Some(set_source_monitoring(state, payload).await),
        "set_source_sync_offset" => Some(set_source_sync_offset(state, payload).await),
        "set_source_track_bitmask" => Some(set_source_track_bitmask(state, payload).await),
        "set_source_fader_curve" => Some(set_source_fader_curve(state, payload).await),
        "set_source_audio_filters" => Some(set_source_audio_filters(state, payload).await),
        "get_source_audio_config" => Some(get_source_audio_config(state, payload).await),
        "get_all_source_audio_configs" => Some(get_all_source_audio_configs(state, payload).await),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Helper: load profile, get mutable source audio config, save
// ---------------------------------------------------------------------------

async fn with_source_audio_config<F>(
    state: &AppState,
    payload: &Value,
    mutate: F,
) -> Result<(String, String, SourceAudioConfig), String>
where
    F: FnOnce(&mut SourceAudioConfig) -> Result<(), String>,
{
    let profile_name: String = get_arg(payload, "profileName")?;
    let source_id: String = get_arg(payload, "sourceId")?;
    let password: Option<String> = get_opt_arg(payload, "password")?;

    let mut profile = state
        .profile_manager
        .load_with_key_decryption(&profile_name, password.as_deref())
        .await?;

    let config = profile
        .source_audio_configs
        .get_mut(&source_id)
        .ok_or_else(|| format!("No audio config for source {}", source_id))?;

    mutate(config)?;

    // Bump config version so the mixer thread detects the change
    config.config_version = config.config_version.wrapping_add(1);

    let config_clone = config.clone();

    let settings = state.settings_manager.load()?;
    state
        .profile_manager
        .save_with_key_encryption(&profile, password.as_deref(), settings.encrypt_stream_keys)
        .await?;

    Ok((profile_name, source_id, config_clone))
}

// ---------------------------------------------------------------------------
// Per-source commands (audio config is source-global, no sceneId needed)
// ---------------------------------------------------------------------------

async fn set_track_volume(state: &AppState, payload: &Value) -> Result<Value, String> {
    let volume: f32 = get_arg(payload, "volume")?;

    let (profile_name, source_id, _config) = with_source_audio_config(state, payload, |config| {
        config.volume = volume.clamp(0.0, 20.0); // OBS range: 0-20x
        Ok(())
    }).await?;

    state.event_bus.emit("source_audio_changed", json!({
        "profileName": profile_name,
        "sourceId": source_id,
        "volume": volume,
    }));
    Ok(Value::Null)
}

async fn set_track_muted(state: &AppState, payload: &Value) -> Result<Value, String> {
    let muted: bool = get_arg(payload, "muted")?;

    let (profile_name, source_id, _config) = with_source_audio_config(state, payload, |config| {
        config.muted = muted;
        Ok(())
    }).await?;

    // Mute is now handled by the audio engine's mixer thread:
    // audio_config.muted → gain=0 → metering naturally reports zeros.

    state.event_bus.emit("source_audio_changed", json!({
        "profileName": profile_name,
        "sourceId": source_id,
        "muted": muted,
    }));
    Ok(Value::Null)
}

async fn set_track_solo(state: &AppState, payload: &Value) -> Result<Value, String> {
    let solo: bool = get_arg(payload, "solo")?;

    let (profile_name, source_id, _config) = with_source_audio_config(state, payload, |config| {
        config.solo = solo;
        Ok(())
    }).await?;

    state.event_bus.emit("source_audio_changed", json!({
        "profileName": profile_name,
        "sourceId": source_id,
        "solo": solo,
    }));
    Ok(Value::Null)
}

async fn set_source_balance(state: &AppState, payload: &Value) -> Result<Value, String> {
    let balance: f32 = get_arg(payload, "balance")?;

    let (profile_name, source_id, _config) = with_source_audio_config(state, payload, |config| {
        config.balance = balance.clamp(-1.0, 1.0);
        Ok(())
    }).await?;

    state.event_bus.emit("source_audio_changed", json!({
        "profileName": profile_name,
        "sourceId": source_id,
        "balance": balance,
    }));
    Ok(Value::Null)
}

async fn set_source_monitoring(state: &AppState, payload: &Value) -> Result<Value, String> {
    let monitoring_type: crate::models::MonitoringType = get_arg(payload, "monitoringType")?;

    let (profile_name, source_id, _config) = with_source_audio_config(state, payload, |config| {
        config.monitoring_type = monitoring_type;
        Ok(())
    }).await?;

    state.event_bus.emit("source_audio_changed", json!({
        "profileName": profile_name,
        "sourceId": source_id,
        "monitoringType": monitoring_type,
    }));
    Ok(Value::Null)
}

async fn set_source_sync_offset(state: &AppState, payload: &Value) -> Result<Value, String> {
    let sync_offset_ms: i64 = get_arg(payload, "syncOffsetMs")?;

    let (profile_name, source_id, _config) = with_source_audio_config(state, payload, |config| {
        config.sync_offset_ms = sync_offset_ms;
        Ok(())
    }).await?;

    state.event_bus.emit("source_audio_changed", json!({
        "profileName": profile_name,
        "sourceId": source_id,
        "syncOffsetMs": sync_offset_ms,
    }));
    Ok(Value::Null)
}

async fn set_source_track_bitmask(state: &AppState, payload: &Value) -> Result<Value, String> {
    let track_bitmask: u8 = get_arg(payload, "trackBitmask")?;

    let (profile_name, source_id, _config) = with_source_audio_config(state, payload, |config| {
        config.track_bitmask = track_bitmask & 0b111111; // 6 tracks max
        Ok(())
    }).await?;

    state.event_bus.emit("source_audio_changed", json!({
        "profileName": profile_name,
        "sourceId": source_id,
        "trackBitmask": track_bitmask,
    }));
    Ok(Value::Null)
}

async fn set_source_fader_curve(state: &AppState, payload: &Value) -> Result<Value, String> {
    let fader_curve: crate::models::FaderCurve = get_arg(payload, "faderCurve")?;

    let (profile_name, source_id, _config) = with_source_audio_config(state, payload, |config| {
        config.fader_curve = fader_curve;
        Ok(())
    }).await?;

    state.event_bus.emit("source_audio_changed", json!({
        "profileName": profile_name,
        "sourceId": source_id,
        "faderCurve": fader_curve,
    }));
    Ok(Value::Null)
}

async fn set_source_audio_filters(state: &AppState, payload: &Value) -> Result<Value, String> {
    let filters: Vec<crate::models::AudioFilterConfig> = get_arg(payload, "audioFilters")?;

    let (profile_name, source_id, _config) = with_source_audio_config(state, payload, |config| {
        config.audio_filters = filters.clone();
        Ok(())
    }).await?;

    state.event_bus.emit("source_audio_changed", json!({
        "profileName": profile_name,
        "sourceId": source_id,
        "audioFilters": filters,
    }));
    Ok(Value::Null)
}

async fn get_source_audio_config(state: &AppState, payload: &Value) -> Result<Value, String> {
    let profile_name: String = get_arg(payload, "profileName")?;
    let source_id: String = get_arg(payload, "sourceId")?;
    let password: Option<String> = get_opt_arg(payload, "password")?;

    let profile = state
        .profile_manager
        .load_with_key_decryption(&profile_name, password.as_deref())
        .await?;

    let config = profile
        .source_audio_configs
        .get(&source_id)
        .ok_or_else(|| format!("No audio config for source {}", source_id))?;

    serde_json::to_value(config).map_err(|e| e.to_string())
}

async fn get_all_source_audio_configs(state: &AppState, payload: &Value) -> Result<Value, String> {
    let profile_name: String = get_arg(payload, "profileName")?;
    let password: Option<String> = get_opt_arg(payload, "password")?;

    let profile = state
        .profile_manager
        .load_with_key_decryption(&profile_name, password.as_deref())
        .await?;

    serde_json::to_value(&profile.source_audio_configs).map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// Scene-level master controls (unchanged from before)
// ---------------------------------------------------------------------------

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
