// FFmpeg Relay Management
// Handles shared relay process for fan-out to multiple output groups

use std::collections::HashSet;
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::sync::atomic::AtomicUsize;
use std::thread;

use crate::services::process_util::{configure_hidden_window, kill_and_wait};

/// FFmpeg relay process for shared ingest
pub(super) struct RelayProcess {
    pub(super) child: Child,
    pub(super) incoming_url: String,
    pub(super) output_groups: HashSet<String>,
}

/// Configuration constants for relay
pub(super) struct RelayConfig;

impl RelayConfig {
    pub(super) const RELAY_HOST: &'static str = "localhost";
    pub(super) const RELAY_PORT_BASE: u16 = 20000;
    pub(super) const RELAY_PORT_RANGE: u16 = 20000;
    pub(super) const RELAY_TCP_OUT_QUERY: &'static str = "tcp_nodelay=1";
    pub(super) const RELAY_TCP_IN_QUERY: &'static str = "listen=1&tcp_nodelay=1";
    pub(super) const RELAY_RTMP_TIMEOUT_SECS: u32 = 604_800;
    pub(super) const RELAY_RTMP_TCP_NODELAY: &'static str = "1";
    pub(super) const RELAY_TEE_FIFO_OPTIONS: &'static str =
        "fifo_format=mpegts:queue_size=512:drop_pkts_on_overflow=1:attempt_recovery=1:recover_any_error=1";
}

/// Manages FFmpeg relay process lifecycle
pub(super) struct FFmpegRelay {
    relay: Arc<Mutex<Option<RelayProcess>>>,
    relay_refcount: Arc<AtomicUsize>,
    ffmpeg_path: String,
}

impl FFmpegRelay {
    pub(super) fn new(ffmpeg_path: String) -> Self {
        Self {
            relay: Arc::new(Mutex::new(None)),
            relay_refcount: Arc::new(AtomicUsize::new(0)),
            ffmpeg_path,
        }
    }

    pub(super) fn relay(&self) -> &Arc<Mutex<Option<RelayProcess>>> {
        &self.relay
    }

    pub(super) fn relay_refcount(&self) -> &Arc<AtomicUsize> {
        &self.relay_refcount
    }

    /// Check if relay is currently active
    pub(super) fn is_active(&self) -> Result<bool, String> {
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

    /// Ensure relay process is running for shared ingest
    pub(super) fn ensure_running(
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
            kill_and_wait(&mut relay.child);
        }

        let args = Self::build_relay_args(incoming_url, &relay_groups)?;
        let sanitized: Vec<String> = args.iter()
            .map(|arg| super::FFmpegHandler::sanitize_arg_static(arg))
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
        configure_hidden_window(&mut cmd);
        let mut child = cmd.spawn()
            .map_err(|e| format!("Failed to start FFmpeg relay: {e}"))?;

        if let Some(stderr) = child.stderr.take() {
            thread::spawn(move || {
                let reader = BufReader::new(stderr);
                for line in reader.lines().map_while(Result::ok) {
                    let sanitized = super::FFmpegHandler::sanitize_arg_static(&line);
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
    pub(super) fn stop(&self) {
        let mut relay_guard = match self.relay.lock() {
            Ok(guard) => guard,
            Err(_) => return,
        };

        if let Some(mut relay) = relay_guard.take() {
            kill_and_wait(&mut relay.child);
        }
    }

    /// Build FFmpeg arguments for the shared relay process
    fn build_relay_args(
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
            RelayConfig::RELAY_RTMP_TIMEOUT_SECS.to_string(),
            "-tcp_nodelay".to_string(),
            RelayConfig::RELAY_RTMP_TCP_NODELAY.to_string(),
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
            RelayConfig::RELAY_TEE_FIFO_OPTIONS.to_string(),
            outputs,
        ])
    }

    pub(super) fn relay_port_for_group(group_id: &str) -> u16 {
        const FNV_OFFSET: u32 = 2166136261;
        const FNV_PRIME: u32 = 16777619;

        let mut hash = FNV_OFFSET;
        for &b in group_id.as_bytes() {
            hash ^= b as u32;
            hash = hash.wrapping_mul(FNV_PRIME);
        }

        let range = RelayConfig::RELAY_PORT_RANGE as u32;
        let port = RelayConfig::RELAY_PORT_BASE as u32 + (hash % range);
        port as u16
    }

    pub(super) fn relay_output_url_for_group(group_id: &str) -> String {
        format!(
            "tcp://{}:{}?{}",
            RelayConfig::RELAY_HOST,
            Self::relay_port_for_group(group_id),
            RelayConfig::RELAY_TCP_OUT_QUERY
        )
    }

    pub(super) fn relay_input_url_for_group(group_id: &str) -> String {
        format!(
            "tcp://{}:{}?{}",
            RelayConfig::RELAY_HOST,
            Self::relay_port_for_group(group_id),
            RelayConfig::RELAY_TCP_IN_QUERY
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
}
