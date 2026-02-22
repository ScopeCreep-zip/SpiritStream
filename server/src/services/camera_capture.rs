// Camera Capture Service
// Uses FFmpeg for camera capture with platform-specific device access

use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::io::{BufReader, Read};
use tokio::sync::broadcast;

use super::capture_core::session::{CaptureSession, CaptureSessionManager};

/// Information about an available camera
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CameraInfo {
    pub id: String,
    pub name: String,
    pub device_path: String,
    pub formats: Vec<CameraFormat>,
}

/// Supported camera format
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CameraFormat {
    pub width: u32,
    pub height: u32,
    pub fps: Vec<u32>,
    pub pixel_format: String,
}

/// Camera capture configuration
#[derive(Debug, Clone)]
pub struct CameraCaptureConfig {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub pixel_format: Option<String>,
}

impl Default for CameraCaptureConfig {
    fn default() -> Self {
        Self {
            width: 1280,
            height: 720,
            fps: 30,
            pixel_format: None,
        }
    }
}

/// Video frame from camera
#[derive(Debug, Clone)]
pub struct VideoFrame {
    pub data: Arc<Vec<u8>>,
    pub width: u32,
    pub height: u32,
    pub pixel_format: &'static str,
    pub timestamp_ms: u64,
}

/// Service for managing camera capture via FFmpeg
pub struct CameraCaptureService {
    ffmpeg_path: String,
    sessions: CaptureSessionManager<VideoFrame>,
}

impl CameraCaptureService {
    pub fn new(ffmpeg_path: String) -> Self {
        Self {
            ffmpeg_path,
            sessions: CaptureSessionManager::new("camera"),
        }
    }

    /// Update FFmpeg path
    pub fn set_ffmpeg_path(&mut self, path: String) {
        self.ffmpeg_path = path;
    }

    /// List available cameras using FFmpeg device enumeration
    pub fn list_cameras(&self) -> Vec<CameraInfo> {
        #[cfg(target_os = "macos")]
        {
            self.list_cameras_macos()
        }
        #[cfg(target_os = "windows")]
        {
            self.list_cameras_windows()
        }
        #[cfg(target_os = "linux")]
        {
            self.list_cameras_linux()
        }
    }

    #[cfg(target_os = "macos")]
    fn list_cameras_macos(&self) -> Vec<CameraInfo> {
        // Use FFmpeg to list AVFoundation devices
        let output = Command::new(&self.ffmpeg_path)
            .args(["-f", "avfoundation", "-list_devices", "true", "-i", ""])
            .stderr(Stdio::piped())
            .output();

        let mut cameras = Vec::new();

        if let Ok(output) = output {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let mut in_video_section = false;
            let mut index = 0;

            for line in stderr.lines() {
                if line.contains("AVFoundation video devices:") {
                    in_video_section = true;
                    continue;
                }
                if line.contains("AVFoundation audio devices:") {
                    break;
                }
                if in_video_section {
                    // Parse device line like "[AVFoundation indev @ 0x...] [0] FaceTime HD Camera"
                    if let Some(bracket_pos) = line.rfind('[') {
                        let rest = &line[bracket_pos + 1..];
                        if let Some(end_bracket) = rest.find(']') {
                            let idx_str = &rest[..end_bracket];
                            if let Ok(_) = idx_str.parse::<u32>() {
                                let name = rest[end_bracket + 1..].trim().to_string();
                                if !name.is_empty() {
                                    cameras.push(CameraInfo {
                                        id: index.to_string(),
                                        name: name.clone(),
                                        device_path: index.to_string(),
                                        formats: vec![CameraFormat {
                                            width: 1280,
                                            height: 720,
                                            fps: vec![30, 60],
                                            pixel_format: "nv12".to_string(),
                                        }],
                                    });
                                    index += 1;
                                }
                            }
                        }
                    }
                }
            }
        }

        cameras
    }

