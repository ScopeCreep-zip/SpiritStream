// Screen Capture Service
// Uses scap for native screen capture with OS permission handling

use scap::{
    capturer::{Capturer, Options, Resolution},
    frame::{Frame, FrameType},
    Target,
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::broadcast;

/// Information about an available display
#[derive(Debug, Clone, serde::Serialize)]
pub struct DisplayInfo {
    pub id: u32,
    pub name: String,
}

/// Information about an available window
#[derive(Debug, Clone, serde::Serialize)]
pub struct WindowInfo {
    pub id: u32,
    pub title: String,
}

/// Screen capture configuration
#[derive(Debug, Clone)]
pub struct ScreenCaptureConfig {
    pub fps: u32,
    pub show_cursor: bool,
    pub show_highlight: bool,
    pub output_resolution: Resolution,
}

impl Default for ScreenCaptureConfig {
    fn default() -> Self {
        Self {
            fps: 30,
            show_cursor: true,
            show_highlight: false,
            output_resolution: Resolution::Captured,
        }
    }
}

/// Active capture session
struct ActiveCapture {
    stop_flag: Arc<AtomicBool>,
    _handle: std::thread::JoinHandle<()>,
}

/// Service for managing screen capture
pub struct ScreenCaptureService {
    active_captures: Mutex<HashMap<String, ActiveCapture>>,
}

impl ScreenCaptureService {
    pub fn new() -> Self {
        Self {
            active_captures: Mutex::new(HashMap::new()),
        }
    }

    /// Check if screen capture is supported on this platform
    pub fn is_supported() -> bool {
        scap::is_supported()
    }

    /// Check if screen capture permission has been granted
    pub fn has_permission() -> bool {
        scap::has_permission()
    }

    /// Request screen capture permission (macOS only, shows system dialog)
    pub fn request_permission() -> bool {
        scap::request_permission()
    }

    /// List available displays/monitors
    pub fn list_displays() -> Vec<DisplayInfo> {
        let targets = scap::get_all_targets();

        targets
            .into_iter()
            .filter_map(|target| {
                if let Target::Display(display) = target {
                    Some(DisplayInfo {
                        id: display.id,
                        name: display.title,
                    })
                } else {
                    None
                }
            })
            .collect()
    }

    /// List available windows
    pub fn list_windows() -> Vec<WindowInfo> {
        let targets = scap::get_all_targets();

        targets
            .into_iter()
            .filter_map(|target| {
                if let Target::Window(window) = target {
                    Some(WindowInfo {
                        id: window.id,
                        title: window.title,
                    })
                } else {
                    None
                }
            })
            .collect()
    }

    /// Start capturing a display
    pub fn start_display_capture(
        &self,
        display_id: u32,
        config: ScreenCaptureConfig,
    ) -> Result<broadcast::Receiver<Arc<Frame>>, String> {
        // Check permission first
        if !Self::has_permission() {
            return Err("Screen capture permission not granted. Please grant permission in System Preferences.".to_string());
        }

        let capture_id = format!("display_{}", display_id);

        // Check if already capturing
        {
            let captures = self.active_captures.lock().unwrap();
            if captures.contains_key(&capture_id) {
                return Err(format!("Already capturing display {}", display_id));
            }
        }

        // Find the display
        let targets = scap::get_all_targets();
        let display = targets
            .into_iter()
            .find_map(|t| {
                if let Target::Display(d) = t {
                    if d.id == display_id {
                        Some(d)
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .ok_or_else(|| format!("Display {} not found", display_id))?;

        // Create capturer options
        let options = Options {
            fps: config.fps,
            show_cursor: config.show_cursor,
            show_highlight: config.show_highlight,
            target: Some(Target::Display(display)),
            excluded_targets: None,
            output_type: FrameType::BGRAFrame,
            output_resolution: config.output_resolution,
            crop_area: None,
            ..Default::default()
        };

        // Create capturer
        let mut capturer = Capturer::build(options)
            .map_err(|e| format!("Failed to build capturer: {:?}", e))?;

        // Create broadcast channel for frames
        let (frame_tx, frame_rx) = broadcast::channel::<Arc<Frame>>(16);
        let stop_flag = Arc::new(AtomicBool::new(false));
        let stop_flag_clone = stop_flag.clone();

        // Start capture in background thread
        let handle = std::thread::spawn(move || {
            capturer.start_capture();

            while !stop_flag_clone.load(Ordering::Relaxed) {
                if let Ok(frame) = capturer.get_next_frame() {
                    let _ = frame_tx.send(Arc::new(frame));
                }
            }

            capturer.stop_capture();
        });

        // Store active capture
        {
            let mut captures = self.active_captures.lock().unwrap();
            captures.insert(capture_id.clone(), ActiveCapture {
                stop_flag,
                _handle: handle,
            });
        }

        log::info!("Started screen capture for display {}", display_id);
        Ok(frame_rx)
    }

    /// Start capturing a window
    pub fn start_window_capture(
        &self,
        window_id: u32,
        config: ScreenCaptureConfig,
    ) -> Result<broadcast::Receiver<Arc<Frame>>, String> {
        // Check permission first
        if !Self::has_permission() {
            return Err("Screen capture permission not granted. Please grant permission in System Preferences.".to_string());
        }

        let capture_id = format!("window_{}", window_id);

        // Check if already capturing
        {
            let captures = self.active_captures.lock().unwrap();
            if captures.contains_key(&capture_id) {
                return Err(format!("Already capturing window {}", window_id));
            }
        }

        // Find the window
        let targets = scap::get_all_targets();
        let window = targets
            .into_iter()
            .find_map(|t| {
                if let Target::Window(w) = t {
                    if w.id == window_id {
                        Some(w)
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .ok_or_else(|| format!("Window {} not found", window_id))?;

        // Create capturer options
        let options = Options {
            fps: config.fps,
            show_cursor: config.show_cursor,
            show_highlight: config.show_highlight,
            target: Some(Target::Window(window)),
            excluded_targets: None,
            output_type: FrameType::BGRAFrame,
            output_resolution: config.output_resolution,
            crop_area: None,
            ..Default::default()
        };

        // Create capturer
        let mut capturer = Capturer::build(options)
            .map_err(|e| format!("Failed to build capturer: {:?}", e))?;

        // Create broadcast channel for frames
        let (frame_tx, frame_rx) = broadcast::channel::<Arc<Frame>>(16);
        let stop_flag = Arc::new(AtomicBool::new(false));
        let stop_flag_clone = stop_flag.clone();

        // Start capture in background thread
        let handle = std::thread::spawn(move || {
            capturer.start_capture();

            while !stop_flag_clone.load(Ordering::Relaxed) {
                if let Ok(frame) = capturer.get_next_frame() {
                    let _ = frame_tx.send(Arc::new(frame));
                }
            }

            capturer.stop_capture();
        });

        // Store active capture
        {
            let mut captures = self.active_captures.lock().unwrap();
            captures.insert(capture_id.clone(), ActiveCapture {
                stop_flag,
                _handle: handle,
            });
        }

        log::info!("Started screen capture for window {}", window_id);
        Ok(frame_rx)
    }

    /// Stop a capture by ID
    pub fn stop_capture(&self, capture_id: &str) -> Result<(), String> {
        let mut captures = self.active_captures.lock().unwrap();

        if let Some(capture) = captures.remove(capture_id) {
            capture.stop_flag.store(true, Ordering::Relaxed);
            log::info!("Stopped screen capture: {}", capture_id);
            Ok(())
        } else {
            Err(format!("No active capture: {}", capture_id))
        }
    }

    /// Stop all active captures
    pub fn stop_all(&self) {
        let mut captures = self.active_captures.lock().unwrap();

        for (id, capture) in captures.drain() {
            capture.stop_flag.store(true, Ordering::Relaxed);
            log::info!("Stopped screen capture: {}", id);
        }
    }

    /// Check if a capture is active
    pub fn is_capturing(&self, capture_id: &str) -> bool {
        let captures = self.active_captures.lock().unwrap();
        captures.contains_key(capture_id)
    }

    /// Get count of active captures
    pub fn active_capture_count(&self) -> usize {
        let captures = self.active_captures.lock().unwrap();
        captures.len()
    }

    /// Get list of active capture IDs
    pub fn active_capture_ids(&self) -> Vec<String> {
        let captures = self.active_captures.lock().unwrap();
        captures.keys().cloned().collect()
    }
}

impl Default for ScreenCaptureService {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for ScreenCaptureService {
    fn drop(&mut self) {
        self.stop_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_supported() {
        let supported = ScreenCaptureService::is_supported();
        println!("Screen capture supported: {}", supported);
    }

    #[test]
    fn test_list_displays() {
        if ScreenCaptureService::is_supported() && ScreenCaptureService::has_permission() {
            let displays = ScreenCaptureService::list_displays();
            println!("Found {} displays:", displays.len());
            for display in displays {
                println!("  - {} (ID: {})", display.name, display.id);
            }
        }
    }

    #[test]
    fn test_list_windows() {
        if ScreenCaptureService::is_supported() && ScreenCaptureService::has_permission() {
            let windows = ScreenCaptureService::list_windows();
            println!("Found {} windows:", windows.len());
            for window in &windows[..windows.len().min(10)] {
                println!("  - {} (ID: {})", window.title, window.id);
            }
        }
    }
}
