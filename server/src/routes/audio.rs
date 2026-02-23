use axum::{
    extract::{Json, Path, State},
    response::IntoResponse,
};
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;

use crate::app_state::AppState;
use crate::models::{AudioFilterConfig, Source};
use crate::services::AudioCaptureConfig;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SetAudioLevelSourcesRequest {
    source_ids: Vec<String>,
    profile_name: Option<String>,
}

/// POST /api/audio-levels/start - Start audio level monitoring with source list
pub(crate) async fn audio_levels_start_handler(
    State(state): State<AppState>,
    Json(req): Json<SetAudioLevelSourcesRequest>,
) -> impl IntoResponse {
    log::info!(
        "[AudioLevels] Starting monitoring for {} sources",
        req.source_ids.len()
    );

    // Set tracked sources
    state
        .audio_level_service
        .set_tracked_sources(req.source_ids.clone());

    // If profile name provided, start audio capture for device sources
    let mut capture_results = std::collections::HashMap::new();
    if let Some(profile_name) = &req.profile_name {
        // Load the profile asynchronously
        if let Ok(profile) = state.profile_manager.load(profile_name, None).await {
            for source_id in &req.source_ids {
                if let Some(source) = profile.sources.iter().find(|s| s.id() == source_id) {
                    if let Source::AudioDevice(audio_source) = source {
                        // Start audio capture for this device
                        let config = AudioCaptureConfig {
                            sample_rate: Some(audio_source.sample_rate.unwrap_or(48000)),
                            channels: Some(audio_source.channels.unwrap_or(2) as u16),
                        };

                        // Use start_input_capture_for_source which tracks the source mapping
                        let sample_rate = audio_source.sample_rate.unwrap_or(48000);
                        let channels = audio_source.channels.unwrap_or(2) as u16;
                        match state.audio_capture.start_input_capture_for_source(
                            source_id,
                            &audio_source.device_id,
                            config,
                        ) {
                            Ok(receiver) => {
                                // Bridge to audio engine for mixing
                                let engine_rx = receiver.resubscribe();
                                state.audio_engine.register_source_from_broadcast(
                                    source_id,
                                    sample_rate,
                                    channels,
                                    engine_rx,
                                );

                                // Register with unified audio bus for metering
                                state.audio_level_service.register_audio_source(
                                    source_id,
                                    receiver,
                                );

                                capture_results.insert(
                                    source_id.clone(),
                                    json!({ "success": true }),
                                );
                            }
                            Err(e) => {
                                capture_results.insert(
                                    source_id.clone(),
                                    json!({ "success": false, "error": e }),
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    // Start the monitoring loop if not already running
    if !state.audio_level_service.is_running() {
        let idle_flag = state.native_preview.idle_flag();
        let throttle_flag = state.power_budget.throttle_flag();
        state
            .audio_level_service
            .start(Arc::new(state.event_bus.clone()), idle_flag, throttle_flag);
    }

    Json(json!({
        "ok": true,
        "data": {
            "running": true,
            "trackedSources": req.source_ids.len(),
            "captureResults": capture_results
        }
    }))
}

/// POST /api/audio-levels/stop - Stop audio level monitoring
pub(crate) async fn audio_levels_stop_handler(State(state): State<AppState>) -> impl IntoResponse {
    log::info!("[AudioLevels] Stopping monitoring");
    state.audio_level_service.stop();

    // Clear tracked sources
    state.audio_level_service.set_tracked_sources(vec![]);

    Json(json!({
        "ok": true,
        "data": { "running": false }
    }))
}

/// GET /api/audio-levels/health - Get audio level monitoring health status
pub(crate) async fn audio_levels_health_handler(State(state): State<AppState>) -> impl IntoResponse {
    let running = state.audio_level_service.is_running();
    let health = state.audio_level_service.get_health_status();

    Json(json!({
        "ok": true,
        "data": {
            "running": running,
            "sources": health
        }
    }))
}

// ============================================================================
// Audio Filter API
// ============================================================================

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SetFiltersRequest {
    profile_name: String,
    filters: Vec<AudioFilterConfig>,
    password: Option<String>,
}

/// POST /api/audio/filters/:source_id - Set audio filters for a source
pub(crate) async fn set_audio_filters_handler(
    State(state): State<AppState>,
    Path(source_id): Path<String>,
    Json(req): Json<SetFiltersRequest>,
) -> impl IntoResponse {
    // Load profile
    let mut profile = match state
        .profile_manager
        .load_with_key_decryption(&req.profile_name, req.password.as_deref())
        .await
    {
        Ok(p) => p,
        Err(e) => {
            return Json(json!({ "ok": false, "error": format!("Failed to load profile: {}", e) }));
        }
    };

    // Update filters in profile
    {
        let config = profile
            .source_audio_configs
            .entry(source_id.clone())
            .or_insert_with(crate::models::SourceAudioConfig::default);
        config.audio_filters = req.filters.clone();
    }

    // Persist
    let settings = state.settings_manager.load().ok();
    let encrypt = settings.map(|s| s.encrypt_stream_keys).unwrap_or(false);
    if let Err(e) = state
        .profile_manager
        .save_with_key_encryption(&profile, req.password.as_deref(), encrypt)
        .await
    {
        return Json(json!({ "ok": false, "error": format!("Failed to save profile: {}", e) }));
    }

    // Update runtime engine config so mixer thread picks up new filters
    if let Some(updated_config) = profile.source_audio_configs.get(&source_id) {
        state
            .audio_engine
            .update_source_config(&source_id, updated_config.clone());
    }

    state.event_bus.sender.send(crate::app_state::ServerEvent::Json {
        event: "source_audio_changed".to_string(),
        payload: json!({
            "sourceId": source_id,
            "filters": req.filters.len()
        }),
    }).ok();

    log::info!(
        "[AudioFilters] Set {} filters for source '{}'",
        req.filters.len(),
        source_id
    );

    Json(json!({ "ok": true, "data": { "filterCount": req.filters.len() } }))
}

/// GET /api/audio/filters/:source_id - Get audio filters for a source
pub(crate) async fn get_audio_filters_handler(
    State(state): State<AppState>,
    Path(source_id): Path<String>,
) -> impl IntoResponse {
    let config = state.audio_engine.get_source_config(&source_id);
    let filters = config
        .map(|c| c.audio_filters)
        .unwrap_or_default();

    Json(json!({ "ok": true, "data": { "filters": filters } }))
}
