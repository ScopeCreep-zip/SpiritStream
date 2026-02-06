// Native Preview Service
// Encodes raw frames from native capture to JPEG for preview

use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use tokio::sync::broadcast;
use bytes::Bytes;

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
    frame_tx: broadcast::Sender<Bytes>,
    last_accessed: Instant,
}

/// Service for generating JPEG previews from native capture frames
pub struct NativePreviewService {
    active_previews: Mutex<HashMap<String, ActivePreview>>,
}

impl NativePreviewService {
    pub fn new() -> Self {
        Self {
            active_previews: Mutex::new(HashMap::new()),
        }
    }

    /// Start a preview for a camera source
    /// Takes frames from the camera capture and encodes them to JPEG
    pub fn start_camera_preview(
        &self,
        device_id: &str,
        mut frame_rx: broadcast::Receiver<Arc<crate::services::camera_capture::VideoFrame>>,
        config: NativePreviewConfig,
    ) -> Result<broadcast::Receiver<Bytes>, String> {
        let preview_id = format!("camera_{}", device_id);

        // Check if already running
        {
            let previews = self.active_previews.lock()
                .map_err(|e| format!("Lock poisoned: {}", e))?;

            if let Some(preview) = previews.get(&preview_id) {
                // Return existing receiver
                return Ok(preview.frame_tx.subscribe());
            }
        }

        // Create broadcast channel for JPEG frames
        let (frame_tx, frame_rx_out) = broadcast::channel::<Bytes>(16);
        let stop_flag = Arc::new(AtomicBool::new(false));
        let stop_flag_clone = stop_flag.clone();
        let preview_id_clone = preview_id.clone();
        let tx_clone = frame_tx.clone();

        // Spawn encoding thread
        std::thread::spawn(move || {
            let frame_interval_ms = 1000 / config.fps.max(1);
            let mut last_frame_time = Instant::now();

            while !stop_flag_clone.load(Ordering::Relaxed) {
                match frame_rx.blocking_recv() {
                    Ok(frame) => {
                        // Rate limit
                        let elapsed = last_frame_time.elapsed().as_millis() as u32;
                        if elapsed < frame_interval_ms {
                            continue;
                        }
                        last_frame_time = Instant::now();

                        // Encode frame to JPEG
                        match Self::encode_rgb_to_jpeg(
                            &frame.data,
                            frame.width,
                            frame.height,
                            config.width,
                            config.height,
                            config.quality,
                        ) {
                            Ok(jpeg_data) => {
                                if tx_clone.send(Bytes::from(jpeg_data)).is_err() {
                                    // No receivers
                                    log::debug!("[NativePreview:{}] No receivers, stopping", preview_id_clone);
                                    break;
                                }
                            }
                            Err(e) => {
                                log::warn!("[NativePreview:{}] Encode error: {}", preview_id_clone, e);
                            }
                        }
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        log::info!("[NativePreview:{}] Source closed", preview_id_clone);
                        break;
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        log::debug!("[NativePreview:{}] Lagged {} frames", preview_id_clone, n);
                    }
                }
            }

            log::info!("[NativePreview:{}] Preview thread stopped", preview_id_clone);
        });

        // Store active preview
        {
            let mut previews = self.active_previews.lock()
                .map_err(|e| format!("Lock poisoned: {}", e))?;

            previews.insert(preview_id.clone(), ActivePreview {
                stop_flag,
                frame_tx,
                last_accessed: Instant::now(),
            });
        }

        log::info!("[NativePreview:{}] Started camera preview", preview_id);
        Ok(frame_rx_out)
    }

