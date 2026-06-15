use std::collections::{HashMap, HashSet, VecDeque};
use std::io::{BufRead, BufReader};
use std::net::UdpSocket;
use std::sync::atomic::{AtomicU16, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use crate::models::{StreamErrorEvent, StreamStats};
use crate::services::{emit_event, EventSink};

use super::relay::{ProcessInfo, RelayProcess};
use super::ReconnectionState;

/// Uptime after which a group's retry budget resets — the stream
/// demonstrably works, so a later crash starts a fresh backoff cycle
/// instead of inheriting attempts from incidents long past.
const STABLE_RUN_RESET: Duration = Duration::from_secs(60);

/// Shared handles the per-group stats-reader thread needs to clean up
/// after an FFmpeg exit. Bundled so the spawn site and the reader can't
/// drift on argument order.
pub(super) struct StatsReaderCtx {
    pub(super) processes: Arc<Mutex<HashMap<String, ProcessInfo>>>,
    pub(super) stopping_groups: Arc<Mutex<HashSet<String>>>,
    pub(super) relay: Arc<Mutex<Option<RelayProcess>>>,
    pub(super) relay_refcount: Arc<AtomicUsize>,
    pub(super) port_assignments: Arc<Mutex<HashMap<String, u16>>>,
    pub(super) next_port_offset: Arc<AtomicU16>,
    pub(super) reconnection_states: Arc<Mutex<HashMap<String, ReconnectionState>>>,
    pub(super) run_dir: std::path::PathBuf,
    pub(super) ffmpeg_path: String,
}

impl StatsReaderCtx {
    /// Mirror of `FFmpegHandler::sync_process_registry` for the reader
    /// thread (which has no `&self`).
    fn sync_process_registry(&self) {
        let mut records: Vec<super::process_registry::StreamProcessRecord> = Vec::new();
        if let Ok(processes) = self.processes.lock() {
            for info in processes.values() {
                records.push(super::process_registry::StreamProcessRecord {
                    group_id: info.group_id.clone(),
                    pid: info.child.id(),
                    started_at_unix_ms: info.started_at_unix_ms,
                    ffmpeg_path: self.ffmpeg_path.clone(),
                });
            }
        }
        if let Ok(relay) = self.relay.lock() {
            if let Some(relay) = relay.as_ref() {
                records.push(super::process_registry::StreamProcessRecord {
                    group_id: super::process_registry::RELAY_GROUP_ID.to_string(),
                    pid: relay.child.id(),
                    started_at_unix_ms: 0,
                    ffmpeg_path: self.ffmpeg_path.clone(),
                });
            }
        }
        if let Err(e) = super::process_registry::write_records(&self.run_dir, &records) {
            // `{e:?}` deliberately: Display for CoreError::Internal hides
            // the context from CLIENTS, but this line is the server-side
            // log — the one place the context is supposed to surface.
            log::error!("failed to persist stream process registry: {e:?}");
        }
    }
}

impl super::FFmpegHandler {
    pub(super) fn start_bitrate_meter(
        &self,
        group_id: &str,
        processes: Arc<Mutex<HashMap<String, ProcessInfo>>>,
    ) -> Option<Arc<AtomicU64>> {
        let port = self.meter_port_for_group(group_id);
        let bind_addr = format!("{}:{}", Self::METER_HOST, port);
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

                if let Ok(procs) = processes.lock() {
                    if !procs.contains_key(&group_id) {
                        break;
                    }
                } else {
                    break;
                }
            }
        });

        Some(bytes)
    }

    /// Background thread that reads FFmpeg stderr and emits stats events.
    pub(super) fn stats_reader(
        stderr: std::process::ChildStderr,
        group_id: String,
        meter_bytes: Option<Arc<AtomicU64>>,
        event_sink: Arc<dyn EventSink>,
        ctx: StatsReaderCtx,
    ) {
        let StatsReaderCtx {
            ref processes,
            ref stopping_groups,
            ref relay,
            ref relay_refcount,
            ref port_assignments,
            ref next_port_offset,
            ref reconnection_states,
            ..
        } = ctx;
        let reader = BufReader::new(stderr);
        let mut stats = StreamStats::new(group_id.clone());
        let mut last_emit = Instant::now();
        let emit_interval = Duration::from_millis(1000);
        let mut was_intentionally_stopped = false;
        let mut retry_budget_reset = false;
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
            {
                if let Ok(stopping) = stopping_groups.lock() {
                    if stopping.contains(&group_id) {
                        was_intentionally_stopped = true;
                        break;
                    }
                }
                if let Ok(procs) = processes.lock() {
                    match procs.get(&group_id) {
                        None => {
                            // Process was removed by stop() — intentional stop.
                            was_intentionally_stopped = true;
                            break;
                        }
                        Some(info) => {
                            // Stable run → fresh retry budget for the next
                            // incident (see STABLE_RUN_RESET docs).
                            if !retry_budget_reset && info.start_time.elapsed() >= STABLE_RUN_RESET
                            {
                                retry_budget_reset = true;
                                if let Ok(mut states) = reconnection_states.lock() {
                                    states.remove(&group_id);
                                }
                            }
                        }
                    }
                }
            }

            let sanitized_line = Self::sanitize_arg_static(&line);
            if recent_lines.len() == 40 {
                recent_lines.pop_front();
            }
            recent_lines.push_back(sanitized_line.clone());

            let parsed = stats.parse_line(&line);
            let is_progress_line = line.trim_start().starts_with("progress=");

            if is_progress_line || (parsed && last_emit.elapsed() >= emit_interval) {
                if let Ok(procs) = processes.lock() {
                    if let Some(info) = procs.get(&group_id) {
                        let uptime = info.start_time.elapsed().as_secs_f64();
                        if stats.time <= 0.0 {
                            stats.time = uptime;
                        }
                    }
                }

                if meter_bytes.is_none()
                    && stats.bitrate == 0.0
                    && stats.size > 0
                    && stats.time > 0.0
                {
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
                                    smoothed_bitrate =
                                        smoothed_bitrate * (1.0 - alpha) + kbps * alpha;
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

                emit_event(event_sink.as_ref(), "stream_stats", &stats);
                last_emit = Instant::now();
            }

            // Only log errors and warnings (frame stats are too verbose).
            if line.contains("[error]")
                || line.contains("[warning]")
                || line.contains("Error")
                || line.contains("error")
            {
                log::warn!("[FFmpeg:{group_id}] {sanitized_line}");
            }
        }

        relay_refcount.fetch_sub(1, Ordering::SeqCst);

        if was_intentionally_stopped {
            if let Ok(mut stopping) = stopping_groups.lock() {
                stopping.remove(&group_id);
            }
            // Intentional stop via stop() — process already removed.
            emit_event(event_sink.as_ref(), "stream_ended", &group_id);
        } else {
            // Unexpected exit (crash, connection loss). Remove from HashMap
            // and check exit status.
            let exit_status = {
                if let Ok(mut procs) = processes.lock() {
                    if let Some(mut info) = procs.remove(&group_id) {
                        // Free the port assignment for this crashed group.
                        if let Ok(mut assignments) = port_assignments.lock() {
                            if let Some(offset) = assignments.remove(&group_id) {
                                log::debug!(
                                    "Freed port offset {offset} from crashed group {group_id}"
                                );
                                if assignments.is_empty() {
                                    next_port_offset.store(0, Ordering::SeqCst);
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
            ctx.sync_process_registry();

            if let Ok(mut stopping) = stopping_groups.lock() {
                if stopping.remove(&group_id) {
                    emit_event(event_sink.as_ref(), "stream_ended", &group_id);
                    return;
                }
            }

            let error_details = Self::parse_error_details(&recent_lines);

            let error_message = match exit_status {
                Some(status) if status.success() => {
                    // FFmpeg exited cleanly (exit code 0) — input stream
                    // probably ended.
                    None
                }
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
                log::error!("[FFmpeg:{group_id}] Stream crashed: {error}");
                if !recent_lines.is_empty() {
                    log::warn!("[FFmpeg:{group_id}] Last 10 stderr lines:");
                    for entry in recent_lines.iter().rev().take(10).rev() {
                        log::warn!("[FFmpeg:{group_id}]   {entry}");
                    }
                }

                emit_event(
                    event_sink.as_ref(),
                    "stream_error",
                    &StreamErrorEvent {
                        group_id: group_id.clone(),
                        error,
                        can_retry: true,
                        suggestion: "Stream connection lost. Click retry to reconnect \
                                     automatically."
                            .to_string(),
                    },
                );
            } else {
                emit_event(event_sink.as_ref(), "stream_ended", &group_id);
            }
        }

        // Check relay refcount and stop relay if no more groups are using it.
        // Atomic load avoids the race where multiple groups finish simultaneously.
        let should_stop_relay = relay_refcount.load(Ordering::SeqCst) == 0;
        if should_stop_relay {
            if let Ok(procs) = processes.lock() {
                if !procs.is_empty() {
                    return;
                }
            }
            if let Ok(mut relay_guard) = relay.lock() {
                if let Some(mut relay_proc) = relay_guard.take() {
                    log::info!("Stopping relay process (no active groups)");
                    let _ = relay_proc.child.kill();
                    let _ = relay_proc.child.wait();
                }
            }
        }
    }
}
