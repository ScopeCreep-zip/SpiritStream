// FFmpegHandler Service
// Manages FFmpeg processes for streaming with real-time stats

use std::collections::{HashMap, HashSet, VecDeque};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};
use crate::models::{OutputGroup, Platform, StreamStats};
use crate::services::PlatformRegistry;

/// Process info for tracking active streams
struct ProcessInfo {
    child: Child,
    start_time: Instant,
    group_id: String,
}

/// FFmpeg relay process for shared ingest
struct RelayProcess {
    child: Child,
    incoming_url: String,
}

/// Manages FFmpeg streaming processes
pub struct FFmpegHandler {
    ffmpeg_path: String,
    processes: Arc<Mutex<HashMap<String, ProcessInfo>>>,
    stopping_groups: Arc<Mutex<HashSet<String>>>,
    disabled_targets: Arc<Mutex<HashSet<String>>>,
    relay: Arc<Mutex<Option<RelayProcess>>>,
    /// Reference count for active groups using the relay
    /// Prevents race condition where relay stops while groups are still active
    relay_refcount: Arc<AtomicUsize>,
    /// Platform registry for URL normalization and redaction
    platform_registry: PlatformRegistry,
}

impl FFmpegHandler {
    // Multicast relay so multiple group processes can receive the same stream
    const RELAY_UDP_OUT: &'static str = "udp://239.255.0.1:5000?ttl=1&pkt_size=1316";
    const RELAY_UDP_IN: &'static str =
        "udp://@239.255.0.1:5000?reuse=1&fifo_size=20000&overrun_nonfatal=1";

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

    /// Redact stream key from URL using platform-specific logic
    fn redact_url(&self, platform: &Platform, url: &str) -> String {
        self.platform_registry.redact_url(platform, url)
    }

