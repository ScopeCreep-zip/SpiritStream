// FFmpegHandler Service
// Manages FFmpeg processes for streaming with real-time stats

use std::collections::{HashMap, HashSet, VecDeque};
use std::io::{BufRead, BufReader, Write};
use std::net::UdpSocket;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::thread;
use std::time::{Duration, Instant};
use crate::services::{emit_event, EventSink};
use crate::models::{OutputGroup, StreamStats};
use crate::services::PlatformRegistry;

// Windows: Hide console windows for spawned processes
#[cfg(windows)]
use std::os::windows::process::CommandExt;
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

/// Process info for tracking active streams
struct ProcessInfo {
    child: Child,
    start_time: Instant,
    group_id: String,
}

/// Cached configuration for restarting groups when relay output set changes
struct ActiveGroupConfig {
    group: OutputGroup,
    incoming_url: String,
}

/// FFmpeg relay process for shared ingest
struct RelayProcess {
    child: Child,
    incoming_url: String,
    output_groups: HashSet<String>,
}

/// Manages FFmpeg streaming processes
pub struct FFmpegHandler {
    ffmpeg_path: String,
    processes: Arc<Mutex<HashMap<String, ProcessInfo>>>,
    stopping_groups: Arc<Mutex<HashSet<String>>>,
    disabled_targets: Arc<Mutex<HashSet<String>>>,
    relay: Arc<Mutex<Option<RelayProcess>>>,
    active_groups: Arc<Mutex<HashMap<String, ActiveGroupConfig>>>,
    /// Reference count for active groups using the relay
    /// Prevents race condition where relay stops while groups are still active
    relay_refcount: Arc<AtomicUsize>,
    /// Platform registry for URL normalization and redaction
    platform_registry: PlatformRegistry,
}

impl FFmpegHandler {
    // Unicast relay fan-out using deterministic local TCP ports per group
    const RELAY_HOST: &'static str = "localhost";
    const RELAY_PORT_BASE: u16 = 20000;
    const RELAY_PORT_RANGE: u16 = 20000;
    const RELAY_TCP_OUT_QUERY: &'static str = "tcp_nodelay=1";
    const RELAY_TCP_IN_QUERY: &'static str = "listen=1&tcp_nodelay=1";
    const RELAY_RTMP_TIMEOUT_SECS: u32 = 604_800;
    const RELAY_RTMP_TCP_NODELAY: &'static str = "1";
    const RELAY_TEE_FIFO_OPTIONS: &'static str =
        "fifo_format=mpegts:queue_size=512:drop_pkts_on_overflow=1:attempt_recovery=1:recover_any_error=1";
    const METER_HOST: &'static str = "127.0.0.1";
    const METER_PORT_BASE: u16 = 40000;
    const METER_PORT_RANGE: u16 = 10000;
    const METER_UDP_QUERY: &'static str = "pkt_size=1316";

    /// Create FFmpegHandler with optional custom FFmpeg path from settings
    /// Falls back to auto-discovery if custom path is empty or invalid
    pub fn new_with_custom_path(app_data_dir: PathBuf, custom_path: Option<String>) -> Self {
        let ffmpeg_path = match custom_path {
            Some(ref path) if !path.is_empty() && std::path::Path::new(path).exists() => {
                log::info!("Using custom FFmpeg path from settings: {path}");
                path.clone()
            }
            _ => {
                log::info!("Using auto-detected FFmpeg path");
                Self::find_ffmpeg_with_bundled(app_data_dir)
            }
        };

        Self {
            ffmpeg_path,
            processes: Arc::new(Mutex::new(HashMap::new())),
            stopping_groups: Arc::new(Mutex::new(HashSet::new())),
            disabled_targets: Arc::new(Mutex::new(HashSet::new())),
            relay: Arc::new(Mutex::new(None)),
            active_groups: Arc::new(Mutex::new(HashMap::new())),
            relay_refcount: Arc::new(AtomicUsize::new(0)),
            platform_registry: PlatformRegistry::new(),
        }
    }

    /// Create a new FFmpegHandler (legacy, without bundled FFmpeg support)
    pub fn new() -> Self {
        Self {
            ffmpeg_path: Self::find_ffmpeg(),
            processes: Arc::new(Mutex::new(HashMap::new())),
            stopping_groups: Arc::new(Mutex::new(HashSet::new())),
            disabled_targets: Arc::new(Mutex::new(HashSet::new())),
            relay: Arc::new(Mutex::new(None)),
            active_groups: Arc::new(Mutex::new(HashMap::new())),
            relay_refcount: Arc::new(AtomicUsize::new(0)),
            platform_registry: PlatformRegistry::new(),
        }
    }

    /// Normalize an RTMP URL for consistency
    fn normalize_rtmp_url(url: &str) -> String {
        let mut url = url.trim().to_string();

        // Remove trailing slashes
        while url.ends_with('/') {
            url.pop();
        }

        // Ensure rtmp:// or rtmps:// prefix if missing
        if !url.starts_with("rtmp://") && !url.starts_with("rtmps://") {
            // Check if it looks like it should be rtmps (common rtmps ports or hosts)
            if url.contains(":443") || url.contains("facebook.com") {
                url = format!("rtmps://{url}");
            } else {
                url = format!("rtmp://{url}");
            }
        }

        url
    }