    /// Start a preview for a screen capture source
    pub fn start_screen_preview(
        &self,
        display_id: &str,
        mut frame_rx: broadcast::Receiver<Arc<scap::frame::Frame>>,
        config: NativePreviewConfig,
    ) -> Result<broadcast::Receiver<Bytes>, String> {
        let preview_id = format!("display_{}", display_id);

        // Check if already running
        {
            let previews = self.active_previews.lock()
                .map_err(|e| format!("Lock poisoned: {}", e))?;

            if let Some(preview) = previews.get(&preview_id) {
                return Ok(preview.frame_tx.subscribe());
            }
        }

        // Create broadcast channel for JPEG frames
        let (frame_tx, frame_rx_out) = broadcast::channel::<Bytes>(16);
        let stop_flag = Arc::new(AtomicBool::new(false));
        let stop_flag_clone = stop_flag.clone();
        let preview_id_clone = preview_id.clone();
        let tx_clone = frame_tx.clone();

        // Spawn encoding thread
        std::thread::spawn(move || {
            let frame_interval_ms = 1000 / config.fps.max(1);
            let mut last_frame_time = Instant::now();

            while !stop_flag_clone.load(Ordering::Relaxed) {
                match frame_rx.blocking_recv() {
                    Ok(frame) => {
                        // Rate limit
                        let elapsed = last_frame_time.elapsed().as_millis() as u32;
                        if elapsed < frame_interval_ms {
                            continue;
                        }
                        last_frame_time = Instant::now();

                        // Extract frame data based on type
                        if let Some((data, width, height)) = Self::extract_scap_frame_data(&frame) {
                            match Self::encode_bgra_to_jpeg(
                                &data,
                                width,
                                height,
                                config.width,
                                config.height,
                                config.quality,
                            ) {
                                Ok(jpeg_data) => {
                                    if tx_clone.send(Bytes::from(jpeg_data)).is_err() {
                                        log::debug!("[NativePreview:{}] No receivers, stopping", preview_id_clone);
                                        break;
                                    }
                                }
                                Err(e) => {
                                    log::warn!("[NativePreview:{}] Encode error: {}", preview_id_clone, e);
                                }
                            }
                        }
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        log::info!("[NativePreview:{}] Source closed", preview_id_clone);
                        break;
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        log::debug!("[NativePreview:{}] Lagged {} frames", preview_id_clone, n);
                    }
                }
            }

            log::info!("[NativePreview:{}] Preview thread stopped", preview_id_clone);
        });

        // Store active preview
        {
            let mut previews = self.active_previews.lock()
                .map_err(|e| format!("Lock poisoned: {}", e))?;

            previews.insert(preview_id.clone(), ActivePreview {
                stop_flag,
                frame_tx,
                last_accessed: Instant::now(),
            });
        }

        log::info!("[NativePreview:{}] Started screen preview", preview_id);
        Ok(frame_rx_out)
    }

