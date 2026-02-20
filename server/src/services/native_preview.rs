// Native Preview Service
// Encodes raw frames from native capture to JPEG for preview

use std::collections::HashMap;
use std::sync::Arc;
use parking_lot::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use tokio::sync::broadcast;
use bytes::Bytes;
use crate::services::ActivityAssertion;

/// Preview configuration
#[derive(Debug, Clone)]
pub struct NativePreviewConfig {
    pub width: u32,
    pub height: u32,
    pub quality: u8, // 1-100
    pub fps: u32,
}

impl Default for NativePreviewConfig {
    fn default() -> Self {
        Self {
            width: 640,
            height: 360,
            quality: 75,
            fps: 15,
        }
    }
}

/// Active preview session
struct ActivePreview {
    stop_flag: Arc<AtomicBool>,
    /// Per-thread idle flag — set_idle() propagates to all active previews
    idle_flag: Arc<AtomicBool>,
    frame_tx: broadcast::Sender<Bytes>,
    last_accessed: Instant,
    /// Thread handle for clean shutdown (joined on stop)
    encode_handle: Option<std::thread::JoinHandle<()>>,
}

/// Default throttled FPS for thermal throttling (5fps)
const THROTTLED_PREVIEW_FPS: u32 = 5;

/// Service for generating JPEG previews from native capture frames
pub struct NativePreviewService {
    active_previews: Mutex<HashMap<String, ActivePreview>>,
    /// When true, the app is in the background and previews should be throttled.
    /// Arc so it can be shared with AudioLevelService for coordinated throttling.
    idle: Arc<AtomicBool>,
    /// When true, system is under thermal pressure — reduce preview FPS.
    /// Swappable so PowerBudgetManager can provide its own flag after construction.
    throttle: Mutex<Arc<AtomicBool>>,
    /// Prevents macOS App Nap from throttling capture threads while previews are active
    activity_assertion: Mutex<Option<ActivityAssertion>>,
}

impl NativePreviewService {
    pub fn new() -> Self {
        Self {
            active_previews: Mutex::new(HashMap::new()),
            idle: Arc::new(AtomicBool::new(false)),
            throttle: Mutex::new(Arc::new(AtomicBool::new(false))),
            activity_assertion: Mutex::new(None),
        }
    }

    /// Set the thermal throttle flag from PowerBudgetManager.
    /// Must be called after both NativePreviewService and PowerBudgetManager are created.
    /// New preview threads will share this flag with the thermal monitor.
    pub fn set_throttle_flag(&self, flag: Arc<AtomicBool>) {
        *self.throttle.lock() = flag;
    }

    /// Acquire App Nap prevention when first preview starts
    fn acquire_activity_assertion(&self) {
        {
            let mut guard = self.activity_assertion.lock();
            if guard.is_none() {
                match ActivityAssertion::begin("SpiritStream preview capture active") {
                    Ok(assertion) => *guard = Some(assertion),
                    Err(e) => log::warn!("Failed to acquire activity assertion: {}", e),
                }
            }
        }
    }

    /// Release App Nap prevention when all previews stop
    fn release_activity_assertion_if_idle(&self) {
        let is_empty = self.active_previews.lock().is_empty();
        if is_empty {
            let mut guard = self.activity_assertion.lock();
            if guard.take().is_some() {
                log::info!("Released preview activity assertion");
            }
        }
    }

