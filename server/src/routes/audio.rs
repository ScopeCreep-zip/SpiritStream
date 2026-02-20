use axum::{
    extract::{Json, State},
    response::IntoResponse,
};
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;

use crate::app_state::AppState;
use crate::models::Source;
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
                        match state.audio_capture.start_input_capture_for_source(
                            source_id,
                            &audio_source.device_id,
                            config,
                        ) {
                            Ok(_) => {
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
