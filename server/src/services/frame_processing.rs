// Frame Processing Utilities
// Shared utilities for frame extraction and manipulation across capture services

use scap::frame::Frame;

/// Extract JPEG frame from MJPEG buffer
///
/// Parses multipart/x-mixed-replace boundary format and extracts a single JPEG frame.
/// Drains the buffer up to the end of the extracted frame.
///
/// # Arguments
/// * `buffer` - Accumulator buffer containing MJPEG stream data
/// * `boundary` - Boundary marker bytes (e.g., b"--ffserver")
///
/// # Returns
/// Some(Vec<u8>) containing JPEG data if a complete frame was found, None otherwise
pub fn extract_jpeg_frame(buffer: &mut Vec<u8>, boundary: &[u8]) -> Option<Vec<u8>> {
    // Find first boundary
    let first = find_subsequence(buffer, boundary)?;

    // Find content type line end (after boundary)
    let header_start = first + boundary.len();
    let header_end = find_subsequence(&buffer[header_start..], b"\r\n\r\n")?;
    let content_start = header_start + header_end + 4;

    // Find next boundary
    let next_boundary = find_subsequence(&buffer[content_start..], boundary)?;
    let content_end = content_start + next_boundary;

    // Check for JPEG markers
    if content_end - content_start < 2 {
        // Not enough data
        buffer.drain(..first + boundary.len());
        return None;
    }

    // Extract JPEG data (trim trailing \r\n before boundary)
    let mut jpeg_end = content_end;
    while jpeg_end > content_start && (buffer[jpeg_end - 1] == b'\n' || buffer[jpeg_end - 1] == b'\r') {
        jpeg_end -= 1;
    }

    let frame = buffer[content_start..jpeg_end].to_vec();
    buffer.drain(..content_end);

    Some(frame)
}

/// Find subsequence in slice
///
/// # Arguments
/// * `haystack` - Slice to search within
/// * `needle` - Pattern to search for
///
/// # Returns
/// Some(usize) with the starting index if found, None otherwise
pub fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// Get dimensions from a scap Frame
///
/// # Arguments
/// * `frame` - scap::Frame to extract dimensions from
///
/// # Returns
/// Tuple of (width, height) in pixels. Returns (1920, 1080) fallback for unsupported formats.
pub fn get_frame_dimensions(frame: &Frame) -> (u32, u32) {
    match frame {
        Frame::BGRA(bgra) => (bgra.width as u32, bgra.height as u32),
        Frame::RGB(rgb) => (rgb.width as u32, rgb.height as u32),
        Frame::YUVFrame(yuv) => (yuv.width as u32, yuv.height as u32),
        _ => (1920, 1080), // Fallback for other formats
    }
}

/// Extract raw frame data from a scap Frame
///
/// Currently only handles BGRA format. Validates data size against expected size.
///
/// # Arguments
/// * `frame` - scap::Frame to extract data from
/// * `expected_size` - Expected byte size of frame data (width * height * 4 for BGRA)
///
/// # Returns
/// Some(Vec<u8>) containing BGRA data if successful, None if format is unsupported or size is invalid
pub fn extract_frame_data(frame: &Frame, expected_size: usize) -> Option<Vec<u8>> {
    let data = match frame {
        Frame::BGRA(bgra) => bgra.data.clone(),
        _ => {
            log::warn!("Unexpected frame format, expected BGRA");
            return None;
        }
    };

    // Reject undersized frames to prevent SIGSEGV in downstream consumers
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
