use std::collections::HashSet;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use crate::errors::CoreError;
use crate::models::OutputGroup;
use crate::services::EventSink;

#[cfg(windows)]
use super::CommandExt;
#[cfg(windows)]
use super::CREATE_NO_WINDOW;
use super::{ffmpeg_internal, lock_poisoned, ReconnectionState};

/// Process info for tracking active streams.
pub(super) struct ProcessInfo {
    pub(super) child: Child,
    pub(super) start_time: Instant,
    pub(super) group_id: String,
    pub(super) reconnection_state: ReconnectionState,
}

/// Cached configuration for restarting groups when relay output set changes.
pub(super) struct ActiveGroupConfig {
    pub(super) group: OutputGroup,
    pub(super) incoming_url: String,
}

/// FFmpeg relay process for shared ingest.
pub(super) struct RelayProcess {
    pub(super) child: Child,
    pub(super) incoming_url: String,
    pub(super) output_groups: HashSet<String>,
}

impl super::FFmpegHandler {
    pub(super) fn record_active_group(
        &self,
        group: &OutputGroup,
        incoming_url: &str,
    ) -> Result<(), CoreError> {
        let mut active = self.active_groups.lock().map_err(lock_poisoned)?;

        if let Some(existing) = active.values().next() {
            if existing.incoming_url != incoming_url {
                return Err(ffmpeg_internal("Incoming URL differs from active groups"));
            }
        }

        active.insert(
            group.id.clone(),
            ActiveGroupConfig {
                group: group.clone(),
                incoming_url: incoming_url.to_string(),
            },
        );

        Ok(())
    }

    pub(super) fn remove_active_group(&self, group_id: &str) {
        if let Ok(mut active) = self.active_groups.lock() {
            active.remove(group_id);
        }
    }

    pub(super) fn collect_active_group_ids(&self) -> Result<HashSet<String>, CoreError> {
        let active = self.active_groups.lock().map_err(lock_poisoned)?;
        Ok(active.keys().cloned().collect())
    }

    pub(super) fn resolve_active_incoming_url(&self) -> Result<String, CoreError> {
        let active = self.active_groups.lock().map_err(lock_poisoned)?;
        let mut incoming_url: Option<String> = None;
        for cfg in active.values() {
            if let Some(existing) = &incoming_url {
                if existing != &cfg.incoming_url {
                    return Err(ffmpeg_internal("Incoming URL differs across active groups"));
                }
            } else {
                incoming_url = Some(cfg.incoming_url.clone());
            }
        }
        incoming_url.ok_or_else(|| ffmpeg_internal("No active groups available"))
    }

    pub(super) fn get_group_pid(&self, group_id: &str) -> Option<u32> {
        self.processes
            .lock()
            .ok()
            .and_then(|procs| procs.get(group_id).map(|info| info.child.id()))
    }

    pub(super) fn relay_needs_restart(
        &self,
        desired_group_ids: &HashSet<String>,
    ) -> Result<bool, CoreError> {
        let relay_guard = self.relay.lock().map_err(lock_poisoned)?;
        if let Some(relay) = relay_guard.as_ref() {
            if !relay.output_groups.is_superset(desired_group_ids) {
                let processes = self.processes.lock().map_err(lock_poisoned)?;
                return Ok(!processes.is_empty());
            }
        }
        Ok(false)
    }

    pub(super) fn is_relay_active(&self) -> Result<bool, CoreError> {
        let mut relay_guard = self.relay.lock().map_err(lock_poisoned)?;
        if let Some(relay) = relay_guard.as_mut() {
            if let Ok(Some(_)) = relay.child.try_wait() {
                *relay_guard = None;
                return Ok(false);
            }
            return Ok(true);
        }
        Ok(false)
    }

    pub(super) fn stop_group_for_restart(&self, group_id: &str) -> Result<(), CoreError> {
        if let Ok(mut stopping) = self.stopping_groups.lock() {
            stopping.insert(group_id.to_string());
        }
        let removed = {
            let mut processes = self.processes.lock().map_err(lock_poisoned)?;
            processes.remove(group_id)
        };

        if let Some(mut info) = removed {
            self.stop_child(&mut info.child);
        }
        Ok(())
    }

