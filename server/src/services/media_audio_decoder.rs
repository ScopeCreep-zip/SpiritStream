// Media Audio Decoder Service
// Decodes audio from local media files in-process using symphonia (pure Rust).
// Replaces FFmpeg astats subprocesses for MediaFile and MediaPlaylist sources.
// Each decode runs on a dedicated thread to avoid blocking the async runtime.

use crate::services::audio_capture::AudioBuffer;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast;
use parking_lot::Mutex;

use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

/// Media file extensions that can have audio (same list as AudioLevelExtractor)
const MEDIA_EXTENSIONS: &[&str] = &[
    // Video
    "mp4", "mkv", "avi", "mov", "wmv", "flv", "webm", "m4v", "mpeg", "mpg", "ts", "m2ts",
    // Audio
    "mp3", "wav", "aac", "flac", "ogg", "m4a", "wma", "opus", "aiff",
];

/// Active decoder state
struct ActiveDecoder {
    stop_flag: Arc<AtomicBool>,
    /// Kept alive to prevent broadcast channel from closing while thread runs.
    #[allow(dead_code)]
    tx: broadcast::Sender<AudioBuffer>,
    thread_handle: Option<std::thread::JoinHandle<()>>,
}

/// Service for decoding audio from local media files using symphonia.
///
/// Each source gets a dedicated decode thread that reads the file,
/// decodes audio packets, converts to f32 interleaved `AudioBuffer`,
/// and sends via `broadcast::Sender` at approximately real-time speed.
pub struct MediaAudioDecoder {
    active_decoders: Mutex<HashMap<String, ActiveDecoder>>,
}

impl MediaAudioDecoder {
    pub fn new() -> Self {
        Self {
            active_decoders: Mutex::new(HashMap::new()),
        }
    }

    /// Start decoding audio from a media file.
    ///
    /// Returns a `broadcast::Receiver<AudioBuffer>` that receives decoded
    /// PCM samples at approximately real-time speed.
    ///
    /// If `looping` is true, the file restarts from the beginning on EOF.
    pub fn start_decode(
        &self,
        source_id: &str,
        file_path: &str,
        looping: bool,
    ) -> Result<broadcast::Receiver<AudioBuffer>, String> {
        // Validate file extension
        let path = std::path::Path::new(file_path);
        let extension = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .unwrap_or_default();

        if !MEDIA_EXTENSIONS.contains(&extension.as_str()) {
            return Err(format!(
                "File '{}' is not a supported media format (found: .{}). Audio metering only works with video/audio files.",
                path.file_name().unwrap_or_default().to_string_lossy(),
                if extension.is_empty() { "none" } else { &extension }
            ));
        }

        // Check if already decoding
        {
            let decoders = self.active_decoders.lock();
            if decoders.contains_key(source_id) {
                return Err(format!("Already decoding audio for source: {}", source_id));
            }
        }

        // Verify file exists
        if !path.exists() {
            return Err(format!("Media file not found: {}", file_path));
        }

        let (tx, rx) = broadcast::channel::<AudioBuffer>(16);
        let stop_flag = Arc::new(AtomicBool::new(false));

        let source_id_owned = source_id.to_string();
        let file_path_owned = file_path.to_string();
        let stop_flag_clone = stop_flag.clone();
        let tx_clone = tx.clone();

        let thread_handle = std::thread::Builder::new()
            .name(format!("ss-media-audio-{}", &source_id[..source_id.len().min(8)]))
            .spawn(move || {
                decode_media_file_loop(
                    &source_id_owned,
                    &file_path_owned,
                    looping,
                    stop_flag_clone,
                    tx_clone,
                );
            })
            .map_err(|e| format!("Failed to spawn media decode thread: {}", e))?;

        self.active_decoders.lock().insert(
            source_id.to_string(),
            ActiveDecoder {
                stop_flag,
                tx,
                thread_handle: Some(thread_handle),
            },
        );

        log::info!(
            "[MediaAudioDecoder] Started decoding '{}' for source '{}' (loop={})",
            file_path, source_id, looping
        );

        Ok(rx)
    }

    /// Stop decoding for a specific source.
    pub fn stop(&self, source_id: &str) {
        if let Some(mut decoder) = self.active_decoders.lock().remove(source_id) {
            decoder.stop_flag.store(true, Ordering::Relaxed);
            if let Some(handle) = decoder.thread_handle.take() {
                let _ = handle.join();
            }
            log::info!("[MediaAudioDecoder] Stopped decoding for source '{}'", source_id);
        }
    }

    /// Stop all active decoders.
    pub fn stop_all(&self) {
        let mut decoders = self.active_decoders.lock();
        for (id, mut decoder) in decoders.drain() {
            decoder.stop_flag.store(true, Ordering::Relaxed);
            if let Some(handle) = decoder.thread_handle.take() {
                let _ = handle.join();
            }
            log::info!("[MediaAudioDecoder] Stopped decoding for source '{}'", id);
        }
    }

    /// Check if a source is currently being decoded.
    pub fn is_decoding(&self, source_id: &str) -> bool {
        self.active_decoders.lock().contains_key(source_id)
    }

    /// Get list of active decoder source IDs.
    pub fn active_ids(&self) -> Vec<String> {
        self.active_decoders.lock().keys().cloned().collect()
    }
}

impl Drop for MediaAudioDecoder {
    fn drop(&mut self) {
        self.stop_all();
    }
}