    /// Get a shared reference to the idle flag for other services
    /// (e.g., AudioLevelService) to coordinate throttling.
    pub fn idle_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.idle)
    }

    /// Set idle mode. When idle, preview encoding threads will reduce their
    /// frame rate to save CPU/GPU while the UI tab is hidden.
    pub fn set_idle(&self, idle: bool) {
        self.idle.store(idle, Ordering::Relaxed);
        // Propagate to all active preview encoding threads
        {
            let previews = self.active_previews.lock();
            for preview in previews.values() {
                preview.idle_flag.store(idle, Ordering::Relaxed);
            }
        }
        log::info!("[NativePreview] Idle mode set to {} ({} active previews)",
            idle, self.active_previews.lock().len());
    }

    /// Check if currently in idle mode
    pub fn is_idle(&self) -> bool {
        self.idle.load(Ordering::Relaxed)
    }

    /// Start a preview for a camera source
    /// Takes frames from the camera capture and encodes them to JPEG
    pub fn start_camera_preview(
        &self,
        device_id: &str,
        frame_rx: broadcast::Receiver<Arc<crate::services::camera_capture::VideoFrame>>,
        config: NativePreviewConfig,
    ) -> Result<broadcast::Receiver<Bytes>, String> {
        let preview_id = format!("camera_{}", device_id);

        // Camera frames are BGRA — extract pixel data directly
        self.start_preview_internal(preview_id, frame_rx, config, |frame| {
            Some(((*frame.data).clone(), frame.width, frame.height))
        })
    }

    /// Start a preview for a screen capture source
    pub fn start_screen_preview(
        &self,
        display_id: &str,
        frame_rx: broadcast::Receiver<Arc<scap::frame::Frame>>,
        config: NativePreviewConfig,
    ) -> Result<broadcast::Receiver<Bytes>, String> {
        let preview_id = format!("display_{}", display_id);

        // Screen frames may be BGRA/RGB/YUV — extract and convert to BGRA
        self.start_preview_internal(preview_id, frame_rx, config, |frame| {
            Self::extract_scap_frame_data(frame)
        })
    }

    /// Internal helper: starts a preview for any frame type.
    /// `extract_fn` converts each received frame into BGRA pixel data (data, width, height).
    fn start_preview_internal<T, F>(
        &self,
        preview_id: String,
        frame_rx: broadcast::Receiver<T>,
        config: NativePreviewConfig,
        extract_fn: F,
    ) -> Result<broadcast::Receiver<Bytes>, String>
    where
        T: Clone + Send + 'static,
        F: Fn(&T) -> Option<(Vec<u8>, u32, u32)> + Send + 'static,
    {
        self.acquire_activity_assertion();

        // Check if already running
        {
            let previews = self.active_previews.lock();
            if let Some(preview) = previews.get(&preview_id) {
                return Ok(preview.frame_tx.subscribe());
            }
        }

        // Create broadcast channel for JPEG frames
        let (frame_tx, frame_rx_out) = broadcast::channel::<Bytes>(16);
        let stop_flag = Arc::new(AtomicBool::new(false));
        let idle_flag = Arc::new(AtomicBool::new(self.idle.load(Ordering::Relaxed)));

        let stop_clone = stop_flag.clone();
        let idle_clone = idle_flag.clone();
        let throttle_clone = Arc::clone(&self.throttle.lock());
        let tx_clone = frame_tx.clone();
        let pid = preview_id.clone();

        let encode_handle = std::thread::spawn(move || {
            run_preview_loop(frame_rx, extract_fn, tx_clone, stop_clone, idle_clone, throttle_clone, config, pid);
        });

        // Store active preview
        {
            let mut previews = self.active_previews.lock();
            previews.insert(preview_id.clone(), ActivePreview {
                stop_flag,
                idle_flag,
                frame_tx,
                last_accessed: Instant::now(),
                encode_handle: Some(encode_handle),
            });
        }

        log::info!("[NativePreview:{}] Started preview", preview_id);
        Ok(frame_rx_out)
    }

    /// Stop a preview by ID
    pub fn stop_preview(&self, preview_id: &str) {
        let handle = {
            let mut previews = self.active_previews.lock();
            if let Some(mut preview) = previews.remove(preview_id) {
                preview.stop_flag.store(true, Ordering::Relaxed);
                log::info!("[NativePreview:{}] Stopped", preview_id);
                preview.encode_handle.take()
            } else {
                None
            }
        };
        // Join thread outside the lock to avoid deadlock
        if let Some(handle) = handle {
            let _ = handle.join();
        }
        self.release_activity_assertion_if_idle();
    }

    /// Stop all active previews
    pub fn stop_all(&self) {
        let handles: Vec<_> = {
            let mut previews = self.active_previews.lock();
            previews.drain().map(|(id, mut preview)| {
                preview.stop_flag.store(true, Ordering::Relaxed);
                log::info!("[NativePreview:{}] Stopped", id);
                preview.encode_handle.take()
            }).collect()
        };
        // Join all threads outside the lock
        for handle in handles.into_iter().flatten() {
            let _ = handle.join();
        }
        // Release activity assertion since all previews stopped
        {
            let mut guard = self.activity_assertion.lock();
            if guard.take().is_some() {
                log::info!("Released preview activity assertion (stop_all)");
            }
        }
    }

    /// Get count of active previews
    pub fn active_count(&self) -> usize {
        self.active_previews.lock().len()
    }

    /// Get active preview IDs
    pub fn active_preview_ids(&self) -> Vec<String> {
        self.active_previews.lock().keys().cloned().collect()
    }

    /// Subscribe to an existing preview's JPEG frame stream
    /// Returns None if the preview doesn't exist
    pub fn subscribe_preview(&self, preview_id: &str) -> Option<broadcast::Receiver<Bytes>> {
        let mut previews = self.active_previews.lock();

        if let Some(preview) = previews.get_mut(preview_id) {
            preview.last_accessed = Instant::now();
            Some(preview.frame_tx.subscribe())
        } else {
            None
        }
    }

    /// Check if a preview exists
    pub fn has_preview(&self, preview_id: &str) -> bool {
        self.active_previews.lock().contains_key(preview_id)
    }

    /// Extract frame data from scap Frame
    fn extract_scap_frame_data(frame: &scap::frame::Frame) -> Option<(Vec<u8>, u32, u32)> {
        match frame {
            scap::frame::Frame::BGRA(bgra_frame) => {
                Some((
                    bgra_frame.data.clone(),
                    bgra_frame.width as u32,
                    bgra_frame.height as u32,
                ))
            }
            scap::frame::Frame::RGB(rgb_frame) => {
                // Convert RGB to BGRA for consistent processing
                let mut bgra = Vec::with_capacity(rgb_frame.data.len() * 4 / 3);
                for chunk in rgb_frame.data.chunks(3) {
                    if chunk.len() == 3 {
                        bgra.push(chunk[2]); // B
                        bgra.push(chunk[1]); // G
                        bgra.push(chunk[0]); // R
                        bgra.push(255);      // A
                    }
                }
                Some((bgra, rgb_frame.width as u32, rgb_frame.height as u32))
            }
            scap::frame::Frame::YUVFrame(yuv_frame) => {
                // Convert YUV to BGRA
                let width = yuv_frame.width as usize;
                let height = yuv_frame.height as usize;
                let bgra = Self::yuv_to_bgra(
                    &yuv_frame.luminance_bytes,
                    &yuv_frame.chrominance_bytes,
                    width,
                    height,
                );
                Some((bgra, width as u32, height as u32))
            }
            _ => {
                log::warn!("[NativePreview] Unsupported frame type");
                None
            }
        }
    }

    /// Convert YUV (NV12) to BGRA
    fn yuv_to_bgra(y_plane: &[u8], uv_plane: &[u8], width: usize, height: usize) -> Vec<u8> {
        let mut bgra = vec![0u8; width * height * 4];

        for row in 0..height {
            for col in 0..width {
                let y_idx = row * width + col;
                let uv_idx = (row / 2) * width + (col / 2) * 2;

                let y = y_plane.get(y_idx).copied().unwrap_or(0) as f32;
                let u = uv_plane.get(uv_idx).copied().unwrap_or(128) as f32 - 128.0;
                let v = uv_plane.get(uv_idx + 1).copied().unwrap_or(128) as f32 - 128.0;

                // YUV to RGB conversion
                let r = (y + 1.402 * v).clamp(0.0, 255.0) as u8;
                let g = (y - 0.344136 * u - 0.714136 * v).clamp(0.0, 255.0) as u8;
                let b = (y + 1.772 * u).clamp(0.0, 255.0) as u8;

                let bgra_idx = (row * width + col) * 4;
                bgra[bgra_idx] = b;
                bgra[bgra_idx + 1] = g;
                bgra[bgra_idx + 2] = r;
                bgra[bgra_idx + 3] = 255;
            }
        }

        bgra
    }

    /// Encode RGB24 frame to JPEG with optional scaling
    /// Uses turbojpeg (libjpeg-turbo) for SIMD-accelerated encoding (~2-4ms release vs ~15-30ms image crate)
    #[allow(dead_code)]
    fn encode_rgb_to_jpeg(
        data: &[u8],
        src_width: u32,
        src_height: u32,
        dst_width: u32,
        dst_height: u32,
        quality: u8,
    ) -> Result<Vec<u8>, String> {
        // Scale if needed using image crate (turbojpeg doesn't have built-in resize)
        let (encode_data, encode_w, encode_h) = if src_width != dst_width || src_height != dst_height {
            use image::{ImageBuffer, Rgb, imageops::FilterType};
            let img: ImageBuffer<Rgb<u8>, Vec<u8>> = ImageBuffer::from_raw(src_width, src_height, data.to_vec())
                .ok_or_else(|| "Failed to create image buffer".to_string())?;
            let scaled = image::DynamicImage::ImageRgb8(img)
                .resize_exact(dst_width, dst_height, FilterType::Triangle)
                .to_rgb8();
            (std::borrow::Cow::Owned(scaled.into_raw()), dst_width as usize, dst_height as usize)
        } else {
            (std::borrow::Cow::Borrowed(data), src_width as usize, src_height as usize)
        };

        let image = turbojpeg::Image {
            pixels: encode_data.as_ref(),
            width: encode_w,
            height: encode_h,
            pitch: encode_w * 3,
            format: turbojpeg::PixelFormat::RGB,
        };

        let mut compressor = turbojpeg::Compressor::new().map_err(|e| e.to_string())?;
        compressor.set_quality(quality as i32).map_err(|e| e.to_string())?;
        compressor.compress_to_vec(image).map_err(|e| e.to_string())
    }

    /// Encode BGRA frame to JPEG with optional scaling
    /// Uses turbojpeg (libjpeg-turbo) for SIMD-accelerated BGRA encoding — no manual pixel conversion needed
    fn encode_bgra_to_jpeg(
        data: &[u8],
        src_width: u32,
        src_height: u32,
        dst_width: u32,
        dst_height: u32,
        quality: u8,
    ) -> Result<Vec<u8>, String> {
        // Scale if needed using image crate (turbojpeg doesn't have built-in resize)
        let (encode_data, encode_w, encode_h, format) = if src_width != dst_width || src_height != dst_height {
            use image::{ImageBuffer, Rgba, imageops::FilterType};
            let img: ImageBuffer<Rgba<u8>, _> = ImageBuffer::from_raw(src_width, src_height, data)
                .ok_or_else(|| "Failed to create image buffer".to_string())?;
            // Convert BGRA→RGBA for image crate resize, then encode as RGBA via turbojpeg
            let rgba_data: Vec<u8> = img.pixels().flat_map(|p| [p[2], p[1], p[0], p[3]]).collect();
            let rgba_img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::from_raw(src_width, src_height, rgba_data)
                .ok_or_else(|| "Failed to create RGBA buffer".to_string())?;
            let scaled = image::DynamicImage::ImageRgba8(rgba_img)
                .resize_exact(dst_width, dst_height, FilterType::Triangle);
            let rgb = scaled.to_rgb8();
            (std::borrow::Cow::Owned(rgb.into_raw()), dst_width as usize, dst_height as usize, turbojpeg::PixelFormat::RGB)
        } else {
            // No scaling needed — turbojpeg handles BGRA natively via SIMD, no conversion needed
            (std::borrow::Cow::Borrowed(data), src_width as usize, src_height as usize, turbojpeg::PixelFormat::BGRA)
        };

        let image = turbojpeg::Image {
            pixels: encode_data.as_ref(),
            width: encode_w,
            height: encode_h,
            pitch: encode_w * if format == turbojpeg::PixelFormat::BGRA { 4 } else { 3 },
            format,
        };

        let mut compressor = turbojpeg::Compressor::new().map_err(|e| e.to_string())?;
        compressor.set_quality(quality as i32).map_err(|e| e.to_string())?;
        compressor.compress_to_vec(image).map_err(|e| e.to_string())
    }
}

