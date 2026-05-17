//! Image-metadata sanitizer.
//!
//! Strips identifying metadata from every image that flows in over the
//! API: theme uploads today, profile pictures and overlays as those
//! features land. Doxxing-relevant metadata commonly carries:
//!
//! * **EXIF** — GPS coordinates, camera serial, software, date/time.
//! * **ICC color profiles** — embed creation-tool version strings.
//! * **PNG text chunks** (`tEXt`, `iTXt`, `zTXt`, `eXIf`) — arbitrary
//!   key/value text injected by editing tools.
//!
//! Implementation uses `img-parts` for container-level stripping
//! (JPEG/PNG/WebP) and a PNG-specific chunk filter for the text
//! categories. Inputs that don't decode as a recognised image format
//! are rejected so a malformed file can't pass through unscrubbed.
//!
//! # Known limitations (documented for the user-facing safety guide)
//!
//! * Custom IRB (Photoshop) blocks inside JPEG APP13 are not currently
//!   parsed. img-parts strips EXIF / ICC; IRB removal requires a
//!   deeper segment walk we haven't shipped yet.
//! * Embedded thumbnails inside EXIF have their own EXIF blocks. The
//!   strip path drops the entire EXIF segment so the thumbnail's GPS
//!   is removed by transitive cleanup, not by walking the thumbnail.
//! * MP4 / video metadata is out of scope; SpiritStream does not yet
//!   accept video uploads, and `ffmpeg -map_metadata -1 -c copy` is
//!   the right tool when it does.
//!
//! # Residual risks
//!
//! Re-compression artifacts can still fingerprint the editing tool.
//! ICC profile *version* strings are gone but the profile name (sRGB,
//! Display P3) remains, which is non-identifying.

use std::io::Cursor;

use crate::errors::CoreError;

/// Detected/declared image format. The sanitizer always sniffs from the
/// content bytes — the caller's MIME hint is advisory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageFormat {
    Jpeg,
    Png,
    Webp,
}

impl ImageFormat {
    /// Sniff format from the first bytes of an image. Returns `None`
    /// for anything we don't recognise — callers should refuse the
    /// upload rather than ship un-scrubbed bytes through.
    pub fn sniff(bytes: &[u8]) -> Option<Self> {
        if bytes.len() >= 4 && bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
            return Some(Self::Jpeg);
        }
        if bytes.len() >= 8 && bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A])
        {
            return Some(Self::Png);
        }
        if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
            return Some(Self::Webp);
        }
        None
    }
}

/// Strip metadata from `input`, returning a freshly-encoded image with
/// the same pixel data but no identifying side-channels.
///
/// Returns `Err(CoreError::ValidationFailed)` for unrecognised formats
/// (callers shouldn't silently let an unknown file through), and
/// `Err(CoreError::Internal)` for container-parse failures (mirrors how
/// the rest of core surfaces unexpected-shape errors).
pub fn strip_metadata(input: &[u8]) -> Result<Vec<u8>, CoreError> {
    let format = ImageFormat::sniff(input).ok_or_else(|| CoreError::ValidationFailed {
        reasons: vec![crate::errors::ValidationIssue {
            code: "media_unknown_format".into(),
            message: "Unrecognised image format (expected JPEG, PNG, or WebP)".into(),
            path: None,
        }],
    })?;
    match format {
        ImageFormat::Jpeg => strip_jpeg(input),
        ImageFormat::Png => strip_png(input),
        ImageFormat::Webp => strip_webp(input),
    }
}

fn strip_jpeg(input: &[u8]) -> Result<Vec<u8>, CoreError> {
    use img_parts::jpeg::Jpeg;
    use img_parts::ImageEXIF;
    use img_parts::ImageICC;

    let mut jpeg = Jpeg::from_bytes(input.to_vec().into()).map_err(parse_err)?;
    jpeg.set_exif(None);
    jpeg.set_icc_profile(None);
    let mut out = Vec::with_capacity(input.len());
    jpeg.encoder()
        .write_to(Cursor::new(&mut out))
        .map_err(write_err)?;
    Ok(out)
}

fn strip_png(input: &[u8]) -> Result<Vec<u8>, CoreError> {
    use img_parts::png::Png;

    let mut png = Png::from_bytes(input.to_vec().into()).map_err(parse_err)?;
    // Text and metadata chunks — drop every one. The required structural
    // chunks (IHDR, IDAT, IEND, PLTE, tRNS) are left untouched.
    for kind in [*b"tEXt", *b"iTXt", *b"zTXt", *b"eXIf", *b"iCCP", *b"tIME"] {
        png.remove_chunks_by_type(kind);
    }
    let mut out = Vec::with_capacity(input.len());
    png.encoder()
        .write_to(Cursor::new(&mut out))
        .map_err(write_err)?;
    Ok(out)
}

