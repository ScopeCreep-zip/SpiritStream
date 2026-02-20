// FFmpeg Stats Collection
// Background thread for parsing FFmpeg progress output and emitting events

use std::collections::VecDeque;
use std::io::{BufRead, BufReader};
use std::net::UdpSocket;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::thread;
use std::time::{Duration, Instant};
use dashmap::{DashMap, DashSet};
use crate::models::StreamStats;
use crate::services::{emit_event, EventSink};
use crate::services::process_util::kill_and_wait;

/// Process info for tracking active streams
pub(super) struct ProcessInfo {
    pub(super) child: std::process::Child,
    pub(super) start_time: Instant,
    pub(super) group_id: String,
}

/// Bitrate meter configuration
pub(super) struct MeterConfig;

impl MeterConfig {
    pub(super) const METER_HOST: &'static str = "127.0.0.1";
    pub(super) const METER_PORT_BASE: u16 = 40000;
    pub(super) const METER_PORT_RANGE: u16 = 10000;
    pub(super) const METER_UDP_QUERY: &'static str = "pkt_size=1316";
}

/// Start bitrate meter for a group
pub(super) fn start_bitrate_meter(
    group_id: &str,
    processes: Arc<DashMap<String, ProcessInfo>>,
) -> Option<Arc<AtomicU64>> {
    let port = meter_port_for_group(group_id);
    let bind_addr = format!("{}:{}", MeterConfig::METER_HOST, port);
    let socket = match UdpSocket::bind(&bind_addr) {
        Ok(socket) => socket,
        Err(err) => {
            log::warn!(
                "Failed to bind bitrate meter for group {group_id} on {bind_addr}: {err}"
            );
            return None;
        }
    };

    if let Err(err) = socket.set_read_timeout(Some(Duration::from_millis(250))) {
        log::warn!(
            "Failed to set meter read timeout for group {group_id} on {bind_addr}: {err}"
        );
    }

    let bytes = Arc::new(AtomicU64::new(0));
    let bytes_clone = Arc::clone(&bytes);
    let group_id = group_id.to_string();

    thread::spawn(move || {
        let mut buffer = [0u8; 2048];
        loop {
            match socket.recv_from(&mut buffer) {
                Ok((len, _)) => {
                    bytes_clone.fetch_add(len as u64, Ordering::Relaxed);
                }
                Err(err) if err.kind() == std::io::ErrorKind::TimedOut => {}
                Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(_) => break,
            }

            if !processes.contains_key(&group_id) {
                break;
            }
        }
    });

    Some(bytes)
}

pub(super) fn meter_port_for_group(group_id: &str) -> u16 {
    const FNV_OFFSET: u32 = 2166136261;
    const FNV_PRIME: u32 = 16777619;

    let mut hash = FNV_OFFSET;
    for &b in group_id.as_bytes() {
        hash ^= b as u32;
        hash = hash.wrapping_mul(FNV_PRIME);
    }

    let range = MeterConfig::METER_PORT_RANGE as u32;
    let port = MeterConfig::METER_PORT_BASE as u32 + (hash % range);
    port as u16
}

pub(super) fn meter_output_url_for_group(group_id: &str) -> String {
    format!(
        "udp://{}:{}?{}",
        MeterConfig::METER_HOST,
        meter_port_for_group(group_id),
        MeterConfig::METER_UDP_QUERY
    )
}

