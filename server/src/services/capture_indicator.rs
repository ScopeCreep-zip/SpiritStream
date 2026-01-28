// Capture Indicator Service
// Tracks active captures and emits status events

use std::collections::HashSet;
use std::sync::Mutex;
use serde::{Deserialize, Serialize};

use crate::services::{emit_event, EventSink};

/// Type of capture source
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "type", content = "id", rename_all = "camelCase")]
pub enum CaptureType {
    /// Camera capture (device_id)
    Camera(String),
    /// Screen capture (display_id)
    Screen(String),
    /// Window capture (window_id)
    Window(String),
    /// Microphone capture (device_id)
    Microphone(String),
    /// System audio capture (loopback)
    SystemAudio,
}

impl CaptureType {
    /// Get a display-friendly name for the capture type
    pub fn display_name(&self) -> String {
        match self {
            CaptureType::Camera(id) => format!("Camera: {}", id),
            CaptureType::Screen(id) => format!("Screen: {}", id),
            CaptureType::Window(id) => format!("Window: {}", id),
            CaptureType::Microphone(id) => format!("Microphone: {}", id),
            CaptureType::SystemAudio => "System Audio".to_string(),
        }
    }

    /// Get the category of the capture
    pub fn category(&self) -> &'static str {
        match self {
            CaptureType::Camera(_) => "camera",
            CaptureType::Screen(_) | CaptureType::Window(_) => "screen",
            CaptureType::Microphone(_) | CaptureType::SystemAudio => "audio",
        }
    }
}

/// Capture status summary
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureStatus {
    pub is_capturing: bool,
    pub camera_active: bool,
    pub screen_active: bool,
    pub audio_active: bool,
    pub active_count: usize,
    pub active_captures: Vec<CaptureType>,
}

/// Service for tracking and indicating capture activity
pub struct CaptureIndicatorService {
    active_captures: Mutex<HashSet<CaptureType>>,
}

impl CaptureIndicatorService {
    pub fn new() -> Self {
        Self {
            active_captures: Mutex::new(HashSet::new()),
        }
    }

    /// Register an active capture (emits event to frontend)
    pub fn register_capture(&self, capture: CaptureType, event_sink: Option<&dyn EventSink>) {
        let mut captures = self.active_captures.lock().unwrap_or_else(|e| {
            log::warn!("Capture indicator lock poisoned, recovering: {}", e);
            e.into_inner()
        });

        let was_empty = captures.is_empty();
        captures.insert(capture.clone());

        log::info!("Capture started: {}", capture.display_name());

        // Emit events
        if let Some(sink) = event_sink {
            emit_event(sink, "capture_started", &capture);

            // Emit status update
            let status = self.build_status_locked(&captures);
            emit_event(sink, "capture_status", &status);

            // First capture started
            if was_empty {
                emit_event(sink, "capture_any_started", &());
            }
        }
    }

    /// Unregister a capture (emits event to frontend)
    pub fn unregister_capture(&self, capture: &CaptureType, event_sink: Option<&dyn EventSink>) {
        let mut captures = self.active_captures.lock().unwrap_or_else(|e| {
            log::warn!("Capture indicator lock poisoned, recovering: {}", e);
            e.into_inner()
        });

        if captures.remove(capture) {
            log::info!("Capture stopped: {}", capture.display_name());

            if let Some(sink) = event_sink {
                emit_event(sink, "capture_stopped", capture);

                // Emit status update
                let status = self.build_status_locked(&captures);
                emit_event(sink, "capture_status", &status);

                // All captures stopped
                if captures.is_empty() {
                    emit_event(sink, "capture_all_stopped", &());
                }
            }
        }
    }

    /// Check if any capture is active
    pub fn is_any_capture_active(&self) -> bool {
        self.active_captures.lock()
            .map(|c| !c.is_empty())
            .unwrap_or(false)
    }

