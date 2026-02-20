// Application State
// Shared state types used by main.rs and command modules

use governor::{
    clock::DefaultClock,
    state::{InMemoryState, NotKeyed},
    RateLimiter,
};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};
use tokio::sync::{broadcast, Mutex as AsyncMutex};
use tokio::task::JoinSet;

use crate::services::{
    AudioCaptureService, AudioLevelService, CameraCaptureService,
    CaptureIndicatorService, DeviceCache, EventSink, FFmpegDownloader, FFmpegHandler,
    Go2RtcManager, H264CaptureService, MediaAudioDecoder, NativePreviewService,
    PowerBudgetManager, PreviewHandler, ProfileManager, RecordingService,
    ReplayBufferService, ScreenCaptureService, SettingsManager, SourceLifecycleService,
    StreamAudioDecoder, ThemeManager,
};
#[cfg(target_os = "macos")]
use crate::services::SckAudioCaptureService;

// ============================================================================
// Event System
// ============================================================================

#[derive(Clone, Serialize)]
pub struct ServerEvent {
    pub event: String,
    pub payload: Value,
}

#[derive(Clone)]
pub struct EventBus {
    pub sender: broadcast::Sender<ServerEvent>,
}

impl EventBus {
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(256);
        Self { sender }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<ServerEvent> {
        self.sender.subscribe()
    }
}

impl EventSink for EventBus {
    fn emit(&self, event: &str, payload: Value) {
        let _ = self.sender.send(ServerEvent {
            event: event.to_string(),
            payload,
        });
    }
}

// ============================================================================
// Application State
// ============================================================================

#[derive(Clone)]
pub struct AppState {
    pub profile_manager: Arc<ProfileManager>,
    pub settings_manager: Arc<SettingsManager>,
    pub ffmpeg_handler: Arc<FFmpegHandler>,
    pub ffmpeg_downloader: Arc<AsyncMutex<FFmpegDownloader>>,
    pub theme_manager: Arc<ThemeManager>,
    pub preview_handler: Arc<PreviewHandler>,
    pub event_bus: EventBus,
    pub log_dir: PathBuf,
    pub app_data_dir: PathBuf,
    pub auth_token: Option<String>,
    pub rate_limiter: Arc<RateLimiter<NotKeyed, InMemoryState, DefaultClock>>,
    pub home_dir: Option<PathBuf>,
    // Native capture services
    pub screen_capture: Arc<ScreenCaptureService>,
    pub audio_capture: Arc<AudioCaptureService>,
    pub camera_capture: Arc<CameraCaptureService>,
    pub native_preview: Arc<NativePreviewService>,
    pub recording_service: Arc<RecordingService>,
    pub replay_buffer: Arc<ReplayBufferService>,
    pub capture_indicator: Arc<CaptureIndicatorService>,
    pub go2rtc_manager: Arc<Go2RtcManager>,
    pub h264_capture: Arc<H264CaptureService>,
    pub audio_level_service: Arc<AudioLevelService>,
    pub media_audio_decoder: Arc<MediaAudioDecoder>,
    pub stream_audio_decoder: Arc<StreamAudioDecoder>,
    #[cfg(target_os = "macos")]
    pub sck_audio_capture: Arc<SckAudioCaptureService>,
    pub source_lifecycle: Arc<SourceLifecycleService>,
    pub power_budget: Arc<PowerBudgetManager>,
    pub device_cache: Arc<DeviceCache>,
    pub server_port: u16,
    pub background_tasks: Arc<Mutex<JoinSet<()>>>,
}

// ============================================================================
// Argument Parsing Helpers
// ============================================================================

pub fn get_arg<T: DeserializeOwned>(payload: &Value, key: &str) -> Result<T, String> {
    let obj = payload
        .as_object()
        .ok_or_else(|| "Invalid payload".to_string())?;
    let value = obj
        .get(key)
        .ok_or_else(|| format!("Missing argument: {key}"))?;
    serde_json::from_value(value.clone()).map_err(|e| format!("Invalid {key}: {e}"))
}

pub fn get_opt_arg<T: DeserializeOwned>(payload: &Value, key: &str) -> Result<Option<T>, String> {
    let obj = payload
        .as_object()
        .ok_or_else(|| "Invalid payload".to_string())?;
    let value = match obj.get(key) {
        Some(value) => value.clone(),
        None => return Ok(None),
    };

    if value.is_null() {
        return Ok(None);
    }

    serde_json::from_value(value)
        .map(Some)
        .map_err(|e| format!("Invalid {key}: {e}"))
}
