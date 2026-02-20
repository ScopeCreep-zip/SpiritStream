// Stream Audio Decoder Service
// Decodes audio from network streams (RTMP, NDI) via go2rtc's HTTP API.
// go2rtc exposes `/api/stream.aac?src=name` which outputs AAC ADTS — symphonia's
// AdtsReader decodes this on a dedicated OS thread with natural network pacing.
// Same broadcast channel pattern as MediaAudioDecoder.

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

/// Why the stream decode ended
enum StreamEndReason {
    Stopped,
    EndOfStream,
}

/// Active decoder state
struct ActiveDecoder {
    stop_flag: Arc<AtomicBool>,
    #[allow(dead_code)]
    tx: broadcast::Sender<AudioBuffer>,
    thread_handle: Option<std::thread::JoinHandle<()>>,
}

/// Decodes audio from network streams accessible via go2rtc's HTTP audio API.
///
/// For each source, spawns a dedicated OS thread that:
/// 1. Registers the stream with go2rtc (idempotent PUT)
/// 2. Fetches AAC ADTS audio via HTTP GET
/// 3. Decodes with symphonia's AdtsReader
/// 4. Sends f32 AudioBuffers via broadcast channel
///
/// Reconnects with exponential backoff on disconnect.
pub struct StreamAudioDecoder {
    active_decoders: Mutex<HashMap<String, ActiveDecoder>>,
}

impl StreamAudioDecoder {
    pub fn new() -> Self {
        Self {
            active_decoders: Mutex::new(HashMap::new()),
        }
    }

    /// Start decoding audio from a network stream via go2rtc.
    ///
    /// - `source_id`: unique source identifier
    /// - `go2rtc_base_url`: e.g. "http://127.0.0.1:1984"
    /// - `stream_name`: go2rtc stream name (used for registration and fetch)
    /// - `stream_source`: go2rtc source URL (e.g. "rtmp://0.0.0.0:1935/live")
    pub fn start_decode(
        &self,
        source_id: &str,
        go2rtc_base_url: &str,
        stream_name: &str,
        stream_source: &str,
    ) -> Result<broadcast::Receiver<AudioBuffer>, String> {
        {
            let decoders = self.active_decoders.lock();
            if decoders.contains_key(source_id) {
                return Err(format!("Already decoding stream audio for source: {}", source_id));
            }
        }

        let (tx, rx) = broadcast::channel::<AudioBuffer>(16);
        let stop_flag = Arc::new(AtomicBool::new(false));

        let source_id_owned = source_id.to_string();
        let go2rtc_url = go2rtc_base_url.to_string();
        let name = stream_name.to_string();
        let source = stream_source.to_string();
        let stop_clone = stop_flag.clone();
        let tx_clone = tx.clone();

        let thread_handle = std::thread::Builder::new()
            .name(format!("ss-stream-audio-{}", &source_id[..source_id.len().min(8)]))
            .spawn(move || {
                decode_stream_loop(
                    &source_id_owned,
                    &go2rtc_url,
                    &name,
                    &source,
                    stop_clone,
                    tx_clone,
                );
            })
            .map_err(|e| format!("Failed to spawn stream decode thread: {}", e))?;

        self.active_decoders.lock().insert(
            source_id.to_string(),
            ActiveDecoder {
                stop_flag,
                tx,
                thread_handle: Some(thread_handle),
            },
        );

        log::info!(
            "[StreamAudioDecoder] Started for source '{}' (stream: '{}', source: '{}')",
            source_id, stream_name, stream_source
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
            log::info!("[StreamAudioDecoder] Stopped for source '{}'", source_id);
        }
    }