    /// Sanitize a single argument with platform context for accurate redaction
    fn sanitize_arg_with_context(&self, arg: &str, group: &OutputGroup) -> String {
        if !(arg.contains("rtmp://") || arg.contains("rtmps://")) {
            return arg.to_string();
        }

        let mut parts = Vec::new();
        for segment in arg.split('|') {
            let redacted = if let Some(pos) = segment.find("rtmp://").or_else(|| segment.find("rtmps://")) {
                let prefix = &segment[..pos];
                let url_start = pos;
                let url_end = segment[url_start..].find(' ').map(|i| url_start + i).unwrap_or(segment.len());
                let url = &segment[url_start..url_end];
                let suffix = &segment[url_end..];

                // Try to find matching target to get platform
                let platform_redacted = group.stream_targets.iter()
                    .find(|target| {
                        // Check if this URL belongs to this target by matching the base URL
                        let normalized = Self::normalize_rtmp_url(&target.url);
                        url.starts_with(&normalized) || url.contains(&target.url)
                    })
                    .and_then(|target| {
                        // Use platform-specific redaction
                        self.platform_registry.get(&target.service)
                            .map(|config| config.redact_url(url))
                    });

                let redacted_url = platform_redacted.unwrap_or_else(|| {
                    // Fallback to generic redaction
                    PlatformRegistry::generic_redact(url)
                });

                format!("{prefix}{redacted_url}{suffix}")
            } else {
                segment.to_string()
            };
            parts.push(redacted);
        }

        parts.join("|")
    }

    /// Sanitize all FFmpeg arguments (redact stream keys) with platform-aware redaction
    fn sanitize_ffmpeg_args(&self, args: &[String], group: &OutputGroup) -> Vec<String> {
        args.iter().map(|arg| self.sanitize_arg_with_context(arg, group)).collect()
    }

    /// Static version of sanitize_arg for use in background threads
    /// Uses generic platform-agnostic redaction
    fn sanitize_arg_static(arg: &str) -> String {
        if !(arg.contains("rtmp://") || arg.contains("rtmps://")) {
            return arg.to_string();
        }

        let mut parts = Vec::new();
        for segment in arg.split('|') {
            let redacted = if let Some(pos) = segment.find("rtmp://") {
                let prefix = &segment[..pos];
                let url_start = pos;
                let url_end = segment[url_start..].find(' ').map(|i| url_start + i).unwrap_or(segment.len());
                let url = &segment[url_start..url_end];
                let suffix = &segment[url_end..];
                format!("{prefix}{}{suffix}", PlatformRegistry::generic_redact(url))
            } else if let Some(pos) = segment.find("rtmps://") {
                let prefix = &segment[..pos];
                let url_start = pos;
                let url_end = segment[url_start..].find(' ').map(|i| url_start + i).unwrap_or(segment.len());
                let url = &segment[url_start..url_end];
                let suffix = &segment[url_end..];
                format!("{prefix}{}{suffix}", PlatformRegistry::generic_redact(url))
            } else {
                segment.to_string()
            };
            parts.push(redacted);
        }

        parts.join("|")
    }

    /// Find FFmpeg at the system install location (where we download to)
    /// Only checks the standard system path - no PATH searching or common location fallbacks
    fn find_ffmpeg_with_bundled(_app_data_dir: PathBuf) -> String {
        use crate::services::FFmpegDownloader;

        // Check the system install path (where we download FFmpeg to)
        let system_path = FFmpegDownloader::get_system_install_path();
        if system_path.exists() {
            log::info!("Using system FFmpeg: {system_path:?}");
            return system_path.to_string_lossy().to_string();
        }

        // No FFmpeg found - return the expected path anyway
        // This will cause FFmpeg commands to fail with a clear error
        log::warn!("FFmpeg not found at system location: {system_path:?}");
        system_path.to_string_lossy().to_string()
    }

    /// Legacy find_ffmpeg - now just delegates to system path check
    fn find_ffmpeg() -> String {
        use crate::services::FFmpegDownloader;
        FFmpegDownloader::get_system_install_path().to_string_lossy().to_string()
    }

    fn record_active_group(&self, group: &OutputGroup, incoming_url: &str) -> Result<(), String> {
        let mut active = self.active_groups.lock()
            .map_err(|e| format!("Lock poisoned: {e}"))?;

        if let Some(existing) = active.values().next() {
            if existing.incoming_url != incoming_url {
                return Err("Incoming URL differs from active groups".to_string());
            }
        }

        active.insert(group.id.clone(), ActiveGroupConfig {
            group: group.clone(),
            incoming_url: incoming_url.to_string(),
        });

        Ok(())
    }

    fn remove_active_group(&self, group_id: &str) {
        if let Ok(mut active) = self.active_groups.lock() {
            active.remove(group_id);
        }
    }

    fn collect_active_group_ids(&self) -> Result<HashSet<String>, String> {
        let active = self.active_groups.lock()
            .map_err(|e| format!("Lock poisoned: {e}"))?;
        Ok(active.keys().cloned().collect())
    }

    fn resolve_active_incoming_url(&self) -> Result<String, String> {
        let active = self.active_groups.lock()
            .map_err(|e| format!("Lock poisoned: {e}"))?;
        let mut incoming_url: Option<String> = None;
        for cfg in active.values() {
            if let Some(existing) = &incoming_url {
                if existing != &cfg.incoming_url {
                    return Err("Incoming URL differs across active groups".to_string());
                }
            } else {
                incoming_url = Some(cfg.incoming_url.clone());
            }
        }
        incoming_url.ok_or_else(|| "No active groups available".to_string())
    }

    fn get_group_pid(&self, group_id: &str) -> Option<u32> {
        self.processes.lock().ok()
            .and_then(|procs| procs.get(group_id).map(|info| info.child.id()))
    }

