use std::collections::HashSet;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crate::errors::CoreError;
use crate::models::{OutputGroup, StreamRetryAttemptEvent, StreamRetryExhaustedEvent};
use crate::services::{emit_event, EventSink};

use super::{
    ffmpeg_internal, lock_poisoned, no_eligible_groups_error, no_live_targets_error,
    validate_incoming_url, ReconnectionState,
};

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
        validate_incoming_url(incoming_url)?;
        Self::validate_output_group(group)?;
        if !group.enabled {
            return Err(no_eligible_groups_error());
        }
        if !self.has_live_targets(group) {
            return Err(no_live_targets_error(&group.id));
        }

        // Serialise admission — see the `admission` field docs.
        let _admission = self.admission.lock().map_err(lock_poisoned)?;

        self.record_active_group(group, incoming_url)?;

        if let Some(pid) = self.get_group_pid(&group.id) {
            return Ok(pid);
        }

        // Manual start = fresh retry budget.
        self.clear_reconnection_state(&group.id);

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
    ///
    /// Eligibility (`group.enabled` + at least one enabled target) is
    /// decided HERE, server-side — the frontend sends every group and
    /// renders the outcome (`started (group_id, pid)` pairs, or the
    /// `no_eligible_groups` error). Returns one `(group_id, pid)` per
    /// started group so clients set their active state from authority
    /// instead of inferring it.
    pub fn start_all(
        &self,
        groups: &[OutputGroup],
        incoming_url: &str,
        event_sink: Arc<dyn EventSink>,
    ) -> Result<Vec<(String, u32)>, CoreError> {
        validate_incoming_url(incoming_url)?;
        let eligible: Vec<&OutputGroup> = groups
            .iter()
            .filter(|g| g.is_eligible() && self.has_live_targets(g))
            .collect();
        if eligible.is_empty() {
            return Err(no_eligible_groups_error());
        }

        // Validate every eligible group up-front so we don't start half
        // the stream pipeline before a later group fails its bounds
        // check. Collect every failure into one InvalidStreamConfig.
        let mut all_issues = Vec::new();
        for (gi, group) in eligible.iter().enumerate() {
            let issues = Self::collect_group_issues(group, &format!("/groups/{gi}"));
            all_issues.extend(issues);
        }
        if !all_issues.is_empty() {
            return Err(CoreError::InvalidStreamConfig {
                reasons: all_issues,
            });
        }

        // Admission is atomic: the check below and the reservations
        // after it happen under one lock, so two concurrent start_all
        // calls can't both pass the idle check and double-spawn.
        let _admission = self.admission.lock().map_err(lock_poisoned)?;

        if self.active_count() > 0 {
            return Err(ffmpeg_internal("Streams already running"));
        }

        if let Ok(mut active) = self.active_groups.lock() {
            active.clear();
        }
        if let Ok(mut states) = self.reconnection_states.lock() {
            states.clear();
        }

        let mut desired_group_ids: HashSet<String> = HashSet::new();
        for group in &eligible {
            self.record_active_group(group, incoming_url)?;
            desired_group_ids.insert(group.id.clone());
        }

        let mut started = Vec::with_capacity(eligible.len());
        for group in &eligible {
            let pid = self.start_group_process(group, Arc::clone(&event_sink))?;
            started.push((group.id.clone(), pid));
        }

        self.ensure_relay_running(incoming_url, &desired_group_ids)?;

        // SpiritStream→OBS start cascade. Fires AFTER the relay is up so OBS
        // connects to a live ingest. The trigger checks the active profile's
        // `obs.direction` server-side; this call is a no-op when the trigger
        // handle isn't installed (tests / CLI) or when direction forbids it.
        self.fire_obs_start_trigger();

        Ok(started)
    }

    /// Stop streaming for an output group.
    pub fn stop(&self, group_id: &str) -> Result<(), CoreError> {
        self.remove_active_group(group_id);
        self.clear_reconnection_state(group_id);
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

        self.sync_process_registry();

        Ok(())
    }

    /// Stop all active streams.
    pub fn stop_all(&self) -> Result<(), CoreError> {
        if let Ok(mut active) = self.active_groups.lock() {
            active.clear();
        }
        if let Ok(mut states) = self.reconnection_states.lock() {
            states.clear();
        }
        {
            let mut processes = self.processes.lock().map_err(lock_poisoned)?;
            let mut stopping = self.stopping_groups.lock().map_err(lock_poisoned)?;
            for (group_id, mut info) in processes.drain() {
                stopping.insert(group_id);
                self.stop_child(&mut info.child);
            }
        }
        self.stop_relay();

        // Clear all port assignments since all groups are stopped.
        if let Ok(mut assignments) = self.port_assignments.lock() {
            assignments.clear();
            self.next_port_offset.store(0, Ordering::SeqCst);
            log::debug!("All groups stopped, cleared all port assignments");
        }

        self.sync_process_registry();

        // NB: no SpiritStream→OBS stop trigger here. The relay's RTMP ingest
        // has just been torn down, so OBS is mid-reconnect; a StopStream now
        // would hang OBS (obs-websocket #1230). App-initiated stops go through
        // `stop_all_orchestrated`, which stops OBS first while the relay is
        // still listening. OBS→SpiritStream cascade stops reach here directly
        // (OBS already stopped itself), as does the panic path.

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
    ///
    /// Retry state lives in `reconnection_states` (handler-level, keyed
    /// by group id) — NOT in `ProcessInfo`. The crash handler removes
    /// the `ProcessInfo` before this runs, so state stored there always
    /// read attempt=0: the counter never advanced, `max_retries` was
    /// unreachable, and a permanently-failed group (revoked stream key,
    /// dead platform) reconnect-looped forever, flooding the event bus.
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

        let attempt_decision = {
            let mut states = self.reconnection_states.lock().map_err(lock_poisoned)?;
            let state = states
                .entry(group_id.to_string())
                .or_insert_with(ReconnectionState::new);
            if state.should_retry(&self.reconnection_config) {
                let delay = state.next_delay(&self.reconnection_config);
                // Count the attempt up-front so a crash between here and
                // the next retry still advances the budget.
                state.increment();
                Some((state.attempt, delay))
            } else {
                None
            }
        };

        let Some((attempt, delay)) = attempt_decision else {
            // Terminal state — emit `stream_retry_exhausted` exactly once
            // so the UI knows the group is permanently failed, and tear
            // down ALL residual state (active-group config, dead process
            // corpse, retry counter) so subsequent queries report it
            // correctly.
            emit_event(
                event_sink.as_ref(),
                "stream_retry_exhausted",
                &StreamRetryExhaustedEvent {
                    group_id: group_id.to_string(),
                    max_attempts: self.reconnection_config.max_retries,
                },
            );
            self.remove_active_group(group_id);
            self.clear_reconnection_state(group_id);
            if let Ok(mut processes) = self.processes.lock() {
                processes.remove(group_id);
            }
            self.sync_process_registry();
            return Err(ffmpeg_internal(format!(
                "Maximum retry attempts ({}) reached",
                self.reconnection_config.max_retries
            )));
        };

        log::info!(
            "[FFmpeg:{group_id}] Retrying stream (attempt {}/{}) after {} seconds",
            attempt,
            self.reconnection_config.max_retries,
            delay.as_secs()
        );

        emit_event(
            event_sink.as_ref(),
            "stream_retry_attempt",
            &StreamRetryAttemptEvent {
                group_id: group_id.to_string(),
                attempt,
                max_attempts: self.reconnection_config.max_retries,
                delay_secs: delay.as_secs(),
            },
        );

        thread::sleep(delay);

        match self.start_group_process(&group, event_sink.clone()) {
            Ok(pid) => {
                // Calculate next retry delay in case this one fails.
                let next_delay = {
                    let states = self.reconnection_states.lock().map_err(lock_poisoned)?;
                    states.get(group_id).and_then(|state| {
                        state
                            .should_retry(&self.reconnection_config)
                            .then(|| state.next_delay(&self.reconnection_config))
                    })
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

    /// Drop the retry budget for a group — called on manual stop, on
    /// manual (re)start, and after a stable run (the stats reader resets
    /// once a stream survives 60s).
    pub fn clear_reconnection_state(&self, group_id: &str) {
        if let Ok(mut states) = self.reconnection_states.lock() {
            states.remove(group_id);
        }
    }
}
