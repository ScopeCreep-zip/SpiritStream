use futures_util::FutureExt;
use governor::{Quota, RateLimiter};
use std::{
    env,
    net::SocketAddr,
    num::NonZeroU32,
    path::PathBuf,
    sync::{Arc, Mutex},
};
use tokio::signal;
use tokio::sync::Mutex as AsyncMutex;
use tokio::task::JoinSet;
use tower_http::services::{ServeDir, ServeFile};

use spiritstream_server::app_state::{AppState, EventBus};
use spiritstream_server::logging::init_logger;
use spiritstream_server::middleware::{parse_bool, DEFAULT_RATE_LIMIT_PER_MINUTE};
use spiritstream_server::server::{build_router, find_themes_dir_fallback, parse_host};
use spiritstream_server::services::{
    prune_logs, EventSink, FFmpegDownloader, FFmpegHandler, ProfileManager, SettingsManager,
    ThemeManager, PreviewHandler,
    // Native capture services
    ScreenCaptureService, AudioCaptureService,
    CameraCaptureService,
    NativePreviewService,
    RecordingService, ReplayBufferService,
    CaptureIndicatorService,
    // WebRTC services
    Go2RtcManager,
    // H264 capture service for native screen capture to WebRTC
    H264CaptureService,
    // Audio level monitoring
    AudioLevelService,
    // In-process media file audio decode (symphonia)
    MediaAudioDecoder,
    // In-process stream audio decode (RTMP/NDI via go2rtc + symphonia)
    StreamAudioDecoder,
    // Source lifecycle tracking
    SourceLifecycleService,
    // Centralized power management
    PowerBudgetManager,
    // Device enumeration cache
    DeviceCache,
};
// ScreenCaptureKit audio capture service (macOS only)
#[cfg(target_os = "macos")]
use spiritstream_server::services::SckAudioCaptureService;

