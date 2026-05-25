use std::collections::HashSet;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crate::errors::CoreError;
use crate::models::OutputGroup;
use crate::services::{emit_event, EventSink};

use super::{ffmpeg_internal, lock_poisoned, ReconnectionState};

impl super::FFmpegHandler {
    /// Start streaming for an output group with stats monitoring.
    pub fn start(
        &self,
        group: &OutputGroup,
        incoming_url: &str,
        event_sink: Arc<dyn EventSink>,
    ) -> Result<u32, CoreError> {
        // Authoritative encoding-config validation. Frontend's
        // `POST /streams/validate` is decorative — this is the gate that
        // refuses a malformed config before any FFmpeg process is spawned.
        Self::validate_output_group(group)?;

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

    /// Start streaming for multiple output groups in one batch.
    pub fn start_all(
        &self,
        groups: &[OutputGroup],
        incoming_url: &str,
        event_sink: Arc<dyn EventSink>,
    ) -> Result<Vec<u32>, CoreError> {
        // Validate every group up-front so we don't start half the stream
        // pipeline before a later group fails its bounds check. Collect
        // every failure into one InvalidStreamConfig with a `reasons` array.
        let mut all_issues = Vec::new();
        for (gi, group) in groups.iter().enumerate() {
            if group.stream_targets.is_empty() {
                continue;
            }
            let issues = Self::collect_group_issues(group, &format!("/groups/{gi}"));
            all_issues.extend(issues);
        }
        if !all_issues.is_empty() {
            return Err(CoreError::InvalidStreamConfig {
                reasons: all_issues,
            });
        }

        if self.active_count() > 0 {
            return Err(ffmpeg_internal("Streams already running"));
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
            return Err(ffmpeg_internal("At least one stream target is required"));
        }

        let mut pids = Vec::with_capacity(start_groups.len());
        for group in &start_groups {
            let pid = self.start_group_process(group, Arc::clone(&event_sink))?;
            pids.push(pid);
        }

        self.ensure_relay_running(incoming_url, &desired_group_ids)?;

        // SpiritStream→OBS cascade. The trigger checks the active
        // profile's `obs.direction` server-side; this call is a no-op
        // when the trigger handle isn't installed (tests / CLI) or
        // when direction forbids it.
        self.fire_obs_trigger(true);

        Ok(pids)
    }

    /// Stop streaming for an output group.
    pub fn stop(&self, group_id: &str) -> Result<(), CoreError> {
        self.remove_active_group(group_id);
        if let Ok(mut stopping) = self.stopping_groups.lock() {
            stopping.insert(group_id.to_string());
        }
        let (removed, should_stop_relay) = {
            let mut processes = self.processes.lock().map_err(lock_poisoned)?;
            let removed = processes.remove(group_id);
            let should_stop_relay = processes.is_empty();
            (removed, should_stop_relay)
        };

        if let Some(mut info) = removed {
            self.stop_child(&mut info.child);
            self.free_port_offset(group_id);
        }

        // Only stop relay when ALL groups are stopped. Restarting the
        // relay during a single-group stop would interrupt input for
        // remaining groups and cause them to fail.
        if should_stop_relay {
            self.stop_relay();
        }

        Ok(())
    }

    /// Stop all active streams.
    pub fn stop_all(&self) -> Result<(), CoreError> {
        if let Ok(mut active) = self.active_groups.lock() {
            active.clear();
        }
        let mut processes = self.processes.lock().map_err(lock_poisoned)?;
        let mut stopping = self.stopping_groups.lock().map_err(lock_poisoned)?;
        for (group_id, mut info) in processes.drain() {
            stopping.insert(group_id);
            self.stop_child(&mut info.child);
        }
        self.stop_relay();

        // Clear all port assignments since all groups are stopped.
        if let Ok(mut assignments) = self.port_assignments.lock() {
            assignments.clear();
            self.next_port_offset.store(0, Ordering::SeqCst);
            log::debug!("All groups stopped, cleared all port assignments");
        }

        // SpiritStream→OBS cascade — mirror of `start_all`.
        self.fire_obs_trigger(false);

        Ok(())
    }

    /// Restart a specific group (used after toggling targets).
    /// Stops the group and restarts it with the updated target list.
    pub fn restart_group(
        &self,
        group_id: &str,
        group: &OutputGroup,
        incoming_url: &str,
        event_sink: Arc<dyn EventSink>,
    ) -> Result<u32, CoreError> {
        if self.is_streaming(group_id) {
            self.stop(group_id)?;
        }

        // Start with updated target list (disabled targets will be filtered out).
        self.start(group, incoming_url, event_sink)
    }

    /// Retry a failed group with exponential backoff.
    /// Returns the delay that should be waited before the next retry.
    pub fn retry_group(
        &self,
        group_id: &str,
        event_sink: Arc<dyn EventSink>,
    ) -> Result<(u32, Option<Duration>), CoreError> {
        let active_groups = self.active_groups.lock().map_err(lock_poisoned)?;

        let config = active_groups
            .get(group_id)
            .ok_or_else(|| ffmpeg_internal("Group not found in active groups"))?;

        let group = config.group.clone();
        drop(active_groups);

        if self.is_streaming(group_id) {
            return Err(ffmpeg_internal("Group is already streaming"));
        }

        // Get or create reconnection state.
        let mut reconnection_state = {
            let processes = self.processes.lock().map_err(lock_poisoned)?;

            processes
                .get(group_id)
                .map(|info| info.reconnection_state.clone())
                .unwrap_or_else(ReconnectionState::new)
        };

        if !reconnection_state.should_retry(&self.reconnection_config) {
            // Terminal state — emit `stream_retry_exhausted` so the UI knows
            // the group is permanently failed, and tear down ALL residual
            // state so subsequent `get_active_group_ids` / `is_streaming`
            // queries report it correctly. Drop both the active-group config
            // AND any dead `processes` entry (the FFmpeg child already
            // exited; the map entry is a corpse).
            emit_event(
                event_sink.as_ref(),
                "stream_retry_exhausted",
                &serde_json::json!({
                    "groupId": group_id,
                    "maxAttempts": self.reconnection_config.max_retries,
                }),
            );
            self.remove_active_group(group_id);
            if let Ok(mut processes) = self.processes.lock() {
                processes.remove(group_id);
            }
            return Err(ffmpeg_internal(format!(
                "Maximum retry attempts ({}) reached",
                self.reconnection_config.max_retries
            )));
        }

        let delay = reconnection_state.next_delay(&self.reconnection_config);

        log::info!(
            "[FFmpeg:{group_id}] Retrying stream (attempt {}/{}) after {} seconds",
            reconnection_state.attempt + 1,
            self.reconnection_config.max_retries,
            delay.as_secs()
        );

        emit_event(
            event_sink.as_ref(),
            "stream_retry_attempt",
            &serde_json::json!({
                "groupId": group_id,
                "attempt": reconnection_state.attempt + 1,
                "maxAttempts": self.reconnection_config.max_retries,
                "delaySecs": delay.as_secs()
            }),
        );

        thread::sleep(delay);

        reconnection_state.increment();

        match self.start_group_process(&group, event_sink.clone()) {
            Ok(pid) => {
                if let Ok(mut processes) = self.processes.lock() {
                    if let Some(info) = processes.get_mut(group_id) {
                        info.reconnection_state = reconnection_state.clone();
                    }
                }

                // Calculate next retry delay in case this one fails.
                let next_delay = if reconnection_state.should_retry(&self.reconnection_config) {
                    Some(reconnection_state.next_delay(&self.reconnection_config))
                } else {
                    None
                };

                log::info!("[FFmpeg:{group_id}] Stream reconnected successfully (PID: {pid})");
                Ok((pid, next_delay))
            }
            Err(e) => {
                log::error!("[FFmpeg:{group_id}] Reconnection failed: {e}");
                Err(ffmpeg_internal(format!("Failed to reconnect: {e}")))
            }
        }
    }

    /// Reset reconnection state for a group (called on successful manual start).
    pub fn reset_reconnection_state(&self, group_id: &str) {
        if let Ok(mut processes) = self.processes.lock() {
            if let Some(info) = processes.get_mut(group_id) {
                info.reconnection_state.reset();
            }
        }
    }
}