    fn relay_needs_restart(&self, desired_group_ids: &HashSet<String>) -> Result<bool, String> {
        let relay_guard = self.relay.lock()
            .map_err(|e| format!("Lock poisoned: {e}"))?;
        if let Some(relay) = relay_guard.as_ref() {
            if !relay.output_groups.is_superset(desired_group_ids) {
                let processes = self.processes.lock()
                    .map_err(|e| format!("Lock poisoned: {e}"))?;
                return Ok(!processes.is_empty());
            }
        }
        Ok(false)
    }

    fn is_relay_active(&self) -> Result<bool, String> {
        let mut relay_guard = self.relay.lock()
            .map_err(|e| format!("Lock poisoned: {e}"))?;
        if let Some(relay) = relay_guard.as_mut() {
            if let Ok(Some(_)) = relay.child.try_wait() {
                *relay_guard = None;
                return Ok(false);
            }
            return Ok(true);
        }
        Ok(false)
    }

    fn stop_group_for_restart(&self, group_id: &str) -> Result<(), String> {
        if let Ok(mut stopping) = self.stopping_groups.lock() {
            stopping.insert(group_id.to_string());
        }
        let removed = {
            let mut processes = self.processes.lock()
                .map_err(|e| format!("Lock poisoned: {e}"))?;
            processes.remove(group_id)
        };

        if let Some(mut info) = removed {
            self.stop_child(&mut info.child);
        }
        Ok(())
    }

    fn start_group_process(
        &self,
        group: &OutputGroup,
        event_sink: Arc<dyn EventSink>,
    ) -> Result<u32, String> {
        let args = self.build_args(group);
        let sanitized = self.sanitize_ffmpeg_args(&args, group);
        log::info!(
            "Starting FFmpeg group {}: {} {}",
            group.id,
            self.ffmpeg_path,
            sanitized.join(" ")
        );

        let mut cmd = Command::new(&self.ffmpeg_path);
        cmd.args(&args)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());
        #[cfg(windows)]
        cmd.creation_flags(CREATE_NO_WINDOW);
        let mut child = cmd.spawn()
            .map_err(|e| format!("Failed to start FFmpeg: {e}"))?;

        let pid = child.id();
        let group_id = group.id.clone();

        let stderr = child.stderr.take()
            .ok_or_else(|| "Failed to capture FFmpeg stderr".to_string())?;

        {
            let mut processes = self.processes.lock()
                .map_err(|e| format!("Lock poisoned: {e}"))?;
            processes.insert(group_id.clone(), ProcessInfo {
                child,
                start_time: Instant::now(),
                group_id: group_id.clone(),
            });
        }

        self.relay_refcount.fetch_add(1, Ordering::SeqCst);

        let event_sink_clone = Arc::clone(&event_sink);
        let processes_clone = Arc::clone(&self.processes);
        let meter_bytes = Self::start_bitrate_meter(&group_id, Arc::clone(&processes_clone));
        let relay_clone = Arc::clone(&self.relay);
        let stopping_clone = Arc::clone(&self.stopping_groups);
        let relay_refcount_clone = Arc::clone(&self.relay_refcount);
        let group_id_clone = group_id.clone();

        thread::spawn(move || {
            Self::stats_reader(
                stderr,
                group_id_clone,
                meter_bytes,
                event_sink_clone,
                processes_clone,
                stopping_clone,
                relay_clone,
                relay_refcount_clone,
            );
        });