/// Graceful shutdown signal handler
/// Waits for Ctrl+C or SIGTERM, then stops all services in order
async fn shutdown_signal(state: AppState) {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("Failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    log::info!("Shutdown signal received, stopping services...");

    // Stop services in dependency order with per-service timeout (5s max each)
    let svc_timeout = std::time::Duration::from_secs(5);

    // 1. Stop active streams (FFmpeg processes)
    log::info!("Stopping FFmpeg streams...");
    if let Err(e) = state.ffmpeg_handler.stop_all() {
        log::warn!("Error stopping FFmpeg streams: {}", e);
    }

    // 2. Stop recording and replay buffer (with timeout to prevent hang)
    log::info!("Stopping recording and replay buffer...");
    if tokio::time::timeout(svc_timeout, async {
        let _ = state.recording_service.stop_all();
    }).await.is_err() {
        log::warn!("Recording stop timed out after {:?}", svc_timeout);
    }
    if tokio::time::timeout(svc_timeout, async {
        if let Err(e) = state.replay_buffer.stop() {
            log::warn!("Error stopping replay buffer: {}", e);
        }
    }).await.is_err() {
        log::warn!("Replay buffer stop timed out after {:?}", svc_timeout);
    }

    // 3. Stop WebRTC/go2rtc (with timeout in case health check is hanging)
    log::info!("Stopping go2rtc...");
    if tokio::time::timeout(svc_timeout, state.go2rtc_manager.stop()).await.is_err() {
        log::warn!("go2rtc stop timed out after {:?}, forcing kill", svc_timeout);
    }

    // 4. Stop all preview handlers
    log::info!("Stopping preview handlers...");
    state.preview_handler.stop_all_previews();
    state.native_preview.stop_all();

    // 5. Stop capture services
    log::info!("Stopping capture services...");
    state.screen_capture.stop_all();
    state.camera_capture.stop_all();
    state.audio_capture.stop_all();
    state.h264_capture.stop_all();

    // 6. Stop audio level monitoring and media decoders
    log::info!("Stopping audio level monitoring...");
    state.audio_level_service.stop();
    state.media_audio_decoder.stop_all();
    state.stream_audio_decoder.stop_all();

    // 7. Clear source lifecycle tracking and stop power monitoring
    state.source_lifecycle.clear();
    state.power_budget.stop();

    // 8. Drain tracked background tasks
    log::info!("Draining background tasks...");
    {
        let mut tasks = state.background_tasks.lock().unwrap();
        tasks.abort_all();
        // Give tasks a moment to abort
        while tasks.join_next().now_or_never().is_some() {}
    }

    log::info!("All services stopped, server shutting down");
}

#[tokio::main(worker_threads = 4)]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load configuration from environment
    let data_dir = env::var("SPIRITSTREAM_DATA_DIR").unwrap_or_else(|_| "data".to_string());
    let log_dir = env::var("SPIRITSTREAM_LOG_DIR")
        .unwrap_or_else(|_| format!("{data_dir}/logs"));
    // Resolve themes directory with fallback logic
    let themes_dir = match env::var("SPIRITSTREAM_THEMES_DIR") {
        Ok(dir) => {
            let path = PathBuf::from(&dir);
            let has_themes = std::fs::read_dir(&path)
                .map(|entries| {
                    entries.flatten().any(|e| {
                        e.path()
                            .extension()
                            .map(|ext| ext == "jsonc" || ext == "json")
                            .unwrap_or(false)
                    })
                })
                .unwrap_or(false);

            if has_themes {
                dir
            } else {
                find_themes_dir_fallback().unwrap_or(dir)
            }
        }
        Err(_) => {
            find_themes_dir_fallback().unwrap_or_else(|| "themes".to_string())
        }
    };
    let ui_dir = env::var("SPIRITSTREAM_UI_DIR").unwrap_or_else(|_| "dist".to_string());
    let env_host = env::var("SPIRITSTREAM_HOST").ok();
    let env_port: Option<u16> = env::var("SPIRITSTREAM_PORT")
        .ok()
        .and_then(|value| value.parse().ok());
    let env_auth_token = env::var("SPIRITSTREAM_API_TOKEN")
        .or_else(|_| env::var("SPIRITSTREAM_DEV_TOKEN"))
        .ok()
        .and_then(|value| {
            let trimmed = value.trim().to_string();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            }
        });

    let app_data_dir = PathBuf::from(&data_dir);
    let log_dir_path = PathBuf::from(&log_dir);
    std::fs::create_dir_all(&app_data_dir)?;
    std::fs::create_dir_all(&log_dir_path)?;

    let profile_manager = Arc::new(ProfileManager::new(app_data_dir.clone()));
    let settings_manager = Arc::new(SettingsManager::new(app_data_dir.clone()));

    // Load settings
    let settings = settings_manager.load().ok();
    let settings_ui_enabled = settings
        .as_ref()
        .map(|settings| settings.backend_ui_enabled)
        .unwrap_or(false);
    let env_ui_enabled = env::var("SPIRITSTREAM_UI_ENABLED")
        .ok()
        .and_then(|value| parse_bool(&value));
    let ui_enabled = env_ui_enabled.unwrap_or(settings_ui_enabled);
    let settings_auth_token = settings.as_ref().and_then(|settings| {
        let trimmed = settings.backend_token.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    });
    let auth_token = env_auth_token.or(settings_auth_token);

    // Determine host/port: env vars take precedence, then settings, then defaults
    let (host, port) = {
        let remote_enabled = settings
            .as_ref()
            .map(|s| s.backend_remote_enabled)
            .unwrap_or(false);
        let settings_host = settings
            .as_ref()
            .map(|s| s.backend_host.clone())
            .unwrap_or_else(|| "127.0.0.1".to_string());
        let settings_port = settings
            .as_ref()
            .map(|s| s.backend_port)
            .unwrap_or(8008);

        let env_host_was_set = env_host.is_some();
        let configured_host = env_host.unwrap_or(settings_host);
        let configured_port = env_port.unwrap_or(settings_port);

        let final_host = if !remote_enabled && !env_host_was_set {
            "127.0.0.1".to_string()
        } else {
            configured_host
        };

        (final_host, configured_port)
    };
    log::info!("Server will bind to {host}:{port}");

    let custom_ffmpeg_path = settings.as_ref().and_then(|s| {
        if s.ffmpeg_path.is_empty() {
            None
        } else {
            Some(s.ffmpeg_path.clone())
        }
    });

    if let Some(settings) = settings.as_ref() {
        let _ = prune_logs(&log_dir_path, settings.log_retention_days);
    }

    let ffmpeg_handler = Arc::new(FFmpegHandler::new_with_custom_path(
        app_data_dir.clone(),
        custom_ffmpeg_path.clone(),
    ));

    // Create preview handler using the same FFmpeg path
    let preview_ffmpeg_path = custom_ffmpeg_path
        .clone()
        .filter(|p| !p.is_empty() && std::path::Path::new(p).exists())
        .unwrap_or_else(|| ffmpeg_handler.get_ffmpeg_path());
    let preview_handler = Arc::new(PreviewHandler::new(preview_ffmpeg_path.clone()));

    let event_bus = EventBus::new();
    init_logger(&log_dir_path, event_bus.clone())?;

    // Log the themes directory configuration
    let themes_path = PathBuf::from(&themes_dir);
    let themes_exist = themes_path.exists();
    let env_was_set = env::var("SPIRITSTREAM_THEMES_DIR").is_ok();
    log::info!(
        "Themes directory: {themes_dir} (exists={themes_exist}, env_set={env_was_set})"
    );
    if !themes_exist {
        log::warn!("Themes directory does not exist - custom themes may not load");
    }

    let theme_manager = Arc::new(ThemeManager::new(app_data_dir.clone(), PathBuf::from(&themes_dir)));

    // Sync themes in background to avoid blocking startup
    {
        let theme_manager_clone = theme_manager.clone();
        tokio::task::spawn_blocking(move || {
            log::info!("Starting theme sync in background");
            theme_manager_clone.sync_project_themes();
            let synced_themes = theme_manager_clone.list_themes();
            log::info!(
                "Theme sync complete. Available themes ({}): {:?}",
                synced_themes.len(),
                synced_themes.iter().map(|t| &t.id).collect::<Vec<_>>()
            );
        });
    }

    let theme_event_sink: Arc<dyn EventSink> = Arc::new(event_bus.clone());
    theme_manager.start_watcher(theme_event_sink);

    // Initialize rate limiter
    let rate_limit = env::var("SPIRITSTREAM_RATE_LIMIT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_RATE_LIMIT_PER_MINUTE);
    let rate_limiter = Arc::new(RateLimiter::direct(Quota::per_minute(
        NonZeroU32::new(rate_limit).unwrap_or(NonZeroU32::new(100).unwrap()),
    )));

    let home_dir = dirs_next::home_dir();

    // Initialize native capture services
    let screen_capture = Arc::new(ScreenCaptureService::new());
    let audio_capture = Arc::new(AudioCaptureService::new());
    let camera_capture = Arc::new(CameraCaptureService::new(preview_ffmpeg_path.clone()));
    let native_preview = Arc::new(NativePreviewService::new());
    let recording_service = Arc::new(
        match RecordingService::new(preview_ffmpeg_path.clone(), app_data_dir.clone()) {
            Ok(svc) => svc,
            Err(e) => {
                log::warn!("Recording service unavailable: {}. Recording disabled.", e);
                RecordingService::disabled()
            }
        }
    );
    let replay_buffer = Arc::new(
        match ReplayBufferService::new(preview_ffmpeg_path.clone(), app_data_dir.clone()) {
            Ok(svc) => svc,
            Err(e) => {
                log::warn!("Replay buffer service unavailable: {}. Replay buffer disabled.", e);
                ReplayBufferService::disabled()
            }
        }
    );
    let capture_indicator = Arc::new(CaptureIndicatorService::new());

    // Initialize go2rtc manager for WebRTC preview streaming (lazy - started on first use)
    let go2rtc_manager = Arc::new(Go2RtcManager::new());

    // Initialize H264 capture service for native screen capture -> WebRTC
    let h264_capture = Arc::new(H264CaptureService::new(
        screen_capture.clone(),
        preview_ffmpeg_path.clone(),
    ));

    // Initialize source lifecycle tracking (ref-counted source management)
    let source_lifecycle = Arc::new(SourceLifecycleService::new());

    // Initialize centralized power budget manager
    let power_budget = Arc::new(PowerBudgetManager::new());
    power_budget.start_thermal_monitor();

    // Wire thermal throttle flag into NativePreviewService
    // so preview encode threads reduce FPS under thermal pressure
    native_preview.set_throttle_flag(power_budget.throttle_flag());

    // Wire event sink into PowerBudgetManager so thermal state changes
    // emit WebSocket events to the frontend
    power_budget.set_event_sink(Arc::new(event_bus.clone()));

    // Initialize audio level monitoring service
    let audio_level_service = Arc::new(AudioLevelService::new());
    let media_audio_decoder = Arc::new(MediaAudioDecoder::new());
    let stream_audio_decoder = Arc::new(StreamAudioDecoder::new());
    #[cfg(target_os = "macos")]
    let sck_audio_capture = Arc::new(SckAudioCaptureService::new());
    let event_bus_for_audio = event_bus.clone();

    let state = AppState {
        profile_manager,
        settings_manager,
        ffmpeg_handler,
        ffmpeg_downloader: Arc::new(AsyncMutex::new(FFmpegDownloader::new())),
        theme_manager,
        preview_handler,
        event_bus,
        log_dir: log_dir_path,
        app_data_dir,
        auth_token,
        rate_limiter,
        home_dir,
        screen_capture,
        audio_capture,
        camera_capture,
        native_preview,
        recording_service,
        replay_buffer,
        capture_indicator,
        go2rtc_manager,
        h264_capture,
        audio_level_service: audio_level_service.clone(),
        media_audio_decoder: media_audio_decoder.clone(),
        stream_audio_decoder: stream_audio_decoder.clone(),
        source_lifecycle,
        power_budget,
        device_cache: Arc::new(DeviceCache::new()),
        #[cfg(target_os = "macos")]
        sck_audio_capture,
        server_port: port,
        background_tasks: Arc::new(Mutex::new(JoinSet::new())),
    };

    // Start audio level monitoring service
    let idle_flag = state.native_preview.idle_flag();
    let throttle_flag = state.power_budget.throttle_flag();
    audio_level_service.start(Arc::new(event_bus_for_audio), idle_flag, throttle_flag);

    // Start H264 orphan session reaper (cleans up sessions inactive for 60s)
    state.h264_capture.start_cleanup_task();

    // Build router
    let mut app = build_router(state.clone());

    // Optionally serve static UI files
    let ui_path = PathBuf::from(ui_dir);
    if ui_enabled && ui_path.exists() {
        app = app.fallback_service(
            ServeDir::new(&ui_path).fallback(ServeFile::new(ui_path.join("index.html"))),
        );
    }

    let address = SocketAddr::new(parse_host(&host), port);
    log::info!("SpiritStream backend listening on http://{address}");
    if state.auth_token.is_some() {
        log::info!("  Authentication: enabled");
    } else {
        log::info!("  Authentication: disabled (no token configured)");
    }

    let listener = tokio::net::TcpListener::bind(address).await?;

    // Run server with graceful shutdown
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal(state))
        .await?;

    Ok(())
}