    /// Start a unified preview from CaptureFrame source (E4)
    /// Works for any capture source type (camera, screen, capture card)
    pub fn start_preview(
        &self,
        preview_id: &str,
        mut frame_rx: broadcast::Receiver<Arc<super::capture_frame::CaptureFrame>>,
        config: NativePreviewConfig,
    ) -> Result<broadcast::Receiver<Bytes>, String> {
        // Check if already running
        {
            let previews = self.active_previews.lock()
                .map_err(|e| format!("Lock poisoned: {}", e))?;

            if let Some(preview) = previews.get(preview_id) {
                return Ok(preview.frame_tx.subscribe());
            }
        }

        let (frame_tx, frame_rx_out) = broadcast::channel::<Bytes>(16);
        let stop_flag = Arc::new(AtomicBool::new(false));
        let stop_flag_clone = stop_flag.clone();
        let preview_id_clone = preview_id.to_string();
        let tx_clone = frame_tx.clone();

        std::thread::spawn(move || {
            let frame_interval_ms = 1000 / config.fps.max(1);
            let mut last_frame_time = Instant::now();

            while !stop_flag_clone.load(Ordering::Relaxed) {
                match frame_rx.blocking_recv() {
                    Ok(frame) => {
                        // Rate limit
                        let elapsed = last_frame_time.elapsed().as_millis() as u32;
                        if elapsed < frame_interval_ms {
                            continue;
                        }
                        last_frame_time = Instant::now();

                        if frame.validate().is_err() {
                            continue;
                        }

                        // Encode based on pixel format
                        let result = match frame.pixel_format {
                            super::capture_frame::PixelFormat::RGB24 => {
                                Self::encode_rgb_to_jpeg(
                                    &frame.data, frame.width, frame.height,
                                    config.width, config.height, config.quality,
                                )
                            }
                            super::capture_frame::PixelFormat::BGRA => {
                                Self::encode_bgra_to_jpeg(
                                    &frame.data, frame.width, frame.height,
                                    config.width, config.height, config.quality,
                                )
                            }
                            super::capture_frame::PixelFormat::NV12 => {
                                // Convert NV12 to BGRA then encode
                                let (y_len, _uv_len) = (
                                    (frame.width * frame.height) as usize,
                                    (frame.width * frame.height / 2) as usize,
                                );
                                if frame.data.len() >= y_len {
                                    let y_plane = &frame.data[..y_len];
                                    let uv_plane = &frame.data[y_len..];
                                    let bgra = Self::yuv_to_bgra(
                                        y_plane, uv_plane,
                                        frame.width as usize, frame.height as usize,
                                    );
                                    Self::encode_bgra_to_jpeg(
                                        &bgra, frame.width, frame.height,
                                        config.width, config.height, config.quality,
                                    )
                                } else {
                                    Err("NV12 data too short".to_string())
                                }
                            }
                        };

                        match result {
                            Ok(jpeg_data) => {
                                if tx_clone.send(Bytes::from(jpeg_data)).is_err() {
                                    log::debug!("[NativePreview:{}] No receivers, stopping", preview_id_clone);
                                    break;
                                }
                            }
                            Err(e) => {
                                log::warn!("[NativePreview:{}] Encode error: {}", preview_id_clone, e);
                            }
                        }
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        log::info!("[NativePreview:{}] Source closed", preview_id_clone);
                        break;
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        log::debug!("[NativePreview:{}] Lagged {} frames", preview_id_clone, n);
                    }
                }
            }

            log::info!("[NativePreview:{}] Preview thread stopped", preview_id_clone);
        });

        {
            let mut previews = self.active_previews.lock()
                .map_err(|e| format!("Lock poisoned: {}", e))?;

            previews.insert(preview_id.to_string(), ActivePreview {
                stop_flag,
                frame_tx,
                last_accessed: Instant::now(),
            });
        }

        log::info!("[NativePreview:{}] Started unified preview", preview_id);
        Ok(frame_rx_out)
    }

    /// Stop a preview by ID
    pub fn stop_preview(&self, preview_id: &str) {
        if let Ok(mut previews) = self.active_previews.lock() {
            if let Some(preview) = previews.remove(preview_id) {
                preview.stop_flag.store(true, Ordering::Relaxed);
                log::info!("[NativePreview:{}] Stopped", preview_id);
            }
        }
    }

    /// Stop all active previews
    pub fn stop_all(&self) {
        if let Ok(mut previews) = self.active_previews.lock() {
            for (id, preview) in previews.drain() {
                preview.stop_flag.store(true, Ordering::Relaxed);
                log::info!("[NativePreview:{}] Stopped", id);
            }
        }
    }

    /// Get count of active previews
    pub fn active_count(&self) -> usize {
        self.active_previews.lock()
            .map(|p| p.len())
            .unwrap_or(0)
    }

    /// Get active preview IDs
    pub fn active_preview_ids(&self) -> Vec<String> {
        self.active_previews.lock()
            .map(|p| p.keys().cloned().collect())
            .unwrap_or_default()
    }

