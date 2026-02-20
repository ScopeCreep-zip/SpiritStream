// FFmpegHandler Service
// Manages FFmpeg processes for streaming with real-time stats

use std::collections::HashSet;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use dashmap::{DashMap, DashSet};
use crate::services::EventSink;
use crate::models::OutputGroup;
use crate::services::PlatformRegistry;
use crate::services::PowerAssertion;

mod args_builder;
mod ffmpeg_relay;
mod ffmpeg_stats;
mod native;

use ffmpeg_relay::FFmpegRelay;
use ffmpeg_stats::ProcessInfo;

pub use native::{NativeVideoConfig, NativeAudioConfig, NativeCaptureHandle};

use crate::services::process_util::{configure_hidden_window, kill_and_wait};

/// Cached configuration for restarting groups when relay output set changes
struct ActiveGroupConfig {
    group: OutputGroup,
    incoming_url: String,
}

/// Manages FFmpeg streaming processes
pub struct FFmpegHandler {
    ffmpeg_path: String,
    processes: Arc<DashMap<String, ProcessInfo>>,
    stopping_groups: Arc<DashSet<String>>,
    disabled_targets: Arc<DashSet<String>>,
    relay: FFmpegRelay,
    active_groups: Arc<DashMap<String, ActiveGroupConfig>>,
    /// Platform registry for URL normalization and redaction
    platform_registry: PlatformRegistry,
    /// Prevents system idle sleep while streaming is active
    power_assertion: std::sync::Mutex<Option<PowerAssertion>>,
}