impl Default for MediaAudioDecoder {
    fn default() -> Self {
        Self::new()
    }
}

/// Decode loop running on a dedicated thread.
/// Opens the file with symphonia, finds the audio track, decodes packets,
/// and sends `AudioBuffer` at real-time speed.
fn decode_media_file_loop(
    source_id: &str,
    file_path: &str,
    looping: bool,
    stop_flag: Arc<AtomicBool>,
    tx: broadcast::Sender<AudioBuffer>,
) {
    loop {
        if stop_flag.load(Ordering::Relaxed) {
            break;
        }

        match decode_file_once(source_id, file_path, &stop_flag, &tx) {
            Ok(()) => {
                if looping && !stop_flag.load(Ordering::Relaxed) {
                    log::debug!("[MediaAudioDecoder] Source '{}' EOF, looping", source_id);
                    continue;
                }
                break;
            }
            Err(e) => {
                log::warn!("[MediaAudioDecoder] Source '{}' decode error: {}", source_id, e);
                // Wait before retry to avoid busy-loop on persistent errors
                std::thread::sleep(Duration::from_secs(2));
                if !looping || stop_flag.load(Ordering::Relaxed) {
                    break;
                }
            }
        }
    }
    log::debug!("[MediaAudioDecoder] Decode thread exiting for source '{}'", source_id);
}

/// Decode a single pass through the file. Returns Ok(()) on EOF, Err on error.
fn decode_file_once(
    source_id: &str,
    file_path: &str,
    stop_flag: &AtomicBool,
    tx: &broadcast::Sender<AudioBuffer>,
) -> Result<(), String> {
    // Open file
    let file = std::fs::File::open(file_path)
        .map_err(|e| format!("Failed to open '{}': {}", file_path, e))?;

    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    // Probe the format
    let mut hint = Hint::new();
    if let Some(ext) = std::path::Path::new(file_path).extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }

    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .map_err(|e| format!("Failed to probe '{}': {}", file_path, e))?;

    let mut format_reader = probed.format;

    // Find first audio track
    let track = format_reader
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or_else(|| format!("No audio track found in '{}'", file_path))?;

    let track_id = track.id;
    let codec_params = track.codec_params.clone();

    let sample_rate = codec_params.sample_rate.unwrap_or(48000);
    let channels = codec_params.channels.map(|c| c.count() as u16).unwrap_or(2);

    // Create decoder
    let mut decoder = symphonia::default::get_codecs()
        .make(&codec_params, &DecoderOptions::default())
        .map_err(|e| format!("Failed to create decoder for '{}': {}", file_path, e))?;

    log::debug!(
        "[MediaAudioDecoder] Source '{}': {}Hz, {} channels",
        source_id, sample_rate, channels
    );

    let start_time = std::time::Instant::now();

    // Decode loop
    loop {
        if stop_flag.load(Ordering::Relaxed) {
            return Ok(());
        }

        let packet = match format_reader.next_packet() {
            Ok(packet) => packet,
            Err(symphonia::core::errors::Error::ResetRequired) => {
                decoder.reset();
                continue;
            }
            Err(symphonia::core::errors::Error::IoError(ref e))
                if e.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                // Normal EOF
                return Ok(());
            }
            Err(e) => {
                return Err(format!("Packet read error: {}", e));
            }
        };

        // Skip packets from other tracks
        if packet.track_id() != track_id {
            continue;
        }

        // Decode the packet
        let decoded = match decoder.decode(&packet) {
            Ok(decoded) => decoded,
            Err(symphonia::core::errors::Error::DecodeError(e)) => {
                log::debug!("[MediaAudioDecoder] Decode error (skipping packet): {}", e);
                continue;
            }
            Err(e) => {
                return Err(format!("Decode error: {}", e));
            }
        };

        // Convert to interleaved f32
        let spec = *decoded.spec();
        let num_frames = decoded.frames();
        let actual_channels = spec.channels.count() as u16;
        let actual_rate = spec.rate;

        if num_frames == 0 {
            continue;
        }

        let mut sample_buf = SampleBuffer::<f32>::new(num_frames as u64, spec);
        sample_buf.copy_interleaved_ref(decoded);

        let samples = sample_buf.samples().to_vec();

        let timestamp_ms = start_time.elapsed().as_millis() as u64;

        let buffer = AudioBuffer {
            samples,
            sample_rate: actual_rate,
            channels: actual_channels,
            timestamp_ms,
        };

        // Send (ignore error if no receivers)
        let _ = tx.send(buffer);

        // Sleep to approximate real-time playback rate
        // This prevents the decode from racing ahead and filling the broadcast buffer
        let frame_duration = Duration::from_micros(
            (num_frames as u64 * 1_000_000) / actual_rate as u64,
        );
        // Sleep for ~80% of the frame duration to account for decode overhead
        // and maintain slight buffer lead
        if frame_duration > Duration::from_millis(1) {
            std::thread::sleep(frame_duration * 4 / 5);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_media_extensions_validation() {
        let decoder = MediaAudioDecoder::new();
        // Should fail for non-media files
        assert!(decoder.start_decode("test", "/tmp/file.html", false).is_err());
        assert!(decoder.start_decode("test", "/tmp/file.txt", false).is_err());
        // Should fail for non-existent files (but passes extension check)
        let result = decoder.start_decode("test", "/tmp/nonexistent.mp4", false);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }
}