        Ok(pid)
    }

    fn restart_relay_with_groups(
        &self,
        requested_group_id: &str,
        event_sink: Arc<dyn EventSink>,
    ) -> Result<u32, String> {
        let incoming_url = self.resolve_active_incoming_url()?;
        let desired_group_ids = self.collect_active_group_ids()?;

        let running_group_ids = self.get_active_group_ids();
        for group_id in running_group_ids {
            let _ = self.stop_group_for_restart(&group_id);
        }
        self.stop_relay();

        let active_groups = self.active_groups.lock()
            .map_err(|e| format!("Lock poisoned: {e}"))?;
        let mut requested_pid: Option<u32> = None;
        for (group_id, cfg) in active_groups.iter() {
            let pid = self.start_group_process(&cfg.group, Arc::clone(&event_sink))?;
            if group_id == requested_group_id {
                requested_pid = Some(pid);
            }
        }

        self.ensure_relay_running(&incoming_url, &desired_group_ids)?;

        requested_pid.ok_or_else(|| "Requested group not started".to_string())
    }

    /// Start streaming for an output group with stats monitoring
    pub fn start(
        &self,
        group: &OutputGroup,
        incoming_url: &str,
        event_sink: Arc<dyn EventSink>,
    ) -> Result<u32, String> {
        self.record_active_group(group, incoming_url)?;

        if let Some(pid) = self.get_group_pid(&group.id) {
            return Ok(pid);
        }

        let desired_group_ids = self.collect_active_group_ids()?;
        if self.relay_needs_restart(&desired_group_ids)? {
            return self.restart_relay_with_groups(&group.id, event_sink);
        }

        if !self.is_relay_active()? {
            let pid = self.start_group_process(group, Arc::clone(&event_sink))?;
            self.ensure_relay_running(incoming_url, &desired_group_ids)?;
            return Ok(pid);
        }

        self.ensure_relay_running(incoming_url, &desired_group_ids)?;
        self.start_group_process(group, event_sink)
    }

    /// Start streaming for multiple output groups in one batch
    pub fn start_all(
        &self,
        groups: &[OutputGroup],
        incoming_url: &str,
        event_sink: Arc<dyn EventSink>,
    ) -> Result<Vec<u32>, String> {
        if self.active_count() > 0 {
            return Err("Streams already running".to_string());
        }

        if let Ok(mut active) = self.active_groups.lock() {
            active.clear();
        }

        let mut desired_group_ids: HashSet<String> = HashSet::new();
        let mut start_groups: Vec<OutputGroup> = Vec::new();
        for group in groups {
            if group.stream_targets.is_empty() {
                continue;
            }
            self.record_active_group(group, incoming_url)?;
            desired_group_ids.insert(group.id.clone());
            start_groups.push(group.clone());
        }

        if start_groups.is_empty() {
            return Err("At least one stream target is required".to_string());
        }

        let mut pids = Vec::with_capacity(start_groups.len());
        for group in &start_groups {
            let pid = self.start_group_process(group, Arc::clone(&event_sink))?;
            pids.push(pid);
        }

        self.ensure_relay_running(incoming_url, &desired_group_ids)?;

        Ok(pids)
    }

    fn start_bitrate_meter(
        group_id: &str,
        processes: Arc<Mutex<HashMap<String, ProcessInfo>>>,
    ) -> Option<Arc<AtomicU64>> {
        let port = Self::meter_port_for_group(group_id);
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

    /// Background thread that reads FFmpeg stderr and emits stats events
    #[allow(clippy::too_many_arguments)]
    fn stats_reader(
        stderr: std::process::ChildStderr,
        group_id: String,
        meter_bytes: Option<Arc<AtomicU64>>,
        event_sink: Arc<dyn EventSink>,
        processes: Arc<Mutex<HashMap<String, ProcessInfo>>>,
        stopping_groups: Arc<Mutex<HashSet<String>>>,
        relay: Arc<Mutex<Option<RelayProcess>>>,
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
                if let Ok(stopping) = stopping_groups.lock() {
                    if stopping.contains(&group_id) {
                        was_intentionally_stopped = true;
                        break;
                    }
                }
                if let Ok(procs) = processes.lock() {
                    if !procs.contains_key(&group_id) {
                        // Process was removed by stop() - intentional stop
                        was_intentionally_stopped = true;
                        break;
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

            // Emit stats at most every second or at progress boundaries
            if is_progress_line || (parsed && last_emit.elapsed() >= emit_interval) {
                // Add uptime from process start if FFmpeg doesn't report time
                if let Ok(procs) = processes.lock() {
                    if let Some(info) = procs.get(&group_id) {
                        let uptime = info.start_time.elapsed().as_secs_f64();
                        if stats.time <= 0.0 {
                            stats.time = uptime;
                        }
                    }
                }

                if meter_bytes.is_none() && stats.bitrate == 0.0 && stats.size > 0 && stats.time > 0.0 {
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
            if let Ok(mut stopping) = stopping_groups.lock() {
                stopping.remove(&group_id);
            }
            // Intentional stop via stop() - process already removed
            emit_event(event_sink.as_ref(), "stream_ended", &group_id);
        } else {
            // Process ended unexpectedly (crash, connection loss, etc.)
            // Remove from HashMap and check exit status
            let exit_status = {
                if let Ok(mut procs) = processes.lock() {
                    if let Some(mut info) = procs.remove(&group_id) {
                        // Try to get exit status
                        match info.child.try_wait() {
                            Ok(Some(status)) => Some(status),
                            Ok(None) => info.child.wait().ok(),  // Process still running, wait for it
                            Err(_) => info.child.wait().ok(),    // Error checking, try to wait anyway
                        }
                    } else {
                        None
                    }
                } else {
                    None
                }
            };

            if let Ok(mut stopping) = stopping_groups.lock() {
                if stopping.remove(&group_id) {
                    emit_event(event_sink.as_ref(), "stream_ended", &group_id);
                    return;
                }
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

    /// Stop streaming for an output group
    pub fn stop(&self, group_id: &str) -> Result<(), String> {
        self.remove_active_group(group_id);
        if let Ok(mut stopping) = self.stopping_groups.lock() {
            stopping.insert(group_id.to_string());
        }
        let (removed, should_stop_relay) = {
            let mut processes = self.processes.lock()
                .map_err(|e| format!("Lock poisoned: {e}"))?;
            let removed = processes.remove(group_id);
            let should_stop_relay = processes.is_empty();
            (removed, should_stop_relay)
        };

        if let Some(mut info) = removed {
            self.stop_child(&mut info.child);
        }
        if should_stop_relay {
            self.stop_relay();
        }
        Ok(())
    }

    /// Stop all active streams
    pub fn stop_all(&self) -> Result<(), String> {
        if let Ok(mut active) = self.active_groups.lock() {
            active.clear();
        }
        let mut processes = self.processes.lock()
            .map_err(|e| format!("Lock poisoned: {e}"))?;
        let mut stopping = self.stopping_groups.lock()
            .map_err(|e| format!("Lock poisoned: {e}"))?;
        for (group_id, mut info) in processes.drain() {
            stopping.insert(group_id);
            self.stop_child(&mut info.child);
        }
        self.stop_relay();
        Ok(())
    }

    /// Get active stream count
    pub fn active_count(&self) -> usize {
        self.processes.lock()
            .map(|procs| procs.len())
            .unwrap_or(0)
    }

    /// Check if a group is streaming
    pub fn is_streaming(&self, group_id: &str) -> bool {
        self.processes.lock()
            .map(|procs| procs.contains_key(group_id))
            .unwrap_or(false)
    }

    /// Get list of active stream group IDs
    pub fn get_active_group_ids(&self) -> Vec<String> {
        self.processes.lock()
            .map(|procs| procs.values().map(|info| info.group_id.clone()).collect())
            .unwrap_or_default()
    }

    /// Enable a specific stream target (removes from disabled set)
    pub fn enable_target(&self, target_id: &str) {
        let mut disabled = self.disabled_targets.lock().unwrap_or_else(|e| {
            log::warn!("Disabled targets mutex poisoned (enable_target), recovering: {e}");
            e.into_inner()
        });
        disabled.remove(target_id);
    }

    /// Disable a specific stream target (adds to disabled set)
    pub fn disable_target(&self, target_id: &str) {
        let mut disabled = self.disabled_targets.lock().unwrap_or_else(|e| {
            log::warn!("Disabled targets mutex poisoned (disable_target), recovering: {e}");
            e.into_inner()
        });
        disabled.insert(target_id.to_string());
    }

    /// Check if a target is currently disabled
    pub fn is_target_disabled(&self, target_id: &str) -> bool {
        let disabled = self.disabled_targets.lock().unwrap_or_else(|e| {
            log::warn!("Disabled targets mutex poisoned (is_target_disabled), recovering: {e}");
            e.into_inner()
        });
        disabled.contains(target_id)
    }

    /// Ensure relay process is running for shared ingest
    fn ensure_relay_running(
        &self,
        incoming_url: &str,
        requested_groups: &HashSet<String>,
    ) -> Result<(), String> {
        let mut relay_guard = self.relay.lock()
            .map_err(|e| format!("Lock poisoned: {e}"))?;

        if let Some(relay) = relay_guard.as_mut() {
            if let Ok(Some(_)) = relay.child.try_wait() {
                *relay_guard = None;
            }
        }

        if let Some(relay) = relay_guard.as_ref() {
            if relay.incoming_url != incoming_url {
                return Err("Incoming URL differs from active relay input".to_string());
            }

            if relay.output_groups.is_superset(requested_groups) {
                return Ok(());
            }
        }

        if requested_groups.is_empty() {
            return Err("No output groups provided for relay fan-out".to_string());
        }

        let mut relay_groups = if let Some(relay) = relay_guard.as_ref() {
            relay.output_groups.clone()
        } else {
            HashSet::new()
        };
        relay_groups.extend(requested_groups.iter().cloned());

        if let Some(mut relay) = relay_guard.take() {
            let _ = relay.child.kill();
            let _ = relay.child.wait();
        }

        let args = self.build_relay_args(incoming_url, &relay_groups)?;
        let sanitized: Vec<String> = args.iter().map(|arg| Self::sanitize_arg_static(arg)).collect();
        log::info!(
            "Starting FFmpeg relay: {} {}",
            self.ffmpeg_path,
            sanitized.join(" ")
        );
        let mut cmd = Command::new(&self.ffmpeg_path);
        cmd.args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());
        #[cfg(windows)]
        cmd.creation_flags(CREATE_NO_WINDOW);
        let mut child = cmd.spawn()
            .map_err(|e| format!("Failed to start FFmpeg relay: {e}"))?;

        if let Some(stderr) = child.stderr.take() {
            thread::spawn(move || {
                let reader = BufReader::new(stderr);
                for line in reader.lines().map_while(Result::ok) {
                    let sanitized = Self::sanitize_arg_static(&line);
                    if line.contains("[error]")
                        || line.contains("[warning]")
                        || line.contains("Error")
                        || line.contains("error")
                        || line.contains("Failed")
                        || line.contains("failed")
                        || line.contains("Connection")
                        || line.contains("connection")
                        || line.contains("listen")
                    {
                        log::warn!("[FFmpeg:relay] {sanitized}");
                    }
                }
            });
        }

        *relay_guard = Some(RelayProcess {
            child,
            incoming_url: incoming_url.to_string(),
            output_groups: relay_groups,
        });

        Ok(())
    }

    /// Stop the relay process if running
    fn stop_relay(&self) {
        let mut relay_guard = match self.relay.lock() {
            Ok(guard) => guard,
            Err(_) => return,
        };

        if let Some(mut relay) = relay_guard.take() {
            let _ = relay.child.kill();
            let _ = relay.child.wait();
        }
    }

    fn stop_child(&self, child: &mut Child) {
        if let Some(stdin) = child.stdin.as_mut() {
            let _ = stdin.write_all(b"q\n");
            let _ = stdin.flush();
        }

        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(2) {
            if let Ok(Some(_)) = child.try_wait() {
                return;
            }
            thread::sleep(Duration::from_millis(100));
        }

        let _ = child.kill();
        let _ = child.wait();
    }

    /// Restart a specific group (used after toggling targets)
    /// This stops the group and restarts it with the updated target list
    pub fn restart_group(
        &self,
        group_id: &str,
        group: &OutputGroup,
        incoming_url: &str,
        event_sink: Arc<dyn EventSink>,
    ) -> Result<u32, String> {
        // Stop the group if it's running
        if self.is_streaming(group_id) {
            self.stop(group_id)?;
        }

        // Start with updated target list (disabled targets will be filtered out)
        self.start(group, incoming_url, event_sink)
    }

    /// Resolve stream key - supports ${ENV_VAR} syntax
    fn resolve_stream_key(key: &str) -> String {
        // Check if key matches ${VAR_NAME} pattern
        if key.starts_with("${") && key.ends_with("}") && key.len() > 3 {
            let var_name = &key[2..key.len()-1];
            match std::env::var(var_name) {
                Ok(value) => {
                    // Security: Do not log the variable name to prevent revealing
                    // which environment variables contain sensitive credentials
                    log::debug!("Resolved stream key from environment variable");
                    value
                }
                Err(_) => {
                    // Security: Do not log the variable name to prevent revealing
                    // credential-related configuration details
                    log::warn!("Environment variable not found for stream key, check your configuration");
                    key.to_string()
                }
            }
        } else {
            key.to_string()
        }
    }

    /// Build FFmpeg arguments for the shared relay process
    fn build_relay_args(
        &self,
        incoming_url: &str,
        group_ids: &HashSet<String>,
    ) -> Result<Vec<String>, String> {
        if group_ids.is_empty() {
            return Err("Relay fan-out requires at least one group".to_string());
        }

        let outputs = Self::relay_tee_output_list(group_ids);
        let listen_url = Self::normalize_relay_input_url(incoming_url);
        Ok(vec![
            "-listen".to_string(),
            "1".to_string(),
            "-timeout".to_string(),
            Self::RELAY_RTMP_TIMEOUT_SECS.to_string(),
            "-tcp_nodelay".to_string(),
            Self::RELAY_RTMP_TCP_NODELAY.to_string(),
            "-i".to_string(),
            listen_url,
            "-c:v".to_string(),
            "copy".to_string(),
            "-c:a".to_string(),
            "copy".to_string(),
            "-map".to_string(),
            "0:v".to_string(),
            "-map".to_string(),
            "0:a".to_string(),
            "-f".to_string(),
            "tee".to_string(),
            "-use_fifo".to_string(),
            "1".to_string(),
            "-fifo_options".to_string(),
            Self::RELAY_TEE_FIFO_OPTIONS.to_string(),
            outputs,
        ])
    }

    fn relay_port_for_group(group_id: &str) -> u16 {
        const FNV_OFFSET: u32 = 2166136261;
        const FNV_PRIME: u32 = 16777619;

        let mut hash = FNV_OFFSET;
        for &b in group_id.as_bytes() {
            hash ^= b as u32;
            hash = hash.wrapping_mul(FNV_PRIME);
        }

        let range = Self::RELAY_PORT_RANGE as u32;
        let port = Self::RELAY_PORT_BASE as u32 + (hash % range);
        port as u16
    }

    fn relay_output_url_for_group(group_id: &str) -> String {
        format!(
            "tcp://{}:{}?{}",
            Self::RELAY_HOST,
            Self::relay_port_for_group(group_id),
            Self::RELAY_TCP_OUT_QUERY
        )
    }

    fn relay_input_url_for_group(group_id: &str) -> String {
        format!(
            "tcp://{}:{}?{}",
            Self::RELAY_HOST,
            Self::relay_port_for_group(group_id),
            Self::RELAY_TCP_IN_QUERY
        )
    }

    fn meter_port_for_group(group_id: &str) -> u16 {
        const FNV_OFFSET: u32 = 2166136261;
        const FNV_PRIME: u32 = 16777619;

        let mut hash = FNV_OFFSET;
        for &b in group_id.as_bytes() {
            hash ^= b as u32;
            hash = hash.wrapping_mul(FNV_PRIME);
        }

        let range = Self::METER_PORT_RANGE as u32;
        let port = Self::METER_PORT_BASE as u32 + (hash % range);
        port as u16
    }

    fn meter_output_url_for_group(group_id: &str) -> String {
        format!(
            "udp://{}:{}?{}",
            Self::METER_HOST,
            Self::meter_port_for_group(group_id),
            Self::METER_UDP_QUERY
        )
    }

    fn relay_tee_output_list(group_ids: &HashSet<String>) -> String {
        let mut ids: Vec<&String> = group_ids.iter().collect();
        ids.sort();
        ids.into_iter()
            .map(|id| format!("[f=mpegts]{}", Self::relay_output_url_for_group(id)))
            .collect::<Vec<String>>()
            .join("|")
    }
    
    fn normalize_relay_input_url(url: &str) -> String {
        if !(url.starts_with("rtmp://") || url.starts_with("rtmps://")) {
            return url.to_string();
        }

        let without_query = url.split('?').next().unwrap_or(url);
        let trimmed = without_query.trim_end_matches('/');

        let (scheme, rest) = match trimmed.split_once("://") {
            Some(parts) => parts,
            None => return url.to_string(),
        };

        let mut host_and_path = rest.splitn(2, '/');
        let host = match host_and_path.next() {
            Some(value) if !value.is_empty() => value,
            _ => return url.to_string(),
        };
        let host = if host == "0.0.0.0" {
            "127.0.0.1".to_string()
        } else if let Some(port) = host.strip_prefix("0.0.0.0:") {
            format!("127.0.0.1:{port}")
        } else {
            host.to_string()
        };

        let path = host_and_path.next().unwrap_or("");
        let app = path.split('/').find(|segment| !segment.is_empty());

        let base_url = if let Some(app) = app {
            format!("{scheme}://{host}/{app}")
        } else {
            format!("{scheme}://{host}")
        };

        base_url
    }

    fn double_bitrate_value(bitrate: &str) -> Option<String> {
        let trimmed = bitrate.trim();
        if trimmed.is_empty() {
            return None;
        }

        let split_at = trimmed
            .find(|c: char| !c.is_ascii_digit() && c != '.')
            .unwrap_or(trimmed.len());
        let (value_str, suffix) = trimmed.split_at(split_at);
        if value_str.is_empty() {
            return None;
        }

        let value: f64 = value_str.parse().ok()?;
        let doubled = value * 2.0;
        let formatted = format!("{doubled}");
        Some(format!("{formatted}{suffix}"))
    }

    fn append_cbr_args(args: &mut Vec<String>, encoder: &str, bitrate: &str) {
        let bufsize = Self::double_bitrate_value(bitrate)
            .unwrap_or_else(|| bitrate.to_string());

        args.push("-minrate".to_string()); args.push(bitrate.to_string());
        args.push("-maxrate".to_string()); args.push(bitrate.to_string());
        args.push("-bufsize".to_string()); args.push(bufsize);

        if encoder.contains("nvenc") || encoder.contains("qsv") || encoder.contains("amf") {
            args.push("-rc".to_string()); args.push("cbr".to_string());
        }

        if encoder == "libx264" {
            args.push("-x264-params".to_string());
            args.push("nal-hrd=cbr:force-cfr=1".to_string());
        } else if encoder == "libx265" {
            args.push("-x265-params".to_string());
            args.push("nal-hrd=cbr".to_string());
        }
    }

    fn map_nvenc_preset(preset: &str) -> String {
        let normalized = preset.trim().to_ascii_lowercase();
        if normalized.is_empty() {
            return "p4".to_string();
        }

        match normalized.as_str() {
            "p1" | "p2" | "p3" | "p4" | "p5" | "p6" | "p7" | "default" | "slow" | "medium"
            | "fast" | "hp" | "hq" | "bd" | "ll" | "llhq" | "llhp" | "lossless"
            | "losslesshp" => normalized,
            "ultrafast" => "p1".to_string(),
            "superfast" => "p2".to_string(),
            "veryfast" => "p3".to_string(),
            "faster" => "p4".to_string(),
            "slower" => "p6".to_string(),
            "veryslow" => "p7".to_string(),
            "quality" => "p7".to_string(),
            "balanced" => "p4".to_string(),
            "performance" => "p2".to_string(),
            "low_latency" | "low-latency" | "lowlatency" => "p1".to_string(),
            _ => "p4".to_string(),
        }
    }


    /// Build FFmpeg arguments for an output group
    ///
    /// Groups read from the shared TCP relay so they can restart independently.
    fn build_args(&self, group: &OutputGroup) -> Vec<String> {
        // Determine if we should use stream copy (passthrough mode)
        // When both video and audio codecs are set to "copy", FFmpeg acts as a pure
        // RTMP relay server, accepting the incoming stream and forwarding it to outputs
        // without re-encoding. This is the default behavior and most efficient mode.
        // Use case-insensitive comparison to handle "Copy", "COPY", etc.
        let use_stream_copy = group.video.codec.eq_ignore_ascii_case("copy")
            && group.audio.codec.eq_ignore_ascii_case("copy");

        let mut args = vec![
            "-i".to_string(),
            Self::relay_input_url_for_group(&group.id),
        ];

        if use_stream_copy {
            args.push("-c:v".to_string()); args.push("copy".to_string());
            args.push("-c:a".to_string()); args.push("copy".to_string());
        } else {
            // Video settings
            args.push("-c:v".to_string()); args.push(group.video.codec.clone());
            args.push("-s".to_string()); args.push(group.video.resolution());
            args.push("-b:v".to_string()); args.push(group.video.bitrate.clone());
            // Add CBR enforcement for consistent streaming bitrate
            Self::append_cbr_args(&mut args, &group.video.codec, &group.video.bitrate);
            args.push("-r".to_string()); args.push(group.video.fps.to_string());
            // Audio settings
            args.push("-c:a".to_string()); args.push(group.audio.codec.clone());
            args.push("-b:a".to_string()); args.push(group.audio.bitrate.clone());
            args.push("-ac".to_string()); args.push(group.audio.channels.to_string());
            args.push("-ar".to_string()); args.push(group.audio.sample_rate.to_string());
            // Add video encoder preset if specified
            if let Some(preset) = &group.video.preset {
                let encoder = group.video.codec.as_str();
                if encoder.contains("amf") {
                    let mut amf_quality: Option<&str> = None;
                    let mut amf_usage: Option<&str> = None;
                    match preset.as_str() {
                        "quality" => amf_quality = Some("quality"),
                        "balanced" => amf_quality = Some("balanced"),
                        "speed" => amf_quality = Some("speed"),
                        "performance" | "fast" | "faster" | "veryfast" | "superfast" | "ultrafast" => {
                            amf_quality = Some("speed");
                        }
                        "medium" => amf_quality = Some("balanced"),
                        "slow" | "slower" | "veryslow" => amf_quality = Some("quality"),
                        "low_latency" | "low-latency" | "lowLatency" => {
                            amf_quality = Some("speed");
                            amf_usage = Some("lowlatency");
                        }
                        _ => {}
                    }
                    if let Some(quality) = amf_quality {
                        args.push("-quality".to_string()); args.push(quality.to_string());
                    }
                    if let Some(usage) = amf_usage {
                        args.push("-usage".to_string()); args.push(usage.to_string());
                    }
                } else if encoder.contains("nvenc") {
                    let ffmpeg_preset = Self::map_nvenc_preset(preset);
                    args.push("-preset".to_string()); args.push(ffmpeg_preset);
                } else if encoder.contains("qsv") {
                    // Intel QuickSync (QSV) accepts text presets that map to TargetUsage (TU1-TU7):
                    // veryfast=TU7, faster=TU6, fast=TU5, medium=TU4, slow=TU3, slower=TU2, veryslow=TU1
                    let ffmpeg_preset = match preset.as_str() {
                        "quality" => "slow",
                        "balanced" => "medium",
                        "performance" => "fast",
                        "low_latency" | "low-latency" | "lowLatency" => "veryfast",
                        _ => preset.as_str(),
                    };
                    args.push("-preset".to_string()); args.push(ffmpeg_preset.to_string());
                    // QSV-specific parameters for streaming compatibility (based on OBS defaults)
                    args.push("-bf".to_string()); args.push("2".to_string());           // B-frames
                    args.push("-look_ahead".to_string()); args.push("1".to_string());   // Enable look-ahead
                    args.push("-look_ahead_depth".to_string()); args.push("30".to_string());
                    args.push("-async_depth".to_string()); args.push("4".to_string());  // Pipeline depth
                } else {
                    let supports_preset = encoder == "libx264"
                        || encoder == "libx265";
                    if supports_preset {
                        let ffmpeg_preset = match preset.as_str() {
                            "quality" => "slow",
                            "balanced" => "medium",
                            "performance" => "fast",
                            "low_latency" | "low-latency" | "lowLatency" => "ultrafast",
                            _ => preset.as_str(),
                        };
                        args.push("-preset".to_string()); args.push(ffmpeg_preset.to_string());
                    }
                }
            }
            // Add H.264 profile if specified
            if let Some(profile) = &group.video.profile {
                args.push("-profile:v".to_string()); args.push(profile.clone());
            }

            // Add H.264 level for QSV to ensure streaming platform compatibility
            // Level 4.2 supports 1080p60, level 5.1 supports 1440p60 and 4K30
            if group.video.codec.contains("qsv") && group.video.codec.contains("264") {
                let level = if group.video.height > 1080 || (group.video.height == 1080 && group.video.fps > 60) {
                    "5.1"
                } else if group.video.height >= 1080 && group.video.fps >= 60 {
                    "4.2"
                } else {
                    "4.1"
                };
                args.push("-level".to_string()); args.push(level.to_string());
            }

            // QSV-specific: Ensure proper pixel format and extradata handling for RTMP/FLV
            if group.video.codec.contains("qsv") {
                // NV12 is the native QSV format and ensures proper color space handling
                args.push("-pix_fmt".to_string()); args.push("nv12".to_string());
            }

            if let Some(interval_seconds) = group.video.keyframe_interval_seconds {
                if interval_seconds > 0 && group.video.fps > 0 {
                    let gop_size = group.video.fps.saturating_mul(interval_seconds);
                    if gop_size > 0 {
                        args.push("-g".to_string()); args.push(gop_size.to_string());

                        if group.video.codec == "libx264" || group.video.codec == "libx265" {
                            args.push("-keyint_min".to_string()); args.push(gop_size.to_string());
                            args.push("-sc_threshold".to_string()); args.push("0".to_string());
                        }

                        args.push("-force_key_frames".to_string());
                        args.push(format!("expr:gte(t,n_forced*{interval_seconds})"));
                    }
                }
            }
        }

        if group.container.format == "flv" {
            let force_flv_video_tag = use_stream_copy || group.video.codec.contains("264");
            if force_flv_video_tag {
                args.push("-tag:v".to_string());
                args.push("7".to_string());
            }

            let force_flv_audio_tag = use_stream_copy || group.audio.codec.contains("aac");
            if force_flv_audio_tag {
                args.push("-tag:a".to_string());
                args.push("10".to_string());
            }

            if use_stream_copy {
                args.push("-bsf:a".to_string());
                args.push("aac_adtstoasc".to_string());
            }
        }

        // Always map video and audio from input 0
        args.push("-map".to_string()); args.push("0:v".to_string());
        args.push("-map".to_string()); args.push("0:a".to_string());

        // Progress output for stats parsing
        args.push("-progress".to_string()); args.push("pipe:2".to_string());
        args.push("-stats".to_string());

        // Add output targets (skip disabled ones)
        let disabled = self.disabled_targets.lock().unwrap_or_else(|e| {
            log::warn!("Disabled targets mutex poisoned (build_args), recovering: {e}");
            e.into_inner()
        });
        let mut target_outputs: Vec<String> = Vec::new();
        for target in &group.stream_targets {
            // Skip targets that have been disabled via toggle_target
            if disabled.contains(&target.id) {
                continue;
            }

            let normalized_url = Self::normalize_rtmp_url(&target.url);
            let normalized_url = self.platform_registry.normalize_url(&target.service, &normalized_url);
            let resolved_key = Self::resolve_stream_key(&target.stream_key);
            let full_url = self.platform_registry.build_url_with_key(&target.service, &normalized_url, &resolved_key);
            target_outputs.push(full_url);
        }

        if target_outputs.is_empty() {
            return args;
        }

        let meter_output = Self::meter_output_url_for_group(&group.id);

        // For QSV encoders outputting to FLV, we need to add the dump_extra bitstream filter
        // to ensure SPS/PPS NAL units are written to each output stream (required for RTMP/Twitch)
        let needs_dump_extra = !use_stream_copy
            && group.video.codec.contains("qsv")
            && group.container.format == "flv";

        let mut tee_outputs: Vec<String> = Vec::new();
        if target_outputs.len() == 1 {
            let output = &target_outputs[0];
            if needs_dump_extra {
                tee_outputs.push(format!("[f={}:bsf/v=dump_extra]{output}", group.container.format));
            } else {
                tee_outputs.push(format!("[f={}]{output}", group.container.format));
            }
        } else {
            tee_outputs.extend(
                target_outputs
                    .iter()
                    .map(|output| {
                        if needs_dump_extra {
                            format!("[f={}:onfail=ignore:bsf/v=dump_extra]{output}", group.container.format)
                        } else {
                            format!("[f={}:onfail=ignore]{output}", group.container.format)
                        }
                    })
            );
        }
        tee_outputs.push(format!("[f=mpegts:onfail=ignore]{meter_output}"));

        args.push("-f".to_string());
        args.push("tee".to_string());
        args.push(tee_outputs.join("|"));

        args
    }
}

impl Default for FFmpegHandler {
    fn default() -> Self {
        Self::new()
    }
}
