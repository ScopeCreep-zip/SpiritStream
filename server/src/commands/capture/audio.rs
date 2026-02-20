// Audio capture control commands (start/stop audio capture, device selection)
// Handles the set_audio_monitor_sources command which orchestrates capture for all source types.
//
// Unified Audio Bus: All source types produce broadcast::Receiver<AudioBuffer>,
// registered with AudioLevelService for unified RMS/peak computation.
// No more FFmpeg astats subprocesses.

use std::collections::HashMap;

use serde_json::{json, Value};
use tokio::task::JoinSet;

use crate::app_state::{AppState, get_arg, get_opt_arg};
use crate::models::Source;
use crate::services::{AudioBuffer, AudioCaptureConfig};

/// The big set_audio_monitor_sources handler.
/// Stops captures/extractions for sources no longer in list, loads profile,
/// starts appropriate capture for each source type.
pub(super) async fn handle_set_audio_monitor_sources(state: &AppState, payload: &Value) -> Result<Value, String> {
    let source_ids: Vec<String> = get_arg(&payload, "sourceIds")?;
    let profile_name: Option<String> = get_opt_arg(&payload, "profileName")?;

    let cmd_start = std::time::Instant::now();
    log::info!("=== set_audio_monitor_sources START ===");
    log::info!("Sources requested: {:?}, Profile: {:?}", source_ids, profile_name);

    // Stop captures for sources no longer in the list
    // 1. Stop cpal device captures
    let active_cpal_sources: Vec<String> = state.audio_capture.active_source_ids();
    for source_id in &active_cpal_sources {
        if !source_ids.contains(source_id) {
            log::info!("Stopping cpal audio capture for removed source: {}", source_id);
            let _ = state.audio_capture.stop_capture_for_source(source_id);
        }
    }

    // 2. Stop media audio decoders for removed sources
    for source_id in state.media_audio_decoder.active_ids() {
        if !source_ids.contains(&source_id) {
            log::info!("Stopping media audio decoder for removed source: {}", source_id);
            state.media_audio_decoder.stop(&source_id);
        }
    }

    // 2b. Stop stream audio decoders for removed sources (RTMP, NDI)
    for source_id in state.stream_audio_decoder.active_ids() {
        if !source_ids.contains(&source_id) {
            log::info!("Stopping stream audio decoder for removed source: {}", source_id);
            state.stream_audio_decoder.stop(&source_id);
        }
    }

    // 3. Stop ScreenCaptureKit captures for sources no longer in the list (macOS only)
    #[cfg(target_os = "macos")]
    {
        let active_sck_captures: Vec<String> = state.sck_audio_capture.active_capture_ids();
        for capture_id in &active_sck_captures {
            if !source_ids.contains(capture_id) {
                log::info!("Stopping ScreenCaptureKit audio capture for removed source: {}", capture_id);
                let _ = state.sck_audio_capture.stop_capture(capture_id);
            }
        }
    }

    // 4. Unregister audio bus sources no longer tracked
    state.audio_level_service.unregister_removed_sources(&source_ids);

    // Set tracked sources in the level service
    state.audio_level_service.set_tracked_sources(source_ids.clone());

    // Track capture results for each source to return to frontend
    let mut capture_results: HashMap<String, serde_json::Value> = HashMap::new();

    // If profile name provided, start real audio capture for audio device sources
    if let Some(profile_name) = profile_name {
        let profile_start = std::time::Instant::now();
        log::info!("Loading profile '{}' to start audio capture for {} sources", profile_name, source_ids.len());
        match state.profile_manager.load(&profile_name, None).await {
            Ok(profile) => {
                log::info!("Profile loaded in {:?}", profile_start.elapsed());
                let captures_start = std::time::Instant::now();

                // Collect AudioDevice sources for parallel CPAL capture
                let mut audio_device_captures: Vec<(String, String, String)> = Vec::new();

                for source_id in &source_ids {
                    let source_start = std::time::Instant::now();
                    if let Some(source) = profile.sources.iter().find(|s| s.id() == source_id) {
                        let source_type = match source {
                            Source::AudioDevice(_) => "AudioDevice",
                            Source::Rtmp(_) => "Rtmp",
                            Source::Camera(_) => "Camera",
                            Source::ScreenCapture(_) => "ScreenCapture",
                            Source::MediaFile(_) => "MediaFile",
                            _ => "Other",
                        };
                        log::info!("[TIMING] Processing source '{}' (type: {})", source_id, source_type);

                        let capture_device: Option<(String, String)> = match source {
                            Source::AudioDevice(audio_source) => {
                                log::info!("Source '{}' is AudioDevice with device_id: '{}', name: '{}'",
                                    source_id, audio_source.device_id, audio_source.name);
                                Some((audio_source.device_id.clone(), audio_source.name.clone()))
                            }
                            Source::ScreenCapture(screen_source) if screen_source.capture_audio => {
                                #[cfg(target_os = "macos")]
                                {
                                    // macOS: Use ScreenCaptureKit → broadcast → register
                                    // Pass display_id as string (CGDirectDisplayID for new profiles,
                                    // AVFoundation index for old) + device_name for migration fallback
                                    let (tx, _) = tokio::sync::broadcast::channel::<AudioBuffer>(16);
                                    match state.sck_audio_capture.start_display_audio_capture(
                                        &source_id,
                                        &screen_source.display_id,
                                        screen_source.device_name.as_deref(),
                                        tx.clone(),
                                    ) {
                                        Ok(()) => {
                                            state.audio_level_service.register_audio_source(
                                                &source_id,
                                                tx.subscribe(),
                                            );
                                            capture_results.insert(source_id.clone(), json!({
                                                "success": true,
                                                "sourceType": "ScreenCapture",
                                                "displayId": screen_source.display_id,
                                                "method": "ScreenCaptureKit"
                                            }));
                                            log::info!("✓ Audio capture STARTED for ScreenCapture source '{}' via ScreenCaptureKit", source_id);
                                        }
                                        Err(e) => {
                                            capture_results.insert(source_id.clone(), json!({
                                                "success": false,
                                                "sourceType": "ScreenCapture",
                                                "reason": "captureError",
                                                "message": format!("Screen audio capture failed: {}. Make sure screen recording permission is granted.", e)
                                            }));
                                            log::warn!("✗ ScreenCaptureKit audio capture FAILED for source '{}': {}", source_id, e);
                                        }
                                    }
                                }

                                #[cfg(not(target_os = "macos"))]
                                {
                                    // Windows/Linux: Use cpal loopback or PipeWire monitor
                                    match start_system_audio_capture(state, source_id, "ScreenCapture") {
                                        Ok(()) => {
                                            capture_results.insert(source_id.clone(), json!({
                                                "success": true,
                                                "sourceType": "ScreenCapture",
                                                "displayId": screen_source.display_id
                                            }));
                                        }
                                        Err(e) => {
                                            capture_results.insert(source_id.clone(), json!({
                                                "success": false,
                                                "sourceType": "ScreenCapture",
                                                "reason": "captureError",
                                                "message": format!("Screen audio capture unavailable: {}", e)
                                            }));
                                        }
                                    }
                                }
                                None
                            }
                            Source::MediaFile(media_source) => {
                                // Decode audio in-process using symphonia
                                let file_path = media_source.file_path.clone();
                                let looping = media_source.loop_playback;

                                match state.media_audio_decoder.start_decode(
                                    &source_id,
                                    &file_path,
                                    looping,
                                ) {
                                    Ok(rx) => {
                                        state.audio_level_service.register_audio_source(
                                            &source_id,
                                            rx,
                                        );
                                        capture_results.insert(source_id.clone(), json!({
                                            "success": true,
                                            "sourceType": "MediaFile",
                                            "filePath": file_path
                                        }));
                                        log::info!("✓ Audio decode STARTED for MediaFile source '{}' via symphonia", source_id);
                                    }
                                    Err(e) => {
                                        let reason = if e.contains("not a supported media format") {
                                            "unsupportedFormat"
                                        } else {
                                            "decodeFailed"
                                        };
                                        capture_results.insert(source_id.clone(), json!({
                                            "success": false,
                                            "sourceType": "MediaFile",
                                            "reason": reason,
                                            "message": format!("Failed to start audio decode: {}", e)
                                        }));
                                        log::warn!("✗ Audio decode FAILED for MediaFile source '{}': {}", source_id, e);
                                    }
                                }
                                None
                            }
                            Source::Rtmp(rtmp_source) if rtmp_source.capture_audio => {
                                // RTMP audio: go2rtc receives the RTMP stream → we tap AAC via HTTP API → symphonia decodes
                                let rtmp_url = format!(
                                    "rtmp://{}:{}/{}",
                                    rtmp_source.bind_address,
                                    rtmp_source.port,
                                    rtmp_source.application
                                );
                                let go2rtc_url = format!("http://127.0.0.1:{}", state.go2rtc_manager.port());
                                let stream_name = source_id.clone();

                                match state.stream_audio_decoder.start_decode(
                                    &source_id,
                                    &go2rtc_url,
                                    &stream_name,
                                    &rtmp_url,
                                ) {
                                    Ok(rx) => {
                                        state.audio_level_service.register_audio_source(
                                            &source_id,
                                            rx,
                                        );
                                        capture_results.insert(source_id.clone(), json!({
                                            "success": true,
                                            "sourceType": "Rtmp",
                                            "rtmpUrl": rtmp_url
                                        }));
                                        log::info!("✓ Audio decode STARTED for RTMP source '{}' via go2rtc+symphonia", source_id);
                                    }
                                    Err(e) => {
                                        capture_results.insert(source_id.clone(), json!({
                                            "success": false,
                                            "sourceType": "Rtmp",
                                            "reason": "decodeFailed",
                                            "message": format!("Failed to start RTMP audio decode: {}", e)
                                        }));
                                        log::warn!("✗ Audio decode FAILED for RTMP source '{}': {}", source_id, e);
                                    }
                                }
                                None
                            }
                            Source::CaptureCard(card_source) if card_source.capture_audio => {
                                // Capture card audio via cpal (exposed as OS audio device)
                                let device_id = card_source.device_id.clone();
                                match state.audio_capture.start_input_capture_for_source(
                                    source_id,
                                    &device_id,
                                    AudioCaptureConfig::default(),
                                ) {
                                    Ok(rx) => {
                                        state.audio_level_service.register_audio_source(
                                            source_id,
                                            rx,
                                        );
                                        capture_results.insert(source_id.clone(), json!({
                                            "success": true,
                                            "sourceType": "CaptureCard",
                                            "deviceId": device_id
                                        }));
                                        log::info!("✓ Audio capture STARTED for CaptureCard source '{}' via cpal", source_id);
                                    }
                                    Err(e) => {
                                        capture_results.insert(source_id.clone(), json!({
                                            "success": false,
                                            "sourceType": "CaptureCard",
                                            "reason": "captureFailed",
                                            "message": format!("Failed to start capture card audio: {}", e)
                                        }));
                                        log::warn!("✗ Audio capture FAILED for CaptureCard source '{}': {}", source_id, e);
                                    }
                                }
                                None
                            }
                            Source::WindowCapture(win_source) if win_source.capture_audio => {
                                let window_id = win_source.window_id.clone();

                                #[cfg(target_os = "macos")]
                                {
                                    // macOS: Use ScreenCaptureKit for window audio capture
                                    let window_id_num = window_id.parse::<u32>().unwrap_or(0);
                                    let (tx, _) = tokio::sync::broadcast::channel::<AudioBuffer>(16);

                                    match state.sck_audio_capture.start_window_audio_capture(
                                        &source_id,
                                        window_id_num,
                                        tx.clone(),
                                    ) {
                                        Ok(()) => {
                                            state.audio_level_service.register_audio_source(
                                                &source_id,
                                                tx.subscribe(),
                                            );
                                            capture_results.insert(source_id.clone(), json!({
                                                "success": true,
                                                "sourceType": "WindowCapture",
                                                "windowId": window_id,
                                                "method": "ScreenCaptureKit"
                                            }));
                                            log::info!("✓ Audio capture STARTED for WindowCapture source '{}' via ScreenCaptureKit", source_id);
                                        }
                                        Err(e) => {
                                            capture_results.insert(source_id.clone(), json!({
                                                "success": false,
                                                "sourceType": "WindowCapture",
                                                "reason": "captureError",
                                                "message": format!("Window audio capture failed: {}", e)
                                            }));
                                            log::warn!("✗ ScreenCaptureKit audio capture FAILED for WindowCapture source '{}': {}", source_id, e);
                                        }
                                    }
                                }

                                #[cfg(not(target_os = "macos"))]
                                {
                                    // Windows/Linux: Use system audio capture
                                    match start_system_audio_capture(state, source_id, "WindowCapture") {
                                        Ok(()) => {
                                            capture_results.insert(source_id.clone(), json!({
                                                "success": true,
                                                "sourceType": "WindowCapture",
                                                "windowId": window_id
                                            }));
                                        }
                                        Err(e) => {
                                            capture_results.insert(source_id.clone(), json!({
                                                "success": false,
                                                "sourceType": "WindowCapture",
                                                "reason": "captureError",
                                                "message": format!("Window audio capture unavailable: {}", e)
                                            }));
                                        }
                                    }
                                }
                                None
                            }
                            Source::GameCapture(game_source) if game_source.capture_audio => {
                                #[cfg(target_os = "macos")]
                                {
                                    // macOS: Use ScreenCaptureKit for system audio capture (game audio)
                                    let (tx, _) = tokio::sync::broadcast::channel::<AudioBuffer>(16);

                                    match state.sck_audio_capture.start_system_audio_capture(
                                        &source_id,
                                        tx.clone(),
                                    ) {
                                        Ok(()) => {
                                            state.audio_level_service.register_audio_source(
                                                &source_id,
                                                tx.subscribe(),
                                            );
                                            capture_results.insert(source_id.clone(), json!({
                                                "success": true,
                                                "sourceType": "GameCapture",
                                                "method": "ScreenCaptureKit"
                                            }));
                                            log::info!("✓ Audio capture STARTED for GameCapture source '{}' via ScreenCaptureKit", source_id);
                                        }
                                        Err(e) => {
                                            capture_results.insert(source_id.clone(), json!({
                                                "success": false,
                                                "sourceType": "GameCapture",
                                                "reason": "captureError",
                                                "message": format!("Game audio capture failed: {}", e)
                                            }));
                                            log::warn!("✗ ScreenCaptureKit audio capture FAILED for GameCapture source '{}': {}", source_id, e);
                                        }
                                    }
                                }

                                #[cfg(not(target_os = "macos"))]
                                {
                                    // Windows/Linux: Use system audio capture
                                    match start_system_audio_capture(state, source_id, "GameCapture") {
                                        Ok(()) => {
                                            capture_results.insert(source_id.clone(), json!({
                                                "success": true,
                                                "sourceType": "GameCapture"
                                            }));
                                        }
                                        Err(e) => {
                                            capture_results.insert(source_id.clone(), json!({
                                                "success": false,
                                                "sourceType": "GameCapture",
                                                "reason": "captureError",
                                                "message": format!("Game audio capture unavailable: {}", e)
                                            }));
                                        }
                                    }
                                }
                                None
                            }
                            Source::MediaPlaylist(playlist_source) => {
                                // Decode audio from current playlist item using symphonia
                                if let Some(current_item) = playlist_source.items.get(playlist_source.current_item_index) {
                                    let file_path = current_item.file_path.clone();
                                    match state.media_audio_decoder.start_decode(
                                        &source_id,
                                        &file_path,
                                        false, // playlist manages transitions, not looping per-item
                                    ) {
                                        Ok(rx) => {
                                            state.audio_level_service.register_audio_source(
                                                &source_id,
                                                rx,
                                            );
                                            capture_results.insert(source_id.clone(), json!({
                                                "success": true,
                                                "sourceType": "MediaPlaylist",
                                                "currentFile": file_path
                                            }));
                                            log::info!("✓ Audio decode STARTED for MediaPlaylist source '{}' via symphonia", source_id);
                                        }
                                        Err(e) => {
                                            capture_results.insert(source_id.clone(), json!({
                                                "success": false,
                                                "sourceType": "MediaPlaylist",
                                                "reason": "decodeFailed",
                                                "message": format!("Failed to start audio decode: {}", e)
                                            }));
                                            log::warn!("✗ Audio decode FAILED for MediaPlaylist source '{}': {}", source_id, e);
                                        }
                                    }
                                } else {
                                    capture_results.insert(source_id.clone(), json!({
                                        "success": false,
                                        "sourceType": "MediaPlaylist",
                                        "reason": "noCurrentItem",
                                        "message": "Playlist has no current item"
                                    }));
                                }
                                None
                            }
                            Source::Ndi(ndi_source) if ndi_source.capture_audio => {
                                // NDI audio: register as FFmpeg source in go2rtc → tap AAC via HTTP API → symphonia decodes
                                let ndi_ffmpeg_source = format!(
                                    "ffmpeg:-f libndi_newtek -i \"{}\"#audio=aac",
                                    ndi_source.source_name
                                );
                                let go2rtc_url = format!("http://127.0.0.1:{}", state.go2rtc_manager.port());
                                let stream_name = format!("ndi_{}", source_id);

                                match state.stream_audio_decoder.start_decode(
                                    &source_id,
                                    &go2rtc_url,
                                    &stream_name,
                                    &ndi_ffmpeg_source,
                                ) {
                                    Ok(rx) => {
                                        state.audio_level_service.register_audio_source(
                                            &source_id,
                                            rx,
                                        );
                                        capture_results.insert(source_id.clone(), json!({
                                            "success": true,
                                            "sourceType": "Ndi",
                                            "sourceName": ndi_source.source_name
                                        }));
                                        log::info!("✓ Audio decode STARTED for NDI source '{}' via go2rtc+symphonia", source_id);
                                    }
                                    Err(e) => {
                                        capture_results.insert(source_id.clone(), json!({
                                            "success": false,
                                            "sourceType": "Ndi",
                                            "reason": "decodeFailed",
                                            "message": format!("Failed to start NDI audio decode: {}. Ensure FFmpeg has libndi_newtek support.", e)
                                        }));
                                        log::warn!("✗ Audio decode FAILED for NDI source '{}': {}", source_id, e);
                                    }
                                }
                                None
                            }
                            _ => {
                                capture_results.insert(source_id.clone(), json!({
                                    "success": false,
                                    "sourceType": source_type,
                                    "reason": "noAudio",
                                    "message": "Source type does not support audio capture"
                                }));
                                None
                            }
                        };

                        // Collect AudioDevice sources for parallel CPAL capture later
                        if let Some((device_id, device_name)) = capture_device {
                            if state.audio_capture.is_capturing_source(source_id) {
                                log::info!("Already capturing audio for source '{}', skipping", source_id);
                                capture_results.insert(source_id.clone(), json!({
                                    "success": true,
                                    "sourceType": "AudioDevice",
                                    "deviceName": device_name,
                                    "alreadyCapturing": true
                                }));
                            } else {
                                audio_device_captures.push((source_id.clone(), device_id, device_name));
                            }
                        }
                        log::info!("[TIMING] Source '{}' completed in {:?}", source_id, source_start.elapsed());
                    } else {
                        capture_results.insert(source_id.clone(), json!({
                            "success": false,
                            "reason": "notFound",
                            "message": "Source not found in profile"
                        }));
                        log::info!("[TIMING] Source '{}' not found, took {:?}", source_id, source_start.elapsed());
                    }
                }

                // ============================================================
                // PARALLEL CPAL CAPTURE: Run all AudioDevice captures in parallel
                // ============================================================
                if !audio_device_captures.is_empty() {
                    log::info!("[TIMING] Starting {} AudioDevice captures in PARALLEL", audio_device_captures.len());
                    let parallel_start = std::time::Instant::now();

                    let mut capture_set: JoinSet<(String, String, Result<tokio::sync::broadcast::Receiver<AudioBuffer>, String>)> = JoinSet::new();

                    for (source_id, device_id, device_name) in audio_device_captures {
                        let audio_capture = state.audio_capture.clone();
                        let source_id_spawn = source_id.clone();
                        let device_id_spawn = device_id.clone();
                        let device_name_spawn = device_name.clone();

                        capture_set.spawn_blocking(move || {
                            log::info!("[PARALLEL] Starting capture for source '{}' (device: '{}')", source_id_spawn, device_name_spawn);
                            let capture_start = std::time::Instant::now();

                            let result = audio_capture.start_input_capture_for_source(
                                &source_id_spawn,
                                &device_name_spawn,
                                AudioCaptureConfig::default(),
                            ).or_else(|e| {
                                log::info!("[PARALLEL] Device name '{}' failed ({}), trying device_id '{}'", device_name_spawn, e, device_id_spawn);
                                audio_capture.start_input_capture_for_source(
                                    &source_id_spawn,
                                    &device_id_spawn,
                                    AudioCaptureConfig::default(),
                                )
                            });

                            log::info!("[PARALLEL] Capture for '{}' completed in {:?}", source_id_spawn, capture_start.elapsed());
                            (source_id_spawn, device_name_spawn, result)
                        });
                    }

                    // Collect all results
                    while let Some(join_result) = capture_set.join_next().await {
                        match join_result {
                            Ok((source_id, device_name, capture_result)) => {
                                match capture_result {
                                    Ok(receiver) => {
                                        log::info!("✓ Audio capture STARTED for source '{}' (device: '{}')", source_id, device_name);
                                        capture_results.insert(source_id.clone(), json!({
                                            "success": true,
                                            "sourceType": "AudioDevice",
                                            "deviceName": device_name
                                        }));

                                        // Register with unified audio bus
                                        state.audio_level_service.register_audio_source(
                                            &source_id,
                                            receiver,
                                        );
                                    }
                                    Err(e) => {
                                        log::error!("✗ Audio capture FAILED for source '{}' (device: '{}'): {}", source_id, device_name, e);
                                        capture_results.insert(source_id.clone(), json!({
                                            "success": false,
                                            "sourceType": "AudioDevice",
                                            "deviceName": device_name,
                                            "reason": "captureFailed",
                                            "message": format!("Failed to capture from device: {}", e)
                                        }));
                                    }
                                }
                            }
                            Err(e) => {
                                log::error!("spawn_blocking task panicked: {:?}", e);
                            }
                        }
                    }

                    log::info!("[TIMING] All {} parallel CPAL captures completed in {:?}", capture_results.len(), parallel_start.elapsed());
                }

                log::info!("All captures processed in {:?}", captures_start.elapsed());
            }
            Err(e) => {
                log::error!("Failed to load profile '{}': {}", profile_name, e);
                for source_id in &source_ids {
                    capture_results.insert(source_id.clone(), json!({
                        "success": false,
                        "reason": "profileLoadFailed",
                        "message": format!("Failed to load profile: {}", e)
                    }));
                }
            }
        }
    }

    log::info!("=== set_audio_monitor_sources COMPLETE in {:?} ===", cmd_start.elapsed());
    Ok(json!({
        "captureResults": capture_results,
        "trackedSources": source_ids.len()
    }))
}

