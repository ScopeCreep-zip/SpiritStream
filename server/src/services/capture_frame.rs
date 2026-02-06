// Unified Capture Frame
// Common frame type for all capture sources (screen, camera, capture card)
// Eliminates incompatible frame types between capture pipelines

use scap::frame::Frame;
use super::camera_capture::VideoFrame;

/// Pixel format of captured frame data
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelFormat {
    /// Blue-Green-Red-Alpha, 4 bytes per pixel (macOS screen capture default)
    BGRA,
    /// Red-Green-Blue, 3 bytes per pixel (camera capture default)
    RGB24,
    /// YUV 4:2:0 semi-planar (hardware capture, some cameras)
    NV12,
}

impl PixelFormat {
    /// Bytes per pixel for this format (NV12 returns 0 since it's planar)
    pub fn bytes_per_pixel(&self) -> usize {
        match self {
            PixelFormat::BGRA => 4,
            PixelFormat::RGB24 => 3,
            PixelFormat::NV12 => 0, // Planar format: width*height * 3/2 total
        }
    }

    /// FFmpeg pixel format string for rawvideo input
    pub fn ffmpeg_pix_fmt(&self) -> &'static str {
        match self {
            PixelFormat::BGRA => "bgra",
            PixelFormat::RGB24 => "rgb24",
            PixelFormat::NV12 => "nv12",
        }
    }

    /// Expected data size for given dimensions
    pub fn expected_size(&self, width: u32, height: u32) -> usize {
        let pixels = (width as usize) * (height as usize);
        match self {
            PixelFormat::BGRA => pixels * 4,
            PixelFormat::RGB24 => pixels * 3,
            PixelFormat::NV12 => pixels * 3 / 2,
        }
    }
}

/// Unified frame type for all capture sources
#[derive(Debug, Clone)]
pub struct CaptureFrame {
    /// Raw pixel data
    pub data: Vec<u8>,
    /// Frame width in pixels
    pub width: u32,
    /// Frame height in pixels
    pub height: u32,
    /// Pixel format of the data
    pub pixel_format: PixelFormat,
    /// Timestamp in milliseconds since capture start
    pub timestamp_ms: u64,
}

impl CaptureFrame {
    /// Validate that this frame has consistent, non-degenerate data.
    /// Returns Err with a description if the frame is invalid.
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.width == 0 || self.height == 0 {
            return Err("zero dimensions");
        }
        if self.data.is_empty() {
            return Err("empty data");
        }
        let expected = self.pixel_format.expected_size(self.width, self.height);
        if expected > 0 && self.data.len() < expected {
            return Err("data undersized for dimensions");
        }
        Ok(())
    }
}

impl CaptureFrame {
    /// Convert from a scap Frame. Returns None for unsupported/invalid frames.
    pub fn from_scap_frame(frame: &Frame) -> Option<CaptureFrame> {
        match frame {
            Frame::BGRA(bgra) => {
                let width = bgra.width as u32;
                let height = bgra.height as u32;
                if width == 0 || height == 0 || bgra.data.is_empty() {
                    return None;
                }
                Some(CaptureFrame {
                    data: bgra.data.clone(),
                    width,
                    height,
                    pixel_format: PixelFormat::BGRA,
                    timestamp_ms: 0,
                })
            }
            Frame::RGB(rgb) => {
                let width = rgb.width as u32;
                let height = rgb.height as u32;
                if width == 0 || height == 0 || rgb.data.is_empty() {
                    return None;
                }
                Some(CaptureFrame {
                    data: rgb.data.clone(),
                    width,
                    height,
                    pixel_format: PixelFormat::RGB24,
                    timestamp_ms: 0,
                })
            }
            Frame::YUVFrame(yuv) => {
                let width = yuv.width as u32;
                let height = yuv.height as u32;
                if width == 0 || height == 0 {
                    return None;
                }
                let mut data = Vec::with_capacity(yuv.luminance_bytes.len() + yuv.chrominance_bytes.len());
                data.extend_from_slice(&yuv.luminance_bytes);
                data.extend_from_slice(&yuv.chrominance_bytes);
                if data.is_empty() {
                    return None;
                }
                Some(CaptureFrame {
                    data,
                    width,
                    height,
                    pixel_format: PixelFormat::NV12,
                    timestamp_ms: 0,
                })
            }
            _ => None,
        }
    }
}

/// Convert from camera VideoFrame
impl From<&VideoFrame> for CaptureFrame {
    fn from(frame: &VideoFrame) -> Self {
        let pixel_format = match frame.pixel_format.as_str() {
            "bgra" => PixelFormat::BGRA,
            "nv12" => PixelFormat::NV12,
            _ => PixelFormat::RGB24, // Default for "rgb24" and others
        };
        CaptureFrame {
            data: frame.data.clone(),
            width: frame.width,
            height: frame.height,
            pixel_format,
            timestamp_ms: frame.timestamp_ms,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pixel_format_sizes() {
        assert_eq!(PixelFormat::BGRA.expected_size(1920, 1080), 1920 * 1080 * 4);
        assert_eq!(PixelFormat::RGB24.expected_size(1280, 720), 1280 * 720 * 3);
        assert_eq!(PixelFormat::NV12.expected_size(1920, 1080), 1920 * 1080 * 3 / 2);
    }

    #[test]
    fn test_pixel_format_ffmpeg() {
        assert_eq!(PixelFormat::BGRA.ffmpeg_pix_fmt(), "bgra");
        assert_eq!(PixelFormat::RGB24.ffmpeg_pix_fmt(), "rgb24");
        assert_eq!(PixelFormat::NV12.ffmpeg_pix_fmt(), "nv12");
    }

    #[test]
    fn test_validate_valid_frame() {
        let frame = CaptureFrame {
            data: vec![0u8; 1920 * 1080 * 4],
            width: 1920,
            height: 1080,
            pixel_format: PixelFormat::BGRA,
            timestamp_ms: 0,
        };
        assert!(frame.validate().is_ok());
    }

    #[test]
    fn test_validate_zero_dimensions() {
        let frame = CaptureFrame {
            data: vec![0u8; 100],
            width: 0,
            height: 1080,
            pixel_format: PixelFormat::BGRA,
            timestamp_ms: 0,
        };
        assert_eq!(frame.validate(), Err("zero dimensions"));
    }

    #[test]
    fn test_validate_empty_data() {
        let frame = CaptureFrame {
            data: vec![],
            width: 1920,
            height: 1080,
            pixel_format: PixelFormat::BGRA,
            timestamp_ms: 0,
        };
        assert_eq!(frame.validate(), Err("empty data"));
    }

    #[test]
    fn test_validate_undersized_data() {
        let frame = CaptureFrame {
            data: vec![0u8; 100],
            width: 1920,
            height: 1080,
            pixel_format: PixelFormat::BGRA,
            timestamp_ms: 0,
        };
        assert_eq!(frame.validate(), Err("data undersized for dimensions"));
    }

    #[test]
    fn test_from_video_frame() {
        let vf = VideoFrame {
            data: vec![0u8; 1280 * 720 * 3],
            width: 1280,
            height: 720,
            pixel_format: "rgb24".to_string(),
            timestamp_ms: 42,
        };
        let cf = CaptureFrame::from(&vf);
        assert_eq!(cf.width, 1280);
        assert_eq!(cf.height, 720);
        assert_eq!(cf.pixel_format, PixelFormat::RGB24);
        assert_eq!(cf.timestamp_ms, 42);
        assert!(cf.validate().is_ok());
    }
}