/// Run preview encoding loop, generic over frame type.
/// `extract_fn` converts each received frame into BGRA pixel data (data, width, height).
fn run_preview_loop<T, F>(
    mut frame_rx: broadcast::Receiver<T>,
    extract_fn: F,
    tx: broadcast::Sender<Bytes>,
    stop_flag: Arc<AtomicBool>,
    idle_flag: Arc<AtomicBool>,
    throttle_flag: Arc<AtomicBool>,
    config: NativePreviewConfig,
    preview_id: String,
) where
    T: Clone + Send + 'static,
    F: Fn(&T) -> Option<(Vec<u8>, u32, u32)> + Send + 'static,
{
    crate::services::thread_config::set_thread_qos(crate::services::thread_config::QosClass::UserInitiated);
    let normal_interval_ms = 1000 / config.fps.max(1);
    let throttled_interval_ms = 1000 / THROTTLED_PREVIEW_FPS.max(1);
    let idle_interval_ms: u32 = 500; // 2fps when idle
    let mut last_frame_time = Instant::now();

    while !stop_flag.load(Ordering::Relaxed) {
        match frame_rx.blocking_recv() {
            Ok(frame) => {
                // Rate limit — use slower rate when idle or thermally throttled
                let frame_interval_ms = if idle_flag.load(Ordering::Relaxed) {
                    idle_interval_ms
                } else if throttle_flag.load(Ordering::Relaxed) {
                    throttled_interval_ms
                } else {
                    normal_interval_ms
                };
                if (last_frame_time.elapsed().as_millis() as u32) < frame_interval_ms {
                    continue;
                }
                last_frame_time = Instant::now();

                if let Some((data, width, height)) = extract_fn(&frame) {
                    match NativePreviewService::encode_bgra_to_jpeg(
                        &data, width, height, config.width, config.height, config.quality,
                    ) {
                        Ok(jpeg) => {
                            if tx.send(Bytes::from(jpeg)).is_err() {
                                log::debug!("[NativePreview:{}] No receivers, stopping", preview_id);
                                break;
                            }
                        }
                        Err(e) => log::warn!("[NativePreview:{}] Encode error: {}", preview_id, e),
                    }
                }
            }
            Err(broadcast::error::RecvError::Closed) => {
                log::info!("[NativePreview:{}] Source closed", preview_id);
                break;
            }
            Err(broadcast::error::RecvError::Lagged(n)) => {
                log::debug!("[NativePreview:{}] Lagged {} frames", preview_id, n);
            }
        }
    }

    log::info!("[NativePreview:{}] Preview thread stopped", preview_id);
}

impl Default for NativePreviewService {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for NativePreviewService {
    fn drop(&mut self) {
        self.stop_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = NativePreviewConfig::default();
        assert_eq!(config.width, 640);
        assert_eq!(config.height, 360);
        assert_eq!(config.quality, 75);
        assert_eq!(config.fps, 15);
    }
}