    /// Subscribe to an existing preview's JPEG frame stream
    /// Returns None if the preview doesn't exist
    pub fn subscribe_preview(&self, preview_id: &str) -> Option<broadcast::Receiver<Bytes>> {
        let mut previews = self.active_previews.lock().ok()?;

        if let Some(preview) = previews.get_mut(preview_id) {
            preview.last_accessed = Instant::now();
            Some(preview.frame_tx.subscribe())
        } else {
            None
        }
    }

    /// Check if a preview exists
    pub fn has_preview(&self, preview_id: &str) -> bool {
        self.active_previews.lock()
            .map(|p| p.contains_key(preview_id))
            .unwrap_or(false)
    }

    /// Extract frame data from scap Frame (zero-copy for BGRA, conversion for others)
    fn extract_scap_frame_data(frame: &scap::frame::Frame) -> Option<(Cow<'_, [u8]>, u32, u32)> {
        match frame {
            scap::frame::Frame::BGRA(bgra_frame) => {
                Some((
                    Cow::Borrowed(&bgra_frame.data),
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
                Some((Cow::Owned(bgra), rgb_frame.width as u32, rgb_frame.height as u32))
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
                Some((Cow::Owned(bgra), width as u32, height as u32))
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
    fn encode_rgb_to_jpeg(
        data: &[u8],
        src_width: u32,
        src_height: u32,
        dst_width: u32,
        dst_height: u32,
        quality: u8,
    ) -> Result<Vec<u8>, String> {
        use image::{ImageBuffer, Rgb, imageops::FilterType};

        // Create image buffer from RGB data
        let img: ImageBuffer<Rgb<u8>, Vec<u8>> = ImageBuffer::from_raw(src_width, src_height, data.to_vec())
            .ok_or_else(|| "Failed to create image buffer".to_string())?;

        // Scale if needed
        let scaled = if src_width != dst_width || src_height != dst_height {
            image::DynamicImage::ImageRgb8(img)
                .resize_exact(dst_width, dst_height, FilterType::Triangle)
                .to_rgb8()
        } else {
            img
        };

        // Encode to JPEG
        let mut jpeg_data = Vec::new();
        let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg_data, quality);
        encoder.encode(
            scaled.as_raw(),
            dst_width,
            dst_height,
            image::ExtendedColorType::Rgb8,
        ).map_err(|e| format!("JPEG encode error: {}", e))?;

        Ok(jpeg_data)
    }

    /// Encode BGRA frame to JPEG with optional scaling
    fn encode_bgra_to_jpeg(
        data: &[u8],
        src_width: u32,
        src_height: u32,
        dst_width: u32,
        dst_height: u32,
        quality: u8,
    ) -> Result<Vec<u8>, String> {
        use image::{ImageBuffer, Rgba, imageops::FilterType};

        // Create image buffer from BGRA data
        let img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::from_raw(src_width, src_height, data.to_vec())
            .ok_or_else(|| "Failed to create image buffer".to_string())?;

        // Convert BGRA to RGB for JPEG
        let rgb_data: Vec<u8> = img.pixels().flat_map(|p| {
            // BGRA -> RGB
            [p[2], p[1], p[0]]
        }).collect();

        let rgb_img: ImageBuffer<image::Rgb<u8>, Vec<u8>> =
            ImageBuffer::from_raw(src_width, src_height, rgb_data)
                .ok_or_else(|| "Failed to create RGB buffer".to_string())?;

        // Scale if needed
        let scaled = if src_width != dst_width || src_height != dst_height {
            image::DynamicImage::ImageRgb8(rgb_img)
                .resize_exact(dst_width, dst_height, FilterType::Triangle)
                .to_rgb8()
        } else {
            rgb_img
        };

        // Encode to JPEG
        let mut jpeg_data = Vec::new();
        let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg_data, quality);
        encoder.encode(
            scaled.as_raw(),
            dst_width,
            dst_height,
            image::ExtendedColorType::Rgb8,
        ).map_err(|e| format!("JPEG encode error: {}", e))?;

        Ok(jpeg_data)
    }
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