/// Background thread that reads FFmpeg stderr and emits stats events
#[allow(clippy::too_many_arguments)]
pub(super) fn stats_reader(
    stderr: std::process::ChildStderr,
    group_id: String,
    meter_bytes: Option<Arc<AtomicU64>>,
    event_sink: Arc<dyn EventSink>,
    processes: Arc<DashMap<String, ProcessInfo>>,
    stopping_groups: Arc<DashSet<String>>,
    relay: Arc<Mutex<Option<super::ffmpeg_relay::RelayProcess>>>,
    relay_refcount: Arc<AtomicUsize>,
) {
    let reader = BufReader::new(stderr);
    let mut stats = StreamStats::new(group_id.clone());
    let mut last_emit = Instant::now();
    let emit_interval = Duration::from_millis(1000); // Emit every second
    let mut was_intentionally_stopped = false;
    let mut recent_lines: VecDeque<String> = VecDeque::with_capacity(40);
    let mut last_meter_bytes = meter_bytes
        .as_ref()
        .map(|bytes| bytes.load(Ordering::Relaxed))
        .unwrap_or(0);
    let mut last_meter_instant = Instant::now();
    let mut has_meter_sample = false;
    let mut smoothed_bitrate = 0.0;
    let mut has_smoothed_bitrate = false;

    for line in reader.lines().map_while(Result::ok) {
        // Check if process is still running (was it intentionally stopped?)
        {
            if stopping_groups.contains(&group_id) {
                was_intentionally_stopped = true;
                break;
            }
            if !processes.contains_key(&group_id) {
                // Process was removed by stop() - intentional stop
                was_intentionally_stopped = true;
                break;
            }
        }

        let sanitized_line = super::FFmpegHandler::sanitize_arg_static(&line);
        if recent_lines.len() == 40 {
            recent_lines.pop_front();
        }
        recent_lines.push_back(sanitized_line.clone());

        let parsed = stats.parse_line(&line);
        let is_progress_line = line.trim_start().starts_with("progress=");

        // Emit stats at most every second or at progress boundaries
        if is_progress_line || (parsed && last_emit.elapsed() >= emit_interval) {
            // Add uptime from process start if FFmpeg doesn't report time
            if let Some(info) = processes.get(&group_id) {
                let uptime = info.value().start_time.elapsed().as_secs_f64();
                if stats.time <= 0.0 {
                    stats.time = uptime;
                }
            }

            if stats.bitrate == 0.0 && stats.size > 0 && stats.time > 0.0 {
                let avg_kbps = (stats.size as f64 * 8.0) / 1000.0 / stats.time;
                if avg_kbps.is_finite() && avg_kbps > 0.0 {
                    stats.bitrate = avg_kbps;
                }
            }

            if let Some(bytes) = meter_bytes.as_ref() {
                let now = Instant::now();
                let current_bytes = bytes.load(Ordering::Relaxed);
                if has_meter_sample {
                    let elapsed = now.duration_since(last_meter_instant).as_secs_f64();
                    let delta_bytes = current_bytes.saturating_sub(last_meter_bytes);
                    if elapsed > 0.0 {
                        let kbps = (delta_bytes as f64 * 8.0) / 1000.0 / elapsed;
                        if kbps.is_finite() {
                            let alpha = 0.2;
                            if has_smoothed_bitrate {
                                smoothed_bitrate = smoothed_bitrate * (1.0 - alpha) + kbps * alpha;
                            } else {
                                smoothed_bitrate = kbps;
                                has_smoothed_bitrate = true;
                            }
                            stats.bitrate = smoothed_bitrate;
                        } else {
                            stats.bitrate = 0.0;
                        }
                    }
                } else {
                    has_meter_sample = true;
                }
                last_meter_bytes = current_bytes;
                last_meter_instant = now;
            }

            // Emit event
            emit_event(event_sink.as_ref(), "stream_stats", &stats);
            last_emit = Instant::now();
        }

        // Only log errors and warnings (not frame stats which are too verbose)
        if line.contains("[error]")
            || line.contains("[warning]")
            || line.contains("Error")
            || line.contains("error")
        {
            log::warn!("[FFmpeg:{group_id}] {sanitized_line}");
        }
    }

    // Decrement relay reference count when group ends
    relay_refcount.fetch_sub(1, Ordering::SeqCst);

    // Process ended - check if it was intentional or a crash
    if was_intentionally_stopped {
        stopping_groups.remove(&group_id);
        // Intentional stop via stop() - process already removed
        emit_event(event_sink.as_ref(), "stream_ended", &group_id);
    } else {
        // Process ended unexpectedly (crash, connection loss, etc.)
        // Remove from DashMap and check exit status
        let exit_status = {
            if let Some((_, mut info)) = processes.remove(&group_id) {
                // Try to get exit status
                match info.child.try_wait() {
                    Ok(Some(status)) => Some(status),
                    Ok(None) => info.child.wait().ok(),  // Process still running, wait for it
                    Err(_) => info.child.wait().ok(),    // Error checking, try to wait anyway
                }
            } else {
                None
            }
        };

        if stopping_groups.remove(&group_id).is_some() {
            emit_event(event_sink.as_ref(), "stream_ended", &group_id);
            return;
        }

        // Determine if this was a crash or normal exit
        let error_message = match exit_status {
            Some(status) if status.success() => {
                // FFmpeg exited cleanly (exit code 0)
                // This might happen if the input stream ended
                None
            }
            Some(status) => {
                // FFmpeg exited with error
                let code = status.code().unwrap_or(-1);
                Some(format!("FFmpeg exited with code {code}"))
            }
            None => {
                // Couldn't get exit status
                Some("FFmpeg process terminated unexpectedly".to_string())
            }
        };

        if let Some(error) = error_message {
            log::warn!("[FFmpeg:{group_id}] Stream error: {error}");
            if !recent_lines.is_empty() {
                log::warn!("[FFmpeg:{group_id}] Last stderr lines:");
                for entry in recent_lines {
                    log::warn!("[FFmpeg:{group_id}] {entry}");
                }
            }
            // Emit stream_error event with group_id and error message
            emit_event(
                event_sink.as_ref(),
                "stream_error",
                &serde_json::json!({
                    "groupId": group_id,
                    "error": error
                }),
            );
        } else {
            // Clean exit (input ended)
            emit_event(event_sink.as_ref(), "stream_ended", &group_id);
        }
    }

    // Check relay refcount and stop relay if no more groups are using it
    // Use atomic load to avoid race condition where multiple groups finish simultaneously
    let should_stop_relay = relay_refcount.load(Ordering::SeqCst) == 0;
    if should_stop_relay {
        if !processes.is_empty() {
            return;
        }
        if let Ok(mut relay_guard) = relay.lock() {
            if let Some(mut relay_proc) = relay_guard.take() {
                log::info!("Stopping relay process (no active groups)");
                kill_and_wait(&mut relay_proc.child);
            }
        }
    }
}
