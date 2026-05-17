//! `spiritstream-cli stream …` — observe and control RTMP streams.

use clap::Subcommand;
use serde::Serialize;
use spiritstream_core::ServiceRegistry;

use crate::error::CliError;
use crate::output::Output;

#[derive(Debug, Subcommand)]
pub enum StreamCmd {
    /// Snapshot of every active stream group.
    Status,
    /// Stop a stream group. If `--group` is omitted, every active group is
    /// stopped (matches the rewrite plan's `stream stop [--group <id>]` UX).
    Stop {
        /// Stream group ID. When omitted, every active group is stopped.
        #[arg(long)]
        group: Option<String>,
    },
    /// Begin a new stream from a loaded profile. The CLI loads the profile,
    /// filters to enabled output groups, and dispatches via `FFmpegHandler`.
    /// Returns the spawned FFmpeg PIDs.
    Start {
        /// Profile name to stream from.
        #[arg(long)]
        profile: String,
        /// Specific output group to start (defaults to every enabled group).
        #[arg(long)]
        group: Option<String>,
        /// Password for encrypted profiles.
        #[arg(long)]
        password: Option<String>,
    },
    /// Re-attempt a previously failed stream group with backoff policy.
    Retry { group_id: String },
    /// Validate a profile's encoding config server-side (decorative — the
    /// same checks run inside `start`). Reads profile JSON from `<file>`.
    Validate { file: std::path::PathBuf },
    /// Print active stream group IDs and count.
    Active,
    /// Report whether `<group-id>` is currently streaming.
    IsActive { group_id: String },
    /// Report whether `<target-id>` is currently disabled (skipped during start).
    TargetDisabled { target_id: String },
    /// Toggle a single target's enabled state and restart its parent group
    /// with the new filter applied.
    ToggleTarget {
        /// Target ID to flip.
        target_id: String,
        /// `true` enables the target, `false` disables it.
        #[arg(long)]
        enabled: bool,
        /// Profile name owning the parent group (the group's incoming URL is
        /// taken from this profile's RTMP input).
        #[arg(long)]
        profile: String,
        /// Parent group ID inside the profile.
        #[arg(long)]
        group: String,
        /// Password for encrypted profiles.
        #[arg(long)]
        password: Option<String>,
    },
}

#[derive(Serialize)]
struct StatusResponse {
    active: Vec<String>,
    count: usize,
}

#[derive(Serialize)]
struct StopResponse {
    stopped: Vec<String>,
}

pub async fn run(
    cmd: StreamCmd,
    registry: &ServiceRegistry,
    out: &mut Output,
) -> Result<(), CliError> {
    match cmd {
        StreamCmd::Status => {
            let active = registry.ffmpeg.get_active_group_ids();
            let count = active.len();
            out.emit(&StatusResponse { active, count })?;
            Ok(())
        }
        StreamCmd::Stop { group } => {
            match group {
                Some(id) => {
                    registry.ffmpeg.stop(&id)?;
                    out.emit(&StopResponse { stopped: vec![id] })?;
                }
                None => {
                    let stopped = registry.ffmpeg.get_active_group_ids();
                    registry.ffmpeg.stop_all()?;
                    out.emit(&StopResponse { stopped })?;
                }
            }
            Ok(())
        }
        StreamCmd::Start {
            profile,
            group,
            password,
        } => {
            let p = registry
                .profiles
                .load_with_key_decryption(&profile, password.as_deref())
                .await?;
            let incoming_url = format!(
                "rtmp://{}:{}/{}",
                p.input.bind_address, p.input.port, p.input.application,
            );

            let groups: Vec<spiritstream_core::models::OutputGroup> = match &group {
                Some(id) => p
                    .output_groups
                    .iter()
                    .filter(|g| &g.id == id)
                    .cloned()
                    .collect(),
                None => p
                    .output_groups
                    .iter()
                    .filter(|g| !g.stream_targets.is_empty())
                    .cloned()
                    .collect(),
            };

            if groups.is_empty() {
                return Err(CliError::Argument(format!(
                    "no eligible output groups to start{}",
                    group
                        .as_deref()
                        .map(|g| format!(" (looked for id={g})"))
                        .unwrap_or_default(),
                )));
            }

            let pids =
                registry
                    .ffmpeg
                    .start_all(&groups, &incoming_url, registry.events.clone())?;

            #[derive(Serialize)]
            struct StartResponse {
                pids: Vec<u32>,
            }
            out.emit(&StartResponse { pids })?;
            Ok(())
        }
        StreamCmd::Retry { group_id } => {
            let event_sink = registry.events.clone();
            let ffmpeg = registry.ffmpeg.clone();
            let group_id_clone = group_id.clone();
            let (pid, next_delay) = tokio::task::spawn_blocking(move || {
                ffmpeg.retry_group(&group_id_clone, event_sink)
            })
            .await
            .map_err(|e| CliError::Io(format!("task join error: {e}")))??;

            #[derive(Serialize)]
            struct RetryResponse {
                pid: u32,
                next_delay_secs: Option<u64>,
            }
            out.emit(&RetryResponse {
                pid,
                next_delay_secs: next_delay.map(|d| d.as_secs()),
            })?;
            Ok(())
        }
        StreamCmd::Validate { file } => {
            let body = std::fs::read_to_string(&file)
                .map_err(|e| CliError::Io(format!("read {}: {}", file.display(), e)))?;
            let profile: spiritstream_core::models::Profile = serde_json::from_str(&body)
                .map_err(|e| CliError::Serialization(format!("parse {}: {}", file.display(), e)))?;
            spiritstream_core::services::FFmpegHandler::validate_config(&profile)?;
            out.emit(&serde_json::json!({ "valid": true }))?;
            Ok(())
        }
        StreamCmd::Active => {
            let ids = registry.ffmpeg.get_active_group_ids();
            #[derive(Serialize)]
            struct ActiveResponse {
                ids: Vec<String>,
                count: usize,
            }
            let count = ids.len();
            out.emit(&ActiveResponse { ids, count })?;
            Ok(())
        }
        StreamCmd::IsActive { group_id } => {
            let active = registry.ffmpeg.is_streaming(&group_id);
            out.emit(&serde_json::json!({ "groupId": group_id, "active": active }))?;
            Ok(())
        }
        StreamCmd::TargetDisabled { target_id } => {
            let disabled = registry.ffmpeg.is_target_disabled(&target_id);
            out.emit(&serde_json::json!({ "targetId": target_id, "disabled": disabled }))?;
            Ok(())
        }
        StreamCmd::ToggleTarget {
            target_id,
            enabled,
            profile,
            group,
            password,
        } => {
            let p = registry
                .profiles
                .load_with_key_decryption(&profile, password.as_deref())
                .await?;
            let target_group = p
                .output_groups
                .iter()
                .find(|g| g.id == group)
                .cloned()
                .ok_or_else(|| {
                    CliError::Argument(format!(
                        "output group '{group}' not found in profile '{profile}'"
                    ))
                })?;
            let incoming_url = format!(
                "rtmp://{}:{}/{}",
                p.input.bind_address, p.input.port, p.input.application,
            );
            if enabled {
                registry.ffmpeg.enable_target(&target_id);
            } else {
                registry.ffmpeg.disable_target(&target_id);
            }
            let pid = registry.ffmpeg.restart_group(
                &target_group.id,
                &target_group,
                &incoming_url,
                registry.events.clone(),
            )?;
            out.emit(
                &serde_json::json!({ "pid": pid, "targetId": target_id, "enabled": enabled }),
            )?;
            Ok(())
        }
    }
}