    pub(super) fn start_group_process(
        &self,
        group: &OutputGroup,
        event_sink: Arc<dyn EventSink>,
    ) -> Result<u32, CoreError> {
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
        let mut child = cmd.spawn().map_err(|_| CoreError::FfmpegNotFound)?;

        let pid = child.id();
        let group_id = group.id.clone();

        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| ffmpeg_internal("Failed to capture FFmpeg stderr"))?;

        {
            let mut processes = self.processes.lock().map_err(lock_poisoned)?;
            processes.insert(
                group_id.clone(),
                ProcessInfo {
                    child,
                    start_time: Instant::now(),
                    group_id: group_id.clone(),
                    reconnection_state: ReconnectionState::new(),
                },
            );
        }

        self.relay_refcount.fetch_add(1, Ordering::SeqCst);

        let event_sink_clone = Arc::clone(&event_sink);
        let processes_clone = Arc::clone(&self.processes);
        let meter_bytes = self.start_bitrate_meter(&group_id, Arc::clone(&processes_clone));
        let relay_clone = Arc::clone(&self.relay);
        let stopping_clone = Arc::clone(&self.stopping_groups);
        let relay_refcount_clone = Arc::clone(&self.relay_refcount);
        let port_assignments_clone = Arc::clone(&self.port_assignments);
        let next_port_offset_clone = Arc::clone(&self.next_port_offset);
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
                port_assignments_clone,
                next_port_offset_clone,
            );
        });

        Ok(pid)
    }

    pub(super) fn restart_relay_with_groups(
        &self,
        requested_group_id: &str,
        event_sink: Arc<dyn EventSink>,
    ) -> Result<u32, CoreError> {
        let incoming_url = self.resolve_active_incoming_url()?;
        let desired_group_ids = self.collect_active_group_ids()?;

        let running_group_ids = self.get_active_group_ids();
        for group_id in running_group_ids {
            let _ = self.stop_group_for_restart(&group_id);
        }
        self.stop_relay();

        let active_groups = self.active_groups.lock().map_err(lock_poisoned)?;
        let mut requested_pid: Option<u32> = None;
        for (group_id, cfg) in active_groups.iter() {
            let pid = self.start_group_process(&cfg.group, Arc::clone(&event_sink))?;
            if group_id == requested_group_id {
                requested_pid = Some(pid);
            }
        }

        self.ensure_relay_running(&incoming_url, &desired_group_ids)?;

        requested_pid.ok_or_else(|| ffmpeg_internal("Requested group not started"))
    }

    /// Ensure relay process is running for shared ingest.
    pub(super) fn ensure_relay_running(
        &self,
        incoming_url: &str,
        requested_groups: &HashSet<String>,
    ) -> Result<(), CoreError> {
        let mut relay_guard = self.relay.lock().map_err(lock_poisoned)?;

        if let Some(relay) = relay_guard.as_mut() {
            if let Ok(Some(_)) = relay.child.try_wait() {
                *relay_guard = None;
            }
        }

        if let Some(relay) = relay_guard.as_ref() {
            if relay.incoming_url != incoming_url {
                return Err(ffmpeg_internal(
                    "Incoming URL differs from active relay input",
                ));
            }

            // Only skip restart if relay has EXACTLY the requested groups
            // (not just a superset, to handle group removal).
            if relay.output_groups == *requested_groups {
                return Ok(());
            }
        }

        if requested_groups.is_empty() {
            return Err(ffmpeg_internal(
                "No output groups provided for relay fan-out",
            ));
        }

        let relay_groups = requested_groups.clone();

        if let Some(mut relay) = relay_guard.take() {
            let _ = relay.child.kill();
            let _ = relay.child.wait();
        }

        let args = self.build_relay_args(incoming_url, &relay_groups)?;
        let sanitized: Vec<String> = args
            .iter()
            .map(|arg| Self::sanitize_arg_static(arg))
            .collect();
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
        let mut child = cmd.spawn().map_err(|_| CoreError::FfmpegNotFound)?;

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

    /// Stop the relay process if running.
    pub(super) fn stop_relay(&self) {
        let mut relay_guard = match self.relay.lock() {
            Ok(guard) => guard,
            Err(_) => return,
        };

        if let Some(mut relay) = relay_guard.take() {
            let _ = relay.child.kill();
            let _ = relay.child.wait();
        }
    }

    pub(super) fn stop_child(&self, child: &mut Child) {
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

    /// Get or assign a port offset for a group (simplified sequential allocation).
    pub(super) fn get_port_offset(&self, group_id: &str) -> u16 {
        let mut assignments = self.port_assignments.lock().unwrap_or_else(|e| {
            log::warn!("Port assignments mutex poisoned, recovering: {e}");
            e.into_inner()
        });

        if let Some(&offset) = assignments.get(group_id) {
            return offset;
        }

        // Assign next available offset, preventing AtomicU16 wraparound.
        let offset = loop {
            let current = self.next_port_offset.load(Ordering::SeqCst);
            if current == u16::MAX {
                log::error!(
                    "Port offset counter reached maximum value ({}); \
                    refusing to allocate new port offset to group {group_id}",
                    u16::MAX
                );
                break current;
            }

            let next = current + 1;
            match self.next_port_offset.compare_exchange(
                current,
                next,
                Ordering::SeqCst,
                Ordering::SeqCst,
            ) {
                Ok(_) => break current,
                Err(_) => continue,
            }
        };
        assignments.insert(group_id.to_string(), offset);

        log::debug!(
            "Assigned port offset {offset} to group {group_id} (relay: {}, meter: {})",
            Self::RELAY_PORT_BASE + (offset * 2),
            Self::RELAY_PORT_BASE + (offset * 2) + 1
        );

        offset
    }

    /// Free port assignment when a group stops.
    pub(super) fn free_port_offset(&self, group_id: &str) {
        let mut assignments = self.port_assignments.lock().unwrap_or_else(|e| {
            log::warn!("Port assignments mutex poisoned, recovering: {e}");
            e.into_inner()
        });

        if let Some(offset) = assignments.remove(group_id) {
            log::debug!("Freed port offset {offset} from group {group_id}");

            if assignments.is_empty() {
                self.next_port_offset.store(0, Ordering::SeqCst);
                log::debug!("All ports freed, reset counter to 0");
            }
        }
    }

    pub(super) fn relay_port_for_group(&self, group_id: &str) -> u16 {
        let offset = self.get_port_offset(group_id);
        Self::RELAY_PORT_BASE + (offset * 2)
    }

    pub(super) fn relay_output_url_for_group(&self, group_id: &str) -> String {
        format!(
            "tcp://{}:{}?{}",
            Self::RELAY_HOST,
            self.relay_port_for_group(group_id),
            Self::RELAY_TCP_OUT_QUERY
        )
    }

    pub(super) fn relay_input_url_for_group(&self, group_id: &str) -> String {
        format!(
            "tcp://{}:{}?{}",
            Self::RELAY_HOST,
            self.relay_port_for_group(group_id),
            Self::RELAY_TCP_IN_QUERY
        )
    }

    pub(super) fn meter_port_for_group(&self, group_id: &str) -> u16 {
        let offset = self.get_port_offset(group_id);
        Self::RELAY_PORT_BASE + (offset * 2) + 1
    }

    pub(super) fn meter_output_url_for_group(&self, group_id: &str) -> String {
        format!(
            "udp://{}:{}?{}",
            Self::METER_HOST,
            self.meter_port_for_group(group_id),
            Self::METER_UDP_QUERY
        )
    }

    pub(super) fn relay_tee_output_list(&self, group_ids: &HashSet<String>) -> String {
        let mut ids: Vec<&String> = group_ids.iter().collect();
        ids.sort();
        ids.into_iter()
            .map(|id| format!("[f=mpegts]{}", self.relay_output_url_for_group(id)))
            .collect::<Vec<String>>()
            .join("|")
    }

    pub(super) fn normalize_relay_input_url(url: &str) -> String {
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
}