    #[cfg(target_os = "windows")]
    fn list_cameras_windows(&self) -> Vec<CameraInfo> {
        // Use FFmpeg to list DirectShow devices
        let output = Command::new(&self.ffmpeg_path)
            .args(["-f", "dshow", "-list_devices", "true", "-i", "dummy"])
            .stderr(Stdio::piped())
            .output();

        let mut cameras = Vec::new();

        if let Ok(output) = output {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let mut in_video_section = false;
            let mut index = 0;

            for line in stderr.lines() {
                if line.contains("DirectShow video devices") {
                    in_video_section = true;
                    continue;
                }
                if line.contains("DirectShow audio devices") {
                    break;
                }
                if in_video_section && line.contains("\"") {
                    // Parse device line like '  "HD Webcam"'
                    if let Some(start) = line.find('"') {
                        if let Some(end) = line[start + 1..].find('"') {
                            let name = line[start + 1..start + 1 + end].to_string();
                            cameras.push(CameraInfo {
                                id: index.to_string(),
                                name: name.clone(),
                                device_path: format!("video={}", name),
                                formats: vec![CameraFormat {
                                    width: 1280,
                                    height: 720,
                                    fps: vec![30],
                                    pixel_format: "yuyv422".to_string(),
                                }],
                            });
                            index += 1;
                        }
                    }
                }
            }
        }

        cameras
    }

    #[cfg(target_os = "linux")]
    fn list_cameras_linux(&self) -> Vec<CameraInfo> {
        // List V4L2 devices
        let mut cameras = Vec::new();

        // Check /dev/video* devices
        if let Ok(entries) = std::fs::read_dir("/dev") {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Some(name) = path.file_name() {
                    let name_str = name.to_string_lossy();
                    if name_str.starts_with("video") {
                        let device_path = path.to_string_lossy().to_string();

                        // Try to get device name using v4l2-ctl or ffprobe
                        let display_name = self.get_v4l2_device_name(&device_path)
                            .unwrap_or_else(|| name_str.to_string());

                        cameras.push(CameraInfo {
                            id: name_str.to_string(),
                            name: display_name,
                            device_path,
                            formats: vec![CameraFormat {
                                width: 1280,
                                height: 720,
                                fps: vec![30],
                                pixel_format: "yuyv422".to_string(),
                            }],
                        });
                    }
                }
            }
        }

