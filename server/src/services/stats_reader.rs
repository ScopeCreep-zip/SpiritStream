// StatsReaderContext — encapsulates FFmpeg stderr reading and stats emission
//
// Extracted from the 244-line `FFmpegHandler::stats_reader` function to flatten
// deep nesting and separate concerns: bitrate metering, line buffering,
// stop detection, crash handling, and relay cleanup.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicU16, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::constants::{STATS_EMIT_INTERVAL_MS, STATS_RECENT_LINES_CAPACITY, STATS_BITRATE_SMOOTHING_ALPHA};
use crate::models::StreamStats;
use crate::services::{emit_event, EventSink};

use super::ffmpeg_handler::{FFmpegHandler, ProcessInfo, RelayProcess};

/// Bitrate metering with exponential moving average smoothing
struct BitrateMetering {
    meter_bytes: Arc<AtomicU64>,
    last_bytes: u64,
    last_instant: Instant,
    smoothed_bitrate: f64,
    has_sample: bool,
    has_smoothed: bool,
}

impl BitrateMetering {
    fn new(meter_bytes: Arc<AtomicU64>) -> Self {
        let last_bytes = meter_bytes.load(Ordering::Relaxed);
        Self {
            meter_bytes,
            last_bytes,
            last_instant: Instant::now(),
            smoothed_bitrate: 0.0,
            has_sample: false,
            has_smoothed: false,
        }
    }

    /// Update bitrate from the byte counter and return the smoothed value, if ready.
    fn update(&mut self) -> Option<f64> {
        let now = Instant::now();
        let current_bytes = self.meter_bytes.load(Ordering::Relaxed);

        if !self.has_sample {
            self.has_sample = true;
            self.last_bytes = current_bytes;
            self.last_instant = now;
            return None;
        }

        let elapsed = now.duration_since(self.last_instant).as_secs_f64();
        let delta_bytes = current_bytes.saturating_sub(self.last_bytes);

        self.last_bytes = current_bytes;
        self.last_instant = now;

        if elapsed <= 0.0 {
            return Some(self.smoothed_bitrate);
        }

        let kbps = (delta_bytes as f64 * 8.0) / 1000.0 / elapsed;
        if !kbps.is_finite() {
            return Some(0.0);
        }

        if self.has_smoothed {
            self.smoothed_bitrate =
                self.smoothed_bitrate * (1.0 - STATS_BITRATE_SMOOTHING_ALPHA) + kbps * STATS_BITRATE_SMOOTHING_ALPHA;
        } else {
            self.smoothed_bitrate = kbps;
            self.has_smoothed = true;
        }

        Some(self.smoothed_bitrate)
    }
}

/// Encapsulates all state for reading FFmpeg stderr and emitting stats.
pub(crate) struct StatsReaderContext {
    group_id: String,
    stats: StreamStats,
    last_emit: Instant,
    emit_interval: Duration,
    was_intentionally_stopped: bool,
    recent_lines: VecDeque<String>,
    meter: Option<BitrateMetering>,
    // Shared handles (same Arcs passed from FFmpegHandler)
    event_sink: Arc<dyn EventSink>,
    processes: Arc<Mutex<HashMap<String, ProcessInfo>>>,
    stopping_groups: Arc<Mutex<HashSet<String>>>,
    relay: Arc<Mutex<Option<RelayProcess>>>,
    relay_refcount: Arc<AtomicUsize>,
    port_assignments: Arc<Mutex<HashMap<String, u16>>>,
    next_port_offset: Arc<AtomicU16>,
}