    /// Sanitize a single FFmpeg argument (redact stream keys from RTMP URLs)
    /// Uses generic platform-agnostic redaction
    fn sanitize_arg(&self, arg: &str) -> String {
        if !(arg.contains("rtmp://") || arg.contains("rtmps://")) {
            return arg.to_string();
        }

        let mut parts = Vec::new();
        for segment in arg.split('|') {
            let redacted = if let Some(pos) = segment.find("rtmp://") {
                let prefix = &segment[..pos];
                let url_start = pos;
                // Find the end of the URL (space or end of string)
                let url_end = segment[url_start..].find(' ').map(|i| url_start + i).unwrap_or(segment.len());
                let url = &segment[url_start..url_end];
                let suffix = &segment[url_end..];
                // Use generic redaction for unknown platforms
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

    /// Sanitize all FFmpeg arguments (redact stream keys)
    fn sanitize_ffmpeg_args(&self, args: &[String]) -> Vec<String> {
        args.iter().map(|arg| self.sanitize_arg(arg)).collect()
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

    /// Start streaming for an output group with stats monitoring
    pub fn start<R: tauri::Runtime>(
        &self,
        group: &OutputGroup,
        incoming_url: &str,
        app_handle: &AppHandle<R>,
    ) -> Result<u32, String> {
        self.ensure_relay_running(incoming_url)?;

        let args = self.build_args(group);
        let sanitized = self.sanitize_ffmpeg_args(&args);
        log::info!(
            "Starting FFmpeg group {}: {} {}",
            group.id,
            self.ffmpeg_path,
            sanitized.join(" ")
        );

        let mut child = Command::new(&self.ffmpeg_path)
            .args(&args)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("Failed to start FFmpeg: {e}"))?;

        let pid = child.id();
        let group_id = group.id.clone();

        // Take stderr for stats parsing
        let stderr = child.stderr.take()
            .ok_or_else(|| "Failed to capture FFmpeg stderr".to_string())?;

        // Store process info
        {
            let mut processes = self.processes.lock()
                .map_err(|e| format!("Lock poisoned: {e}"))?;
            processes.insert(group_id.clone(), ProcessInfo {
                child,
                start_time: Instant::now(),
                group_id: group_id.clone(),
            });
        }

        // Increment relay reference count (prevents premature relay shutdown)
        self.relay_refcount.fetch_add(1, Ordering::SeqCst);

        // Spawn background thread to read stderr and emit stats
        let app_handle_clone = app_handle.clone();
        let processes_clone = Arc::clone(&self.processes);
        let relay_clone = Arc::clone(&self.relay);
        let stopping_clone = Arc::clone(&self.stopping_groups);
        let relay_refcount_clone = Arc::clone(&self.relay_refcount);
        let group_id_clone = group_id.clone();

        thread::spawn(move || {
            Self::stats_reader(
                stderr,
                group_id_clone,
                app_handle_clone,
                processes_clone,
                stopping_clone,
                relay_clone,
                relay_refcount_clone,
            );
        });

        Ok(pid)
    }

    /// Background thread that reads FFmpeg stderr and emits stats events
    fn stats_reader<R: tauri::Runtime>(
        stderr: std::process::ChildStderr,
        group_id: String,
        app_handle: AppHandle<R>,
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

        for line_result in reader.lines() {
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

            if let Ok(line) = line_result {
                let sanitized_line = Self::sanitize_arg_static(&line);
                if recent_lines.len() == 40 {
                    recent_lines.pop_front();
                }
                recent_lines.push_back(sanitized_line.clone());

                // Parse stats from FFmpeg output
                if stats.parse_line(&line) {
                    // Emit stats at most every second
                    if last_emit.elapsed() >= emit_interval {
                        // Add uptime from process start
                        if let Ok(procs) = processes.lock() {
                            if let Some(info) = procs.get(&group_id) {
                                stats.time = info.start_time.elapsed().as_secs_f64();
                            }
                        }

                        // Emit event
                        let _ = app_handle.emit("stream_stats", stats.clone());
                        last_emit = Instant::now();
                    }
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
        }

        // Decrement relay reference count when group ends
        relay_refcount.fetch_sub(1, Ordering::SeqCst);

        // Process ended - check if it was intentional or a crash
        if was_intentionally_stopped {
            if let Ok(mut stopping) = stopping_groups.lock() {
                stopping.remove(&group_id);
            }
            // Intentional stop via stop() - process already removed
            let _ = app_handle.emit("stream_ended", &group_id);
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
                    let _ = app_handle.emit("stream_ended", &group_id);
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
                let _ = app_handle.emit("stream_error", serde_json::json!({
                    "groupId": group_id,
                    "error": error
                }));
            } else {
                // Clean exit (input ended)
                let _ = app_handle.emit("stream_ended", &group_id);
            }
        }

        // Check relay refcount and stop relay if no more groups are using it
        // Use atomic load to avoid race condition where multiple groups finish simultaneously
        let should_stop_relay = relay_refcount.load(Ordering::SeqCst) == 0;
        if should_stop_relay {
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
            log::warn!("Disabled targets mutex poisoned (enable_target), recovering: {}", e);
            e.into_inner()
        });
        disabled.remove(target_id);
    }

    /// Disable a specific stream target (adds to disabled set)
    pub fn disable_target(&self, target_id: &str) {
        let mut disabled = self.disabled_targets.lock().unwrap_or_else(|e| {
            log::warn!("Disabled targets mutex poisoned (disable_target), recovering: {}", e);
            e.into_inner()
        });
        disabled.insert(target_id.to_string());
    }

    /// Check if a target is currently disabled
    pub fn is_target_disabled(&self, target_id: &str) -> bool {
        let disabled = self.disabled_targets.lock().unwrap_or_else(|e| {
            log::warn!("Disabled targets mutex poisoned (is_target_disabled), recovering: {}", e);
            e.into_inner()
        });
        disabled.contains(target_id)
    }

    /// Ensure relay process is running for shared ingest
    fn ensure_relay_running(&self, incoming_url: &str) -> Result<(), String> {
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
            return Ok(());
        }

        let args = self.build_relay_args(incoming_url);
        let sanitized = self.sanitize_ffmpeg_args(&args);
        log::info!(
            "Starting FFmpeg relay: {} {}",
            self.ffmpeg_path,
            sanitized.join(" ")
        );
        let mut child = Command::new(&self.ffmpeg_path)
            .args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("Failed to start FFmpeg relay: {e}"))?;

        if let Some(stderr) = child.stderr.take() {
            thread::spawn(move || {
                let reader = BufReader::new(stderr);
                for line_result in reader.lines() {
                    if let Ok(line) = line_result {
                        if line.contains("[error]") || line.contains("[warning]") {
                            log::warn!("[FFmpeg:relay] {line}");
                        }
                    }
                }
            });
        }

        *relay_guard = Some(RelayProcess {
            child,
            incoming_url: incoming_url.to_string(),
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
    pub fn restart_group<R: tauri::Runtime>(
        &self,
        group_id: &str,
        group: &OutputGroup,
        incoming_url: &str,
        app_handle: &AppHandle<R>,
    ) -> Result<u32, String> {
        // Stop the group if it's running
        if self.is_streaming(group_id) {
            self.stop(group_id)?;
        }

        // Start with updated target list (disabled targets will be filtered out)
        self.start(group, incoming_url, app_handle)
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
    fn build_relay_args(&self, incoming_url: &str) -> Vec<String> {
        let mut args = Vec::new();

        args.push("-listen".to_string());
        args.push("1".to_string());
        args.push("-i".to_string());
        args.push(incoming_url.to_string());
        args.push("-c:v".to_string());
        args.push("copy".to_string());
        args.push("-c:a".to_string());
        args.push("copy".to_string());
        args.push("-f".to_string());
        args.push("mpegts".to_string());
        args.push(Self::RELAY_UDP_OUT.to_string());

        args
    }

    /// Build FFmpeg arguments for an output group
    ///
    /// Groups read from the shared UDP relay so they can restart independently.
    fn build_args(&self, group: &OutputGroup) -> Vec<String> {
        let mut args = Vec::new();

        // Input configuration (shared relay)
        args.push("-fflags".to_string());
        args.push("nobuffer".to_string());
        args.push("-flags".to_string());
        args.push("low_delay".to_string());
        args.push("-i".to_string());
        args.push(Self::RELAY_UDP_IN.to_string());

        // Determine if we should use stream copy (passthrough mode)
        // When both video and audio codecs are set to "copy", FFmpeg acts as a pure
        // RTMP relay server, accepting the incoming stream and forwarding it to outputs
        // without re-encoding. This is the default behavior and most efficient mode.
        // Use case-insensitive comparison to handle "Copy", "COPY", etc.
        let use_stream_copy = group.video.codec.eq_ignore_ascii_case("copy")
            && group.audio.codec.eq_ignore_ascii_case("copy");

        if use_stream_copy {
            args.push("-c:v".to_string()); args.push("copy".to_string());
            args.push("-c:a".to_string()); args.push("copy".to_string());
        } else {
            // Video settings
            args.push("-c:v".to_string()); args.push(group.video.codec.clone());
            args.push("-s".to_string()); args.push(group.video.resolution());
            args.push("-b:v".to_string()); args.push(group.video.bitrate.clone());
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
                } else {
                    let supports_preset = encoder == "libx264"
                        || encoder == "libx265"
                        || encoder.contains("nvenc");
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
                        args.push(format!("expr:gte(t,n_forced*{})", interval_seconds));
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
            log::warn!("Disabled targets mutex poisoned (build_args), recovering: {}", e);
            e.into_inner()
        });
        let mut outputs: Vec<String> = Vec::new();
        for target in &group.stream_targets {
            // Skip targets that have been disabled via toggle_target
            if disabled.contains(&target.id) {
                continue;
            }

            let normalized_url = Self::normalize_rtmp_url(&target.url);
            let normalized_url = self.platform_registry.normalize_url(&target.service, &normalized_url);
            let resolved_key = Self::resolve_stream_key(&target.stream_key);
            let full_url = self.platform_registry.build_url_with_key(&target.service, &normalized_url, &resolved_key);
            outputs.push(full_url);
        }

        if outputs.len() <= 1 {
            if let Some(output) = outputs.first() {
                args.push("-f".to_string());
                args.push(group.container.format.clone());
                args.push(output.clone());
            }
        } else {
            let tee_outputs = outputs
                .iter()
                .map(|output| format!("[f={}:onfail=ignore]{output}", group.container.format))
                .collect::<Vec<_>>()
                .join("|");
            args.push("-f".to_string());
            args.push("tee".to_string());
            args.push(tee_outputs);
        }

        args
    }
}

impl Default for FFmpegHandler {
    fn default() -> Self {
        Self::new()
    }
}