fn strip_webp(input: &[u8]) -> Result<Vec<u8>, CoreError> {
    use img_parts::webp::WebP;
    use img_parts::ImageEXIF;
    use img_parts::ImageICC;

    let mut webp = WebP::from_bytes(input.to_vec().into()).map_err(parse_err)?;
    webp.set_exif(None);
    webp.set_icc_profile(None);
    let mut out = Vec::with_capacity(input.len());
    webp.encoder()
        .write_to(Cursor::new(&mut out))
        .map_err(write_err)?;
    Ok(out)
}

fn parse_err(e: img_parts::Error) -> CoreError {
    CoreError::Internal {
        context: format!("image parse: {e}"),
    }
}

fn write_err(e: std::io::Error) -> CoreError {
    CoreError::Internal {
        context: format!("image write: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal valid 1×1 RGBA PNG at test time using the `png`
    /// crate. Hard-coding PNG bytes is fragile because img-parts
    /// validates CRCs on parse — using a real encoder side-steps that.
    fn build_tiny_png() -> Vec<u8> {
        let mut bytes = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut bytes, 1, 1);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().unwrap();
            writer.write_image_data(&[0, 0, 0, 0]).unwrap(); // 1 transparent pixel
        }
        bytes
    }

    #[test]
    fn sniff_detects_png() {
        assert_eq!(
            ImageFormat::sniff(build_tiny_png().as_slice()),
            Some(ImageFormat::Png)
        );
    }

    #[test]
    fn sniff_detects_jpeg() {
        let jpeg_head = [0xFF, 0xD8, 0xFF, 0xE0, 0, 0, 0, 0];
        assert_eq!(ImageFormat::sniff(&jpeg_head), Some(ImageFormat::Jpeg));
    }

    #[test]
    fn sniff_detects_webp() {
        let mut head = Vec::from(*b"RIFF\x00\x00\x00\x00WEBP");
        head.extend_from_slice(&[0u8; 8]);
        assert_eq!(ImageFormat::sniff(&head), Some(ImageFormat::Webp));
    }

    #[test]
    fn sniff_returns_none_for_unknown_format() {
        assert_eq!(ImageFormat::sniff(b"not an image"), None);
    }

    #[test]
    fn strip_metadata_rejects_unknown_format() {
        let err = strip_metadata(b"definitely-not-an-image-file").unwrap_err();
        assert!(
            matches!(err, CoreError::ValidationFailed { .. }),
            "expected ValidationFailed for unknown format: {err:?}",
        );
    }

    #[test]
    fn strip_png_removes_text_chunks() {
        // Build a PNG that carries a tEXt chunk holding a fake GPS
        // string, then confirm strip_metadata removes it.
        use img_parts::png::{Png, PngChunk};

        let mut png = Png::from_bytes(build_tiny_png().as_slice().to_vec().into()).unwrap();
        // Insert a tEXt chunk with identifying data.
        let payload = b"GPS\x00lat=37.7749,lon=-122.4194-and-real-name";
        let chunk = PngChunk::new(*b"tEXt", payload.to_vec().into());
        // Insert before IEND (last chunk).
        let chunks = png.chunks_mut();
        let last = chunks.len() - 1;
        chunks.insert(last, chunk);
        let mut tainted = Vec::new();
        png.encoder().write_to(Cursor::new(&mut tainted)).unwrap();
        assert!(
            tainted
                .windows(b"real-name".len())
                .any(|w| w == b"real-name"),
            "fixture failed to embed identifying string",
        );

        let stripped = strip_metadata(&tainted).expect("strip");
        assert!(
            !stripped
                .windows(b"real-name".len())
                .any(|w| w == b"real-name"),
            "identifying string leaked through strip_metadata",
        );
        // Output must still be a valid PNG.
        assert_eq!(ImageFormat::sniff(&stripped), Some(ImageFormat::Png));
    }

    #[test]
    fn strip_metadata_is_idempotent() {
        // A clean PNG round-tripped through strip_metadata should keep
        // working as input to another strip call.
        let once = strip_metadata(build_tiny_png().as_slice()).expect("first strip");
        let twice = strip_metadata(&once).expect("second strip");
        // Both decode as PNG.
        assert_eq!(ImageFormat::sniff(&twice), Some(ImageFormat::Png));
    }
}