/// Helper: Start system audio capture on non-macOS platforms.
/// Uses cpal loopback on Windows, or PipeWire monitor source on Linux.
#[cfg(not(target_os = "macos"))]
fn start_system_audio_capture(
    state: &AppState,
    source_id: &str,
    source_type: &str,
) -> Result<(), String> {
    // Try loopback capture first (Windows WASAPI)
    #[cfg(target_os = "windows")]
    {
        let rx = state.audio_capture.start_loopback_capture(
            source_id,
            AudioCaptureConfig::default(),
        )?;
        state.audio_level_service.register_audio_source(source_id, rx);
        log::info!("✓ System audio capture STARTED for {} source '{}' via WASAPI loopback", source_type, source_id);
        return Ok(());
    }

    // Linux: Try PipeWire monitor source
    #[cfg(target_os = "linux")]
    {
        if let Some(monitor_device) = find_monitor_device() {
            let rx = state.audio_capture.start_input_capture_for_source(
                source_id,
                &monitor_device,
                AudioCaptureConfig::default(),
            )?;
            state.audio_level_service.register_audio_source(source_id, rx);
            log::info!("✓ System audio capture STARTED for {} source '{}' via PipeWire monitor '{}'", source_type, source_id, monitor_device);
            return Ok(());
        }
        return Err("No PipeWire monitor source found. Install pipewire-alsa for system audio capture.".to_string());
    }

    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    {
        Err(format!("System audio capture not supported on this platform for {} sources", source_type))
    }
}

/// Find the default PipeWire/PulseAudio monitor source for system audio capture.
/// Monitor sources have names like "alsa_output.pci-0000_04_00.6.analog-stereo.monitor".
#[cfg(target_os = "linux")]
fn find_monitor_device() -> Option<String> {
    use cpal::traits::{DeviceTrait, HostTrait};
    let host = cpal::default_host();
    host.input_devices().ok()?.find_map(|d| {
        let name = d.name().ok()?;
        if name.contains(".monitor") {
            Some(name)
        } else {
            None
        }
    })
}