        cameras
    }

    #[cfg(target_os = "linux")]
    fn get_v4l2_device_name(&self, device_path: &str) -> Option<String> {
        let output = Command::new("v4l2-ctl")
            .args(["--device", device_path, "--info"])
            .output()
            .ok()?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            if line.contains("Card type") {
                return line.split(':').nth(1).map(|s| s.trim().to_string());
            }
        }
        None
    }

    /// Start capturing from a camera
    pub fn start_capture(
        &self,
        camera_id: &str,
        config: CameraCaptureConfig,
    ) -> Result<broadcast::Receiver<Arc<VideoFrame>>, String> {
        // If already capturing this camera, return a new subscriber
        if let Some(rx) = self.sessions.subscribe(camera_id) {
            log::debug!("Reusing existing camera capture for {} (new subscriber)", camera_id);
            return Ok(rx);
        }

        let cameras = self.list_cameras();
        let camera = cameras
            .iter()
            .find(|c| c.id == camera_id)
            .ok_or_else(|| format!("Camera {} not found", camera_id))?;

        // Build FFmpeg command for raw frame capture
        let fps_str = config.fps.to_string();
        let video_size = format!("{}x{}", config.width, config.height);
        let device_path = &camera.device_path;

        let mut args: Vec<&str> = Vec::new();

        #[cfg(target_os = "macos")]
        {
            args.extend([
                "-f", "avfoundation",
                "-framerate", &fps_str,
                "-video_size", &video_size,
                "-i", device_path,
            ]);
        }

        #[cfg(target_os = "windows")]
        {
            args.extend([
                "-f", "dshow",
                "-framerate", &fps_str,
                "-video_size", &video_size,
                "-i", device_path,
            ]);
        }

        #[cfg(target_os = "linux")]
        {
            args.extend([
                "-f", "v4l2",
                "-framerate", &fps_str,
                "-video_size", &video_size,
                "-i", device_path,
            ]);
        }

        // Output raw video frames to stdout (BGRA for consistency with screen capture)
        args.extend([
            "-f", "rawvideo",
            "-pix_fmt", "bgra",
            "-",
        ]);

        let mut process = Command::new(&self.ffmpeg_path)
            .args(&args)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("Failed to start FFmpeg: {}", e))?;

        // Create session with broadcast channel
        let (session, frame_rx) = CaptureSession::<VideoFrame>::new(
            camera_id,
            "camera",
            16,
        );

        let stop_flag = session.stop_flag();
        let frame_tx = session.sender();

        // Start frame reading thread
        let stdout = process.stdout.take().ok_or("Failed to capture stdout")?;
        let width = config.width;
        let height = config.height;

        let capture_handle = std::thread::spawn(move || {
            crate::services::thread_config::set_thread_qos(crate::services::thread_config::QosClass::UserInteractive);
            let frame_size = (width * height * 4) as usize; // BGRA
            let mut reader = BufReader::new(stdout);
            let mut buffer = vec![0u8; frame_size];
            let start_time = std::time::Instant::now();

            while !stop_flag.load(Ordering::Relaxed) {
                // Skip frame reading when no consumers are connected
                if frame_tx.receiver_count() == 0 {
                    std::thread::sleep(std::time::Duration::from_millis(100));
                    continue;
                }
                match reader.read_exact(&mut buffer) {
                    Ok(_) => {
                        let data = Arc::new(buffer.clone());
                        let frame = VideoFrame {
                            data,
                            width,
                            height,
                            pixel_format: "bgra",
                            timestamp_ms: start_time.elapsed().as_millis() as u64,
                        };
                        let _ = frame_tx.send(Arc::new(frame));
                    }
                    Err(_) => break,
                }
            }
        });

        // Store session with thread handle and on_stop callback to kill FFmpeg
        let camera_name = camera.name.clone();
        self.sessions.insert(
            camera_id.to_string(),
            session
                .with_label(&camera_name)
                .with_handle(capture_handle)
                .with_on_stop(move || {
                    let _ = process.kill();
                }),
        );

        log::info!("Started camera capture for {} ({})", camera_id, camera_name);
        Ok(frame_rx)
    }

    /// Stop capturing from a camera
    pub fn stop_capture(&self, camera_id: &str) -> Result<(), String> {
        self.sessions.stop(camera_id)
    }

    /// Stop all active captures
    pub fn stop_all(&self) {
        self.sessions.stop_all();
    }

    /// Check if a camera is currently being captured
    pub fn is_capturing(&self, camera_id: &str) -> bool {
        self.sessions.is_active(camera_id)
    }

    /// Get count of active captures
    pub fn active_capture_count(&self) -> usize {
        self.sessions.active_count()
    }

    /// Get list of active capture IDs with camera names
    pub fn active_captures_info(&self) -> Vec<(String, String)> {
        self.sessions.map_sessions(|id, session| {
            (id.to_string(), session.metadata.label.clone().unwrap_or_default())
        })
    }

    /// Subscribe to an existing camera capture to receive frames
    /// Returns None if the capture doesn't exist
    pub fn subscribe_capture(&self, camera_id: &str) -> Option<broadcast::Receiver<Arc<VideoFrame>>> {
        self.sessions.subscribe(camera_id)
    }
}

// Drop handled by CaptureSessionManager's Drop impl

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_cameras() {
        let service = CameraCaptureService::new("ffmpeg".to_string());
        let cameras = service.list_cameras();
        println!("Found {} cameras", cameras.len());
        for camera in cameras {
            println!("  - {} ({})", camera.name, camera.device_path);
        }
    }
}
