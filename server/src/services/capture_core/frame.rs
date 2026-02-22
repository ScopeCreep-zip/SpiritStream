// Capture Frame Types
//
// Unified frame metadata and type wrappers shared across capture services.
// Consolidates get_frame_dimensions() and extract_frame_data() from
// frame_processing.rs into a more structured interface.

use scap::frame::Frame;
use std::time::Instant;

/// Pixel format of captured frame data
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelFormat {
    Bgra,
    Rgb,
    Yuv,
    /// Encoded data (H.264, MPEG-TS, JPEG) — not raw pixels
    Encoded,
}

/// Common metadata for any captured frame
#[derive(Debug, Clone)]
pub struct FrameInfo {
    pub width: u32,
    pub height: u32,
    pub format: PixelFormat,
    pub timestamp: Instant,
}

impl FrameInfo {
    /// Expected raw data size in bytes (width * height * bytes_per_pixel)
    /// Returns None for encoded formats where size isn't predictable.
    pub fn expected_size(&self) -> Option<usize> {
        let bpp = match self.format {
            PixelFormat::Bgra => 4,
            PixelFormat::Rgb => 3,
            PixelFormat::Yuv => 2, // NV12 = 1.5, but padded rows often round to 2
            PixelFormat::Encoded => return None,
        };
        Some(self.width as usize * self.height as usize * bpp)
    }
}

/// Extract dimensions from a scap Frame.
///
/// Returns (width, height) in pixels. Falls back to (1920, 1080) for
/// unrecognized formats.
pub fn get_frame_dimensions(frame: &Frame) -> (u32, u32) {
    match frame {
        Frame::BGRA(bgra) => (bgra.width as u32, bgra.height as u32),
        Frame::RGB(rgb) => (rgb.width as u32, rgb.height as u32),
        Frame::YUVFrame(yuv) => (yuv.width as u32, yuv.height as u32),
        _ => (1920, 1080),
    }
}

/// Extract the pixel format from a scap Frame.
pub fn get_frame_format(frame: &Frame) -> PixelFormat {
    match frame {
        Frame::BGRA(_) => PixelFormat::Bgra,
        Frame::RGB(_) => PixelFormat::Rgb,
        Frame::YUVFrame(_) => PixelFormat::Yuv,
        _ => PixelFormat::Bgra, // Fallback assumption
    }
}

/// Build FrameInfo from a scap Frame.
pub fn frame_info(frame: &Frame) -> FrameInfo {
    let (width, height) = get_frame_dimensions(frame);
    FrameInfo {
        width,
        height,
        format: get_frame_format(frame),
        timestamp: Instant::now(),
    }
}

/// Extract raw BGRA data from a scap Frame with size validation.
///
/// Returns None if the frame format is not BGRA or if the data is
/// undersized (which would cause SIGSEGV in downstream consumers).
///
/// # Arguments
/// * `frame` — scap Frame to extract from
/// * `expected_size` — minimum expected byte count (width * height * 4 for BGRA)
pub fn extract_frame_data(frame: &Frame, expected_size: usize) -> Option<Vec<u8>> {
    let data = match frame {
        Frame::BGRA(bgra) => bgra.data.clone(),
        _ => {
            log::warn!("Unexpected frame format, expected BGRA");
            return None;
        }
    };

    if data.len() < expected_size {
        log::warn!(
            "Frame data size mismatch: expected {}, got {} — rejecting undersized frame",
            expected_size,
            data.len()
        );
        return None;
    }

    Some(data)
}