impl StatsReaderContext {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        group_id: String,
        meter_bytes: Option<Arc<AtomicU64>>,
        event_sink: Arc<dyn EventSink>,
        processes: Arc<Mutex<HashMap<String, ProcessInfo>>>,
        stopping_groups: Arc<Mutex<HashSet<String>>>,
        relay: Arc<Mutex<Option<RelayProcess>>>,
        relay_refcount: Arc<AtomicUsize>,
        port_assignments: Arc<Mutex<HashMap<String, u16>>>,
        next_port_offset: Arc<AtomicU16>,
    ) -> Self {
        let stats = StreamStats::new(group_id.clone());
        let meter = meter_bytes.map(BitrateMetering::new);
        Self {
            group_id,
            stats,
            last_emit: Instant::now(),
            emit_interval: Duration::from_millis(STATS_EMIT_INTERVAL_MS),
            was_intentionally_stopped: false,
            recent_lines: VecDeque::with_capacity(STATS_RECENT_LINES_CAPACITY),
            meter,
            event_sink,
            processes,
            stopping_groups,
            relay,
            relay_refcount,
            port_assignments,
            next_port_offset,
        }
    }

    /// Check whether this group has been intentionally stopped.
    pub(crate) fn is_stopped(&self) -> bool {
        if let Ok(stopping) = self.stopping_groups.lock() {
            if stopping.contains(&self.group_id) {
                return true;
            }
        }
        if let Ok(procs) = self.processes.lock() {
            if !procs.contains_key(&self.group_id) {
                return true;
            }
        }
        false
    }

    pub(crate) fn mark_intentional_stop(&mut self) {
        self.was_intentionally_stopped = true;
    }

    /// Sanitize and buffer a stderr line.
    pub(crate) fn record_line(&mut self, line: &str) {
        let sanitized = FFmpegHandler::sanitize_arg_static(line);
        if self.recent_lines.len() == STATS_RECENT_LINES_CAPACITY {
            self.recent_lines.pop_front();
        }
        self.recent_lines.push_back(sanitized);
    }

    /// Parse the line into stats, returning whether it was parsed.
    pub(crate) fn parse_line(&mut self, line: &str) -> bool {
        self.stats.parse_line(line)
    }

    /// Emit stats if enough time has elapsed or at progress boundaries.
    pub(crate) fn maybe_emit_stats(&mut self, parsed: bool, is_progress: bool) {
        if !(is_progress || parsed && self.last_emit.elapsed() >= self.emit_interval) {
            return;
        }

        // Fill uptime from process start if FFmpeg doesn't report time
        if let Ok(procs) = self.processes.lock() {
            if let Some(info) = procs.get(&self.group_id) {
                let uptime = info.start_time.elapsed().as_secs_f64();
                if self.stats.time <= 0.0 {
                    self.stats.time = uptime;
                }
            }
        }

        self.update_bitrate();

        emit_event(self.event_sink.as_ref(), "stream_stats", &self.stats);
        self.last_emit = Instant::now();
    }

    /// Update bitrate from meter or fallback calculation.
    fn update_bitrate(&mut self) {
        if let Some(meter) = self.meter.as_mut() {
            if let Some(bitrate) = meter.update() {
                self.stats.bitrate = bitrate;
            }
        } else if self.stats.bitrate == 0.0 && self.stats.size > 0 && self.stats.time > 0.0 {
            let avg_kbps = (self.stats.size as f64 * 8.0) / 1000.0 / self.stats.time;
            if avg_kbps.is_finite() && avg_kbps > 0.0 {
                self.stats.bitrate = avg_kbps;
            }
        }
    }

    /// Log errors and warnings from FFmpeg stderr.
    pub(crate) fn log_errors(&self, raw_line: &str) {
        if raw_line.contains("[error]")
            || raw_line.contains("[warning]")
            || raw_line.contains("Error")
            || raw_line.contains("error")
        {
            // Use the last recorded (sanitized) line for logging
            if let Some(sanitized) = self.recent_lines.back() {
                log::warn!("[FFmpeg:{}] {sanitized}", self.group_id);
            }
        }
    }

    /// Handle process end — dispatch to intentional or unexpected end + relay cleanup.
    pub(crate) fn handle_process_end(&mut self) {
        self.relay_refcount.fetch_sub(1, Ordering::SeqCst);

        if self.was_intentionally_stopped {
            self.handle_intentional_stop();
        } else {
            self.handle_unexpected_end();
        }

        self.maybe_stop_relay();
    }

    fn handle_intentional_stop(&self) {
        if let Ok(mut stopping) = self.stopping_groups.lock() {
            stopping.remove(&self.group_id);
        }
        emit_event(self.event_sink.as_ref(), "stream_ended", &self.group_id);
    }

    fn handle_unexpected_end(&mut self) {
        let exit_status = {
            if let Ok(mut procs) = self.processes.lock() {
                if let Some(mut info) = procs.remove(&self.group_id) {
                    // Free the port assignment for this crashed group
                    if let Ok(mut assignments) = self.port_assignments.lock() {
                        if let Some(offset) = assignments.remove(&self.group_id) {
                            log::debug!(
                                "Freed port offset {offset} from crashed group {}",
                                self.group_id
                            );
                            if assignments.is_empty() {
                                self.next_port_offset.store(0, Ordering::SeqCst);
                                log::debug!("All ports freed after crash, reset counter to 0");
                            }
                        }
                    }

                    match info.child.try_wait() {
                        Ok(Some(status)) => Some(status),
                        Ok(None) => info.child.wait().ok(),
                        Err(_) => info.child.wait().ok(),
                    }
                } else {
                    None
                }
            } else {
                None
            }
        };

        // Check if a late stop arrived while we were processing
        if let Ok(mut stopping) = self.stopping_groups.lock() {
            if stopping.remove(&self.group_id) {
                emit_event(self.event_sink.as_ref(), "stream_ended", &self.group_id);
                return;
            }
        }

        let error_details = FFmpegHandler::parse_error_details(&self.recent_lines);

        let error_message = match exit_status {
            Some(status) if status.success() => None,
            Some(status) => {
                let code = status.code().unwrap_or(-1);
                let base_msg = format!("FFmpeg exited with code {code}");
                if let Some(details) = error_details {
                    Some(format!("{base_msg}: {details}"))
                } else {
                    Some(base_msg)
                }
            }
            None => {
                let base_msg = "FFmpeg process terminated unexpectedly".to_string();
                if let Some(details) = error_details {
                    Some(format!("{base_msg}: {details}"))
                } else {
                    Some(base_msg)
                }
            }
        };

        if let Some(error) = error_message {
            log::error!("[FFmpeg:{}] Stream crashed: {error}", self.group_id);
            if !self.recent_lines.is_empty() {
                log::warn!("[FFmpeg:{}] Last 10 stderr lines:", self.group_id);
                for entry in self.recent_lines.iter().rev().take(10).rev() {
                    log::warn!("[FFmpeg:{}]   {entry}", self.group_id);
                }
            }

            emit_event(
                self.event_sink.as_ref(),
                "stream_error",
                &serde_json::json!({
                    "groupId": self.group_id,
                    "error": error,
                    "canRetry": true,
                    "suggestion": "Stream connection lost. Click retry to reconnect automatically."
                }),
            );
        } else {
            emit_event(self.event_sink.as_ref(), "stream_ended", &self.group_id);
        }
    }

    fn maybe_stop_relay(&self) {
        let should_stop = self.relay_refcount.load(Ordering::SeqCst) == 0;
        if !should_stop {
            return;
        }
        if let Ok(procs) = self.processes.lock() {
            if !procs.is_empty() {
                return;
            }
        }
        if let Ok(mut relay_guard) = self.relay.lock() {
            if let Some(mut relay_proc) = relay_guard.take() {
                log::info!("Stopping relay process (no active groups)");
                let _ = relay_proc.child.kill();
                let _ = relay_proc.child.wait();
            }
        }
    }
}