    /// Stop all active stream decoders.
    pub fn stop_all(&self) {
        let mut decoders = self.active_decoders.lock();
        for (id, mut decoder) in decoders.drain() {
            decoder.stop_flag.store(true, Ordering::Relaxed);
            if let Some(handle) = decoder.thread_handle.take() {
                let _ = handle.join();
            }
            log::info!("[StreamAudioDecoder] Stopped for source '{}'", id);
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

impl Drop for StreamAudioDecoder {
    fn drop(&mut self) {
        self.stop_all();
    }
}

impl Default for StreamAudioDecoder {
    fn default() -> Self {
        Self::new()
    }
}

/// Retry loop with exponential backoff: 1s → 2s → 4s → 8s → 10s (capped).
/// Live streams may disconnect and reconnect, so we keep retrying.
fn decode_stream_loop(
    source_id: &str,
    go2rtc_url: &str,
    stream_name: &str,
    stream_source: &str,
    stop_flag: Arc<AtomicBool>,
    tx: broadcast::Sender<AudioBuffer>,
) {
    let mut backoff = Duration::from_secs(1);
    let max_backoff = Duration::from_secs(10);

    loop {
        if stop_flag.load(Ordering::Relaxed) {
            break;
        }

        match decode_stream_once(source_id, go2rtc_url, stream_name, stream_source, &stop_flag, &tx) {
            Ok(StreamEndReason::Stopped) => break,
            Ok(StreamEndReason::EndOfStream) => {
                if stop_flag.load(Ordering::Relaxed) {
                    break;
                }
                log::info!(
                    "[StreamAudioDecoder] Stream ended for '{}', retrying in {:?}",
                    source_id, backoff
                );
            }
            Err(e) => {
                if stop_flag.load(Ordering::Relaxed) {
                    break;
                }
                log::warn!(
                    "[StreamAudioDecoder] Error for '{}': {}. Retrying in {:?}",
                    source_id, e, backoff
                );
            }
        }

        // Interruptible sleep: check stop_flag every 100ms during backoff
        let sleep_end = std::time::Instant::now() + backoff;
        while std::time::Instant::now() < sleep_end {
            if stop_flag.load(Ordering::Relaxed) {
                log::debug!("[StreamAudioDecoder] Stop requested during backoff for '{}'", source_id);
                return;
            }
            std::thread::sleep(Duration::from_millis(100));
        }

        // Exponential backoff with cap
        backoff = (backoff * 2).min(max_backoff);
    }

    log::debug!("[StreamAudioDecoder] Decode thread exiting for source '{}'", source_id);
}

/// Single decode attempt: register stream, fetch AAC, decode, send buffers.
fn decode_stream_once(
    source_id: &str,
    go2rtc_url: &str,
    stream_name: &str,
    stream_source: &str,
    stop_flag: &AtomicBool,
    tx: &broadcast::Sender<AudioBuffer>,
) -> Result<StreamEndReason, String> {
    // 1. Ensure stream is registered with go2rtc (idempotent).
    //    Audio metering may start before video preview, so we register here too.
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let register_url = format!(
        "{}/api/streams?src={}&name={}",
        go2rtc_url,
        urlencoding::encode(stream_source),
        urlencoding::encode(stream_name)
    );
    client
        .put(&register_url)
        .send()
        .map_err(|e| format!("Failed to register stream with go2rtc: {}", e))?;

    // 2. Fetch AAC ADTS audio stream from go2rtc
    //    Use a separate client without timeout for the streaming response
    let stream_client = reqwest::blocking::Client::builder()
        .build()
        .map_err(|e| format!("Failed to create stream client: {}", e))?;

    let audio_url = format!(
        "{}/api/stream.aac?src={}",
        go2rtc_url,
        urlencoding::encode(stream_name)
    );
    let response = stream_client
        .get(&audio_url)
        .send()
        .map_err(|e| format!("Failed to connect to go2rtc audio stream: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("go2rtc returned HTTP {}", response.status()));
    }

    // 3. Wrap HTTP response body in symphonia's non-seekable source adapter
    let source = ReadOnlySource::new(response);
    let mss = MediaSourceStream::new(Box::new(source), Default::default());

    let mut hint = Hint::new();
    hint.with_extension("aac");

    // 4. Probe format — will find AdtsReader for AAC ADTS
    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .map_err(|e| format!("Failed to probe audio stream: {}", e))?;

    let mut format_reader = probed.format;

    // 5. Find audio track and create decoder
    let track = format_reader
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or("No audio track found in stream")?;

    let track_id = track.id;
    let codec_params = track.codec_params.clone();
    let sample_rate = codec_params.sample_rate.unwrap_or(48000);
    let channels = codec_params.channels.map(|c| c.count() as u16).unwrap_or(2);

    let mut decoder = symphonia::default::get_codecs()
        .make(&codec_params, &DecoderOptions::default())
        .map_err(|e| format!("Failed to create audio decoder: {}", e))?;

    log::info!(
        "[StreamAudioDecoder] Source '{}': decoding {}Hz {}ch AAC from '{}'",
        source_id, sample_rate, channels, stream_name
    );

    let start_time = std::time::Instant::now();

    // 6. Decode loop — NO sleep, network provides natural real-time pacing
    loop {
        if stop_flag.load(Ordering::Relaxed) {
            return Ok(StreamEndReason::Stopped);
        }

        let packet = match format_reader.next_packet() {
            Ok(p) => p,
            Err(symphonia::core::errors::Error::ResetRequired) => {
                decoder.reset();
                continue;
            }
            Err(symphonia::core::errors::Error::IoError(ref e))
                if e.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                return Ok(StreamEndReason::EndOfStream);
            }
            Err(e) => return Err(format!("Packet read error: {}", e)),
        };

        if packet.track_id() != track_id {
            continue;
        }

        let decoded = match decoder.decode(&packet) {
            Ok(d) => d,
            Err(symphonia::core::errors::Error::DecodeError(e)) => {
                log::debug!("[StreamAudioDecoder] Decode error (skipping): {}", e);
                continue;
            }
            Err(e) => return Err(format!("Decode error: {}", e)),
        };

        let spec = *decoded.spec();
        let num_frames = decoded.frames();
        if num_frames == 0 {
            continue;
        }

        let mut sample_buf = SampleBuffer::<f32>::new(num_frames as u64, spec);
        sample_buf.copy_interleaved_ref(decoded);

        let buffer = AudioBuffer {
            samples: sample_buf.samples().to_vec(),
            sample_rate: spec.rate,
            channels: spec.channels.count() as u16,
            timestamp_ms: start_time.elapsed().as_millis() as u64,
        };

        let _ = tx.send(buffer);
    }
}

/// Adapter that wraps a `reqwest::blocking::Response` as a symphonia `MediaSource`.
/// Non-seekable — symphonia's AdtsReader handles streaming AAC ADTS fine without seeking.
struct ReadOnlySource {
    response: reqwest::blocking::Response,
}

impl ReadOnlySource {
    fn new(response: reqwest::blocking::Response) -> Self {
        Self { response }
    }
}

impl std::io::Read for ReadOnlySource {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.response.read(buf)
    }
}

impl std::io::Seek for ReadOnlySource {
    fn seek(&mut self, _pos: std::io::SeekFrom) -> std::io::Result<u64> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "stream is not seekable",
        ))
    }
}

impl symphonia::core::io::MediaSource for ReadOnlySource {
    fn is_seekable(&self) -> bool {
        false
    }

    fn byte_len(&self) -> Option<u64> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_and_default() {
        let decoder = StreamAudioDecoder::new();
        assert!(decoder.active_ids().is_empty());
        assert!(!decoder.is_decoding("test"));

        let decoder2 = StreamAudioDecoder::default();
        assert!(decoder2.active_ids().is_empty());
    }

    #[test]
    fn test_stop_nonexistent_is_noop() {
        let decoder = StreamAudioDecoder::new();
        decoder.stop("nonexistent"); // should not panic
    }

    #[test]
    fn test_stop_all_empty() {
        let decoder = StreamAudioDecoder::new();
        decoder.stop_all(); // should not panic
    }
}
