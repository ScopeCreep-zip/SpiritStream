// Camera Capture Service
// Uses FFmpeg for camera capture with platform-specific device access

use std::collections::HashMap;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::io::{BufReader, Read};
use tokio::sync::broadcast;

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
    pub data: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub pixel_format: String,
    pub timestamp_ms: u64,
}

/// Active camera capture
struct ActiveCapture {
    process: Child,
    camera_name: String,
    frame_tx: broadcast::Sender<Arc<VideoFrame>>,
    /// CaptureFrame broadcast for unified pipeline (E2)
    capture_frame_tx: broadcast::Sender<Arc<super::capture_frame::CaptureFrame>>,
    stop_flag: Arc<std::sync::atomic::AtomicBool>,
}

/// Service for managing camera capture via FFmpeg
pub struct CameraCaptureService {
    ffmpeg_path: String,
    active_captures: Mutex<HashMap<String, ActiveCapture>>,
}

impl CameraCaptureService {
    pub fn new(ffmpeg_path: String) -> Self {
        Self {
            ffmpeg_path,
            active_captures: Mutex::new(HashMap::new()),
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
        let cameras = self.list_cameras();
        let camera = cameras
            .iter()
            .find(|c| c.id == camera_id)
            .ok_or_else(|| format!("Camera {} not found", camera_id))?;

        // Build FFmpeg command for raw frame capture
        // Bind strings to variables so they live long enough
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

        // Output raw video frames to stdout
        args.extend([
            "-f", "rawvideo",
            "-pix_fmt", "rgb24",
            "-",
        ]);

        let mut process = Command::new(&self.ffmpeg_path)
            .args(&args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("Failed to start FFmpeg: {}", e))?;

        // Create broadcast channels for frames
        let (frame_tx, frame_rx) = broadcast::channel(16);
        let (capture_frame_tx, _) = broadcast::channel::<Arc<super::capture_frame::CaptureFrame>>(16);
        let stop_flag = Arc::new(std::sync::atomic::AtomicBool::new(false));

        // Spawn stderr reader thread for debuggability
        if let Some(stderr) = process.stderr.take() {
            let camera_id_log = camera_id.to_string();
            std::thread::spawn(move || {
                use std::io::BufRead;
                let reader = BufReader::new(stderr);
                for line in reader.lines() {
                    match line {
                        Ok(line) if !line.trim().is_empty() => {
                            if line.contains("error") || line.contains("Error") || line.contains("Invalid") {
                                log::warn!("[CameraFFmpeg:{}] {}", camera_id_log, line.trim());
                            } else {
                                log::debug!("[CameraFFmpeg:{}] {}", camera_id_log, line.trim());
                            }
                        }
                        Err(_) => break,
                        _ => {}
                    }
                }
            });
        }

        // Start frame reading thread
        let stdout = process.stdout.take().ok_or("Failed to capture stdout")?;
        let tx_clone = frame_tx.clone();
        let cf_tx_clone = capture_frame_tx.clone();
        let stop_flag_clone = stop_flag.clone();
        let width = config.width;
        let height = config.height;

        std::thread::spawn(move || {
            let frame_size = (width * height * 3) as usize; // RGB24
            let mut reader = BufReader::new(stdout);
            let mut buffer = vec![0u8; frame_size];
            let start_time = std::time::Instant::now();

            while !stop_flag_clone.load(std::sync::atomic::Ordering::Relaxed) {
                match reader.read_exact(&mut buffer) {
                    Ok(_) => {
                        let ts = start_time.elapsed().as_millis() as u64;
                        // Swap buffer ownership to avoid clone
                        let data = std::mem::replace(&mut buffer, vec![0u8; frame_size]);

                        // Broadcast as CaptureFrame for unified pipeline (E2)
                        let cf = super::capture_frame::CaptureFrame {
                            data: data.clone(),
                            width,
                            height,
                            pixel_format: super::capture_frame::PixelFormat::RGB24,
                            timestamp_ms: ts,
                        };
                        let _ = cf_tx_clone.send(Arc::new(cf));

                        // Legacy VideoFrame broadcast
                        let frame = VideoFrame {
                            data,
                            width,
                            height,
                            pixel_format: "rgb24".to_string(),
                            timestamp_ms: ts,
                        };
                        let _ = tx_clone.send(Arc::new(frame));
                    }
                    Err(_) => break,
                }
            }
        });

        // Store active capture
        {
            let mut captures = self.active_captures.lock().unwrap();
            captures.insert(
                camera_id.to_string(),
                ActiveCapture {
                    process,
                    camera_name: camera.name.clone(),
                    frame_tx,
                    capture_frame_tx,
                    stop_flag,
                },
            );
        }

        log::info!("Started camera capture for {} ({})", camera_id, camera.name);
        Ok(frame_rx)
    }

    /// Stop capturing from a camera
    pub fn stop_capture(&self, camera_id: &str) -> Result<(), String> {
        let mut captures = self.active_captures.lock().unwrap();

        if let Some(mut capture) = captures.remove(camera_id) {
            capture.stop_flag.store(true, std::sync::atomic::Ordering::Relaxed);
            let _ = capture.process.kill();
            log::info!("Stopped camera capture for {}", camera_id);
            Ok(())
        } else {
            Err(format!("No active capture for camera {}", camera_id))
        }
    }

    /// Stop all active captures
    pub fn stop_all(&self) {
        let mut captures = self.active_captures.lock().unwrap();

        for (id, mut capture) in captures.drain() {
            capture.stop_flag.store(true, std::sync::atomic::Ordering::Relaxed);
            let _ = capture.process.kill();
            log::info!("Stopped camera capture for {}", id);
        }
    }

    /// Check if a camera is currently being captured
    pub fn is_capturing(&self, camera_id: &str) -> bool {
        let captures = self.active_captures.lock().unwrap();
        captures.contains_key(camera_id)
    }

    /// Get count of active captures
    pub fn active_capture_count(&self) -> usize {
        let captures = self.active_captures.lock().unwrap();
        captures.len()
    }

    /// Get list of active capture IDs with camera names
    pub fn active_captures_info(&self) -> Vec<(String, String)> {
        let captures = self.active_captures.lock().unwrap();
        captures
            .iter()
            .map(|(id, capture)| (id.clone(), capture.camera_name.clone()))
            .collect()
    }

    /// Subscribe to an existing camera capture to receive frames
    /// Returns None if the capture doesn't exist
    pub fn subscribe_capture(&self, camera_id: &str) -> Option<broadcast::Receiver<Arc<VideoFrame>>> {
        let captures = self.active_captures.lock().unwrap();
        captures.get(camera_id).map(|c| c.frame_tx.subscribe())
    }

    /// Subscribe to an existing camera capture to receive CaptureFrame (unified pipeline)
    /// Returns None if the capture doesn't exist
    pub fn subscribe_capture_frames(&self, camera_id: &str) -> Option<broadcast::Receiver<Arc<super::capture_frame::CaptureFrame>>> {
        let captures = self.active_captures.lock().unwrap();
        captures.get(camera_id).map(|c| c.capture_frame_tx.subscribe())
    }
}

impl Drop for CameraCaptureService {
    fn drop(&mut self) {
        self.stop_all();
    }
}

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