    /// Check if a specific capture type category is active
    pub fn is_category_active(&self, category: &str) -> bool {
        self.active_captures.lock()
            .map(|c| c.iter().any(|cap| cap.category() == category))
            .unwrap_or(false)
    }

    /// Get all active captures
    pub fn get_active_captures(&self) -> Vec<CaptureType> {
        self.active_captures.lock()
            .map(|c| c.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Get current capture status
    pub fn get_status(&self) -> CaptureStatus {
        let captures = self.active_captures.lock().unwrap_or_else(|e| {
            log::warn!("Capture indicator lock poisoned, recovering: {}", e);
            e.into_inner()
        });

        self.build_status_locked(&captures)
    }

    /// Build status from locked captures
    fn build_status_locked(&self, captures: &HashSet<CaptureType>) -> CaptureStatus {
        let camera_active = captures.iter().any(|c| matches!(c, CaptureType::Camera(_)));
        let screen_active = captures.iter().any(|c| {
            matches!(c, CaptureType::Screen(_) | CaptureType::Window(_))
        });
        let audio_active = captures.iter().any(|c| {
            matches!(c, CaptureType::Microphone(_) | CaptureType::SystemAudio)
        });

        CaptureStatus {
            is_capturing: !captures.is_empty(),
            camera_active,
            screen_active,
            audio_active,
            active_count: captures.len(),
            active_captures: captures.iter().cloned().collect(),
        }
    }

    /// Get count of active captures
    pub fn active_count(&self) -> usize {
        self.active_captures.lock()
            .map(|c| c.len())
            .unwrap_or(0)
    }

    /// Clear all active captures (for cleanup)
    pub fn clear_all(&self, event_sink: Option<&dyn EventSink>) {
        let mut captures = self.active_captures.lock().unwrap_or_else(|e| {
            log::warn!("Capture indicator lock poisoned, recovering: {}", e);
            e.into_inner()
        });

        if !captures.is_empty() {
            captures.clear();

            if let Some(sink) = event_sink {
                let status = self.build_status_locked(&captures);
                emit_event(sink, "capture_status", &status);
                emit_event(sink, "capture_all_stopped", &());
            }

            log::info!("All captures cleared");
        }
    }
}

impl Default for CaptureIndicatorService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capture_type_display_name() {
        let camera = CaptureType::Camera("FaceTime".to_string());
        assert_eq!(camera.display_name(), "Camera: FaceTime");

        let screen = CaptureType::Screen("Display 1".to_string());
        assert_eq!(screen.display_name(), "Screen: Display 1");

        let system = CaptureType::SystemAudio;
        assert_eq!(system.display_name(), "System Audio");
    }

    #[test]
    fn test_capture_type_category() {
        assert_eq!(CaptureType::Camera("test".to_string()).category(), "camera");
        assert_eq!(CaptureType::Screen("test".to_string()).category(), "screen");
        assert_eq!(CaptureType::Window("test".to_string()).category(), "screen");
        assert_eq!(CaptureType::Microphone("test".to_string()).category(), "audio");
        assert_eq!(CaptureType::SystemAudio.category(), "audio");
    }

    #[test]
    fn test_register_unregister() {
        let service = CaptureIndicatorService::new();

        let camera = CaptureType::Camera("test".to_string());
        service.register_capture(camera.clone(), None);

        assert!(service.is_any_capture_active());
        assert_eq!(service.active_count(), 1);

        service.unregister_capture(&camera, None);

        assert!(!service.is_any_capture_active());
        assert_eq!(service.active_count(), 0);
    }

    #[test]
    fn test_get_status() {
        let service = CaptureIndicatorService::new();

        service.register_capture(CaptureType::Camera("cam".to_string()), None);
        service.register_capture(CaptureType::Microphone("mic".to_string()), None);

        let status = service.get_status();

        assert!(status.is_capturing);
        assert!(status.camera_active);
        assert!(!status.screen_active);
        assert!(status.audio_active);
        assert_eq!(status.active_count, 2);
    }
}