impl FFmpegHandler {
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
            relay: FFmpegRelay::new(ffmpeg_path.clone()),
            ffmpeg_path,
            processes: Arc::new(DashMap::new()),
            stopping_groups: Arc::new(DashSet::new()),
            disabled_targets: Arc::new(DashSet::new()),
            active_groups: Arc::new(DashMap::new()),
            platform_registry: PlatformRegistry::new(),
            power_assertion: std::sync::Mutex::new(None),
        }
    }

    /// Create a new FFmpegHandler (legacy, without bundled FFmpeg support)
    pub fn new() -> Self {
        let ffmpeg_path = Self::find_ffmpeg();
        Self {
            relay: FFmpegRelay::new(ffmpeg_path.clone()),
            ffmpeg_path,
            processes: Arc::new(DashMap::new()),
            stopping_groups: Arc::new(DashSet::new()),
            disabled_targets: Arc::new(DashSet::new()),
            active_groups: Arc::new(DashMap::new()),
            platform_registry: PlatformRegistry::new(),
            power_assertion: std::sync::Mutex::new(None),
        }
    }

    // ========================================================================
    // URL and Key Helpers
    // ========================================================================

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

    // ========================================================================
    // Sanitization / Redaction
    // ========================================================================

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
    pub(crate) fn sanitize_arg_static(arg: &str) -> String {
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

    // ========================================================================
    // FFmpeg Discovery
    // ========================================================================

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

    // ========================================================================
    // Active Group State Management
    // ========================================================================

    fn record_active_group(&self, group: &OutputGroup, incoming_url: &str) -> Result<(), String> {
        // Check if any existing group has a different incoming URL
        for entry in self.active_groups.iter() {
            if entry.value().incoming_url != incoming_url {
                return Err("Incoming URL differs from active groups".to_string());
            }
        }

        self.active_groups.insert(group.id.clone(), ActiveGroupConfig {
            group: group.clone(),
            incoming_url: incoming_url.to_string(),
        });

        Ok(())
    }

    fn remove_active_group(&self, group_id: &str) {
        self.active_groups.remove(group_id);
    }

    fn collect_active_group_ids(&self) -> Result<HashSet<String>, String> {
        Ok(self.active_groups.iter().map(|entry| entry.key().clone()).collect())
    }

    fn resolve_active_incoming_url(&self) -> Result<String, String> {
        let mut incoming_url: Option<String> = None;
        for entry in self.active_groups.iter() {
            let cfg = entry.value();
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
        self.processes.get(group_id).map(|entry| entry.child.id())
    }

    fn relay_needs_restart(&self, desired_group_ids: &HashSet<String>) -> Result<bool, String> {
        let relay_guard = self.relay.relay().lock()
            .map_err(|e| format!("Lock poisoned: {e}"))?;
        if let Some(relay) = relay_guard.as_ref() {
            if !relay.output_groups.is_superset(desired_group_ids) {
                return Ok(!self.processes.is_empty());
            }
        }
        Ok(false)
    }

    // ========================================================================
    // Process Lifecycle
    // ========================================================================

    fn stop_group_for_restart(&self, group_id: &str) -> Result<(), String> {
        self.stopping_groups.insert(group_id.to_string());
        let removed = self.processes.remove(group_id);

        if let Some((_, mut info)) = removed {
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
        configure_hidden_window(&mut cmd);
        let mut child = cmd.spawn()
            .map_err(|e| format!("Failed to start FFmpeg: {e}"))?;

        let pid = child.id();
        let group_id = group.id.clone();

        let stderr = child.stderr.take()
            .ok_or_else(|| "Failed to capture FFmpeg stderr".to_string())?;

        self.processes.insert(group_id.clone(), ProcessInfo {
            child,
            start_time: Instant::now(),
            group_id: group_id.clone(),
        });

        self.relay.relay_refcount().fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        let event_sink_clone = Arc::clone(&event_sink);
        let processes_clone = Arc::clone(&self.processes);
        let meter_bytes = ffmpeg_stats::start_bitrate_meter(&group_id, Arc::clone(&processes_clone));
        let relay_clone = Arc::clone(self.relay.relay());
        let stopping_clone = Arc::clone(&self.stopping_groups);
        let relay_refcount_clone = Arc::clone(self.relay.relay_refcount());
        let group_id_clone = group_id.clone();

        thread::spawn(move || {
            ffmpeg_stats::stats_reader(
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
        self.relay.stop();

        // Collect active groups to avoid holding DashMap reference during start_group_process
        let active_snapshot: Vec<(String, OutputGroup)> = self.active_groups.iter()
            .map(|entry| (entry.key().clone(), entry.value().group.clone()))
            .collect();
        let mut requested_pid: Option<u32> = None;
        for (group_id, group) in &active_snapshot {
            let pid = self.start_group_process(group, Arc::clone(&event_sink))?;
            if group_id == requested_group_id {
                requested_pid = Some(pid);
            }
        }

        self.relay.ensure_running(&incoming_url, &desired_group_ids)?;

        requested_pid.ok_or_else(|| "Requested group not started".to_string())
    }

    /// Acquire power assertion to prevent system sleep during streaming
    fn acquire_power_assertion(&self) {
        if let Ok(mut guard) = self.power_assertion.lock() {
            if guard.is_none() {
                match PowerAssertion::prevent_idle_sleep("SpiritStream streaming active") {
                    Ok(assertion) => *guard = Some(assertion),
                    Err(e) => log::warn!("Failed to acquire power assertion: {}", e),
                }
            }
        }
    }

    /// Release power assertion when all streams stop
    fn release_power_assertion(&self) {
        if let Ok(mut guard) = self.power_assertion.lock() {
            if let Some(_assertion) = guard.take() {
                log::info!("Released streaming power assertion");
            }
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

        kill_and_wait(child);
    }

    // ========================================================================
    // Public API
    // ========================================================================

    /// Start streaming for an output group with stats monitoring
    pub fn start(
        &self,
        group: &OutputGroup,
        incoming_url: &str,
        event_sink: Arc<dyn EventSink>,
    ) -> Result<u32, String> {
        // Validate BEFORE acquiring power assertion to avoid leaking on error
        self.record_active_group(group, incoming_url)?;

        if let Some(pid) = self.get_group_pid(&group.id) {
            return Ok(pid);
        }

        // Acquire power assertion only after validation succeeds
        self.acquire_power_assertion();

        let desired_group_ids = self.collect_active_group_ids()?;
        if self.relay_needs_restart(&desired_group_ids)? {
            return self.restart_relay_with_groups(&group.id, event_sink);
        }

        if !self.relay.is_active()? {
            let pid = self.start_group_process(group, Arc::clone(&event_sink))?;
            self.relay.ensure_running(incoming_url, &desired_group_ids)?;
            return Ok(pid);
        }

        self.relay.ensure_running(incoming_url, &desired_group_ids)?;
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

        // Validate groups BEFORE acquiring power assertion
        self.active_groups.clear();

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

        // Acquire power assertion only after validation succeeds
        self.acquire_power_assertion();

        let mut pids = Vec::with_capacity(start_groups.len());
        for group in &start_groups {
            let pid = self.start_group_process(group, Arc::clone(&event_sink))?;
            pids.push(pid);
        }

        self.relay.ensure_running(incoming_url, &desired_group_ids)?;

        Ok(pids)
    }

    /// Stop streaming for an output group
    pub fn stop(&self, group_id: &str) -> Result<(), String> {
        self.remove_active_group(group_id);
        self.stopping_groups.insert(group_id.to_string());
        let removed = self.processes.remove(group_id);
        let should_stop_relay = self.processes.is_empty();

        if let Some((_, mut info)) = removed {
            self.stop_child(&mut info.child);
        }
        if should_stop_relay {
            self.relay.stop();
            self.release_power_assertion();
        }
        Ok(())
    }

    /// Stop all active streams
    pub fn stop_all(&self) -> Result<(), String> {
        self.active_groups.clear();
        // Collect all keys, then remove and stop each process
        let group_ids: Vec<String> = self.processes.iter()
            .map(|entry| entry.key().to_string())
            .collect::<Vec<_>>();
        for group_id in group_ids {
            if let Some((_, mut info)) = self.processes.remove(&group_id) {
                self.stopping_groups.insert(group_id);
                self.stop_child(&mut info.child);
            }
        }
        self.relay.stop();
        self.release_power_assertion();
        Ok(())
    }

    /// Get active stream count
    pub fn active_count(&self) -> usize {
        self.processes.len()
    }

    /// Check if a group is streaming
    pub fn is_streaming(&self, group_id: &str) -> bool {
        self.processes.contains_key(group_id)
    }

    /// Get list of active stream group IDs
    pub fn get_active_group_ids(&self) -> Vec<String> {
        self.processes.iter()
            .map(|entry| entry.group_id.clone())
            .collect::<Vec<_>>()
    }

    /// Get the FFmpeg path being used by this handler
    pub fn get_ffmpeg_path(&self) -> String {
        self.ffmpeg_path.clone()
    }

    /// Enable a specific stream target (removes from disabled set)
    pub fn enable_target(&self, target_id: &str) {
        self.disabled_targets.remove(target_id);
    }

    /// Disable a specific stream target (adds to disabled set)
    pub fn disable_target(&self, target_id: &str) {
        self.disabled_targets.insert(target_id.to_string());
    }

    /// Check if a target is currently disabled
    pub fn is_target_disabled(&self, target_id: &str) -> bool {
        self.disabled_targets.contains(target_id)
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
}

impl Default for FFmpegHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for FFmpegHandler {
    fn drop(&mut self) {
        // Stop all active streams and relay on drop
        let _ = self.stop_all();
    }
}
