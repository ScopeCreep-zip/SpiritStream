//! Cross-process registry of live FFmpeg processes.
//!
//! `FFmpegHandler` state is per-process: a one-shot CLI invocation (or
//! a server started after a crash) sees an empty handler even while
//! FFmpeg children from another process are still streaming. For an app
//! whose flagship safety feature is the panic button, that made
//! `spiritstream-cli safety panic` a silent no-op against any stream it
//! didn't start itself.
//!
//! The handler persists every spawn/stop/crash to
//! `DATA_DIR/run/stream_processes.json` (atomic, owner-only). The
//! kill path verifies each recorded pid still belongs to an FFmpeg
//! process (guards against pid reuse) before TERM → 3s → KILL.
//! Stale or mismatched records are dropped with a loud log line —
//! never killed.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::errors::CoreError;

const REGISTRY_FILENAME: &str = "stream_processes.json";

/// One live FFmpeg process owned by SpiritStream.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamProcessRecord {
    /// Output group id, or `"__relay__"` for the shared ingest relay.
    pub group_id: String,
    pub pid: u32,
    pub started_at_unix_ms: i64,
    /// FFmpeg binary path at spawn time — used to verify pid identity
    /// before any kill.
    pub ffmpeg_path: String,
}

/// Sentinel group id for the relay record.
pub const RELAY_GROUP_ID: &str = "__relay__";

/// Outcome of [`kill_recorded_processes`].
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct OrphanKillReport {
    /// Processes that were verified as FFmpeg and killed.
    pub killed: usize,
    /// Records whose pid was dead or belonged to a different program —
    /// dropped, never killed.
    pub stale: usize,
}

fn registry_path(run_dir: &Path) -> PathBuf {
    run_dir.join(REGISTRY_FILENAME)
}

/// Read the registry. A missing file is an empty registry; a corrupt
/// file is an error (fail loud — a half-written registry should never
/// silently report "nothing to kill" to a panic).
pub fn read_records(run_dir: &Path) -> Result<Vec<StreamProcessRecord>, CoreError> {
    let path = registry_path(run_dir);
    if !path.exists() {
        return Ok(Vec::new());
    }
    let text = std::fs::read_to_string(&path).map_err(|e| CoreError::Internal {
        context: format!("read {}: {e}", path.display()),
    })?;
    serde_json::from_str(&text).map_err(|e| CoreError::Internal {
        context: format!("parse {}: {e}", path.display()),
    })
}

pub(super) fn write_records(
    run_dir: &Path,
    records: &[StreamProcessRecord],
) -> Result<(), CoreError> {
    std::fs::create_dir_all(run_dir).map_err(|e| CoreError::Internal {
        context: format!("create {}: {e}", run_dir.display()),
    })?;
    let json = serde_json::to_string_pretty(records).map_err(|e| CoreError::Internal {
        context: format!("serialize stream process registry: {e}"),
    })?;
    crate::services::write_owner_only_atomic(&registry_path(run_dir), json.as_bytes())
}

/// Does `pid` still belong to the FFmpeg binary the record was spawned
/// with? Compares the executable/cmd basename — a recycled pid running
/// someone else's program never matches.
fn pid_is_recorded_ffmpeg(system: &sysinfo::System, record: &StreamProcessRecord) -> bool {
    let Some(process) = system.process(sysinfo::Pid::from_u32(record.pid)) else {
        return false;
    };
    let expected = Path::new(&record.ffmpeg_path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| record.ffmpeg_path.clone());
    let name_matches = process.name().to_string_lossy().contains(&expected);
    let exe_matches = process
        .exe()
        .and_then(|p| p.file_name())
        .map(|n| n.to_string_lossy().contains(&expected))
        .unwrap_or(false);
    name_matches || exe_matches
}

/// Kill every recorded FFmpeg process that is verifiably still ours,
/// then clear the registry. TERM first, 3s grace, then KILL.
pub fn kill_recorded_processes(run_dir: &Path) -> Result<OrphanKillReport, CoreError> {
    let records = read_records(run_dir)?;
    if records.is_empty() {
        return Ok(OrphanKillReport::default());
    }

    let mut system = sysinfo::System::new();
    system.refresh_processes(sysinfo::ProcessesToUpdate::All, true);

    let mut report = OrphanKillReport::default();
    let mut to_kill: Vec<&StreamProcessRecord> = Vec::new();
    for record in &records {
        if pid_is_recorded_ffmpeg(&system, record) {
            to_kill.push(record);
        } else {
            log::warn!(
                "stream process registry: dropping stale record (group {}, pid {}) — \
                 process is gone or is not {}",
                record.group_id,
                record.pid,
                record.ffmpeg_path
            );
            report.stale += 1;
        }
    }

    for record in &to_kill {
        if let Some(process) = system.process(sysinfo::Pid::from_u32(record.pid)) {
            // Graceful first. `kill_with` returns None on platforms
            // without signal support (Windows) — fall through to the
            // hard kill below either way.
            let _ = process.kill_with(sysinfo::Signal::Term);
        }
    }
    if !to_kill.is_empty() {
        std::thread::sleep(Duration::from_secs(3));
        system.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
        for record in &to_kill {
            if let Some(process) = system.process(sysinfo::Pid::from_u32(record.pid)) {
                if pid_is_recorded_ffmpeg(&system, record) {
                    process.kill();
                }
            }
            log::info!(
                "stream process registry: killed orphaned FFmpeg (group {}, pid {})",
                record.group_id,
                record.pid
            );
            report.killed += 1;
        }
    }

    write_records(run_dir, &[])?;
    Ok(report)
}

impl super::FFmpegHandler {
    /// Snapshot the current process + relay state into the on-disk
    /// registry. Best-effort with a loud log on failure: the registry
    /// is the panic button's reach into other processes, so a write
    /// failure is worth an error line, but it must never fail the
    /// stream operation that triggered it.
    pub(super) fn sync_process_registry(&self) {
        let mut records: Vec<StreamProcessRecord> = Vec::new();
        if let Ok(processes) = self.processes.lock() {
            for info in processes.values() {
                records.push(StreamProcessRecord {
                    group_id: info.group_id.clone(),
                    pid: info.child.id(),
                    started_at_unix_ms: info.started_at_unix_ms,
                    ffmpeg_path: self.ffmpeg_path.clone(),
                });
            }
        }
        if let Ok(relay) = self.relay.lock() {
            if let Some(relay) = relay.as_ref() {
                records.push(StreamProcessRecord {
                    group_id: RELAY_GROUP_ID.to_string(),
                    pid: relay.child.id(),
                    started_at_unix_ms: 0,
                    ffmpeg_path: self.ffmpeg_path.clone(),
                });
            }
        }
        if let Err(e) = write_records(&self.run_dir, &records) {
            // `{e:?}` deliberately: Display for CoreError::Internal hides
            // the context from CLIENTS, but this line is the server-side
            // log — the one place the context is supposed to surface.
            log::error!("failed to persist stream process registry: {e:?}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn missing_registry_reads_empty() {
        let dir = TempDir::new().unwrap();
        assert!(read_records(dir.path()).unwrap().is_empty());
    }

    #[test]
    fn records_round_trip() {
        let dir = TempDir::new().unwrap();
        let records = vec![StreamProcessRecord {
            group_id: "g1".into(),
            pid: 4242,
            started_at_unix_ms: 1,
            ffmpeg_path: "/usr/bin/ffmpeg".into(),
        }];
        write_records(dir.path(), &records).unwrap();
        let read = read_records(dir.path()).unwrap();
        assert_eq!(read.len(), 1);
        assert_eq!(read[0].group_id, "g1");
        assert_eq!(read[0].pid, 4242);
    }

    /// Pid-reuse guard: a record pointing at a live process that is NOT
    /// the recorded FFmpeg binary must be dropped as stale, never
    /// killed. We use our own test process pid — definitely alive,
    /// definitely not ffmpeg.
    #[test]
    fn live_but_foreign_pid_is_stale_not_killed() {
        let dir = TempDir::new().unwrap();
        let records = vec![StreamProcessRecord {
            group_id: "g1".into(),
            pid: std::process::id(),
            started_at_unix_ms: 1,
            ffmpeg_path: "/usr/bin/ffmpeg".into(),
        }];
        write_records(dir.path(), &records).unwrap();
        let report = kill_recorded_processes(dir.path()).unwrap();
        assert_eq!(report.killed, 0);
        assert_eq!(report.stale, 1);
        // Registry cleared either way.
        assert!(read_records(dir.path()).unwrap().is_empty());
        // And we are demonstrably still alive.
    }

    /// The kill path itself: a live process whose identity matches the
    /// record gets TERM→KILLed. We stand in a long-running `sleep` for
    /// ffmpeg by recording its own path — identity verification compares
    /// the recorded binary's basename against the live process.
    #[cfg(unix)]
    #[test]
    fn matching_live_process_is_killed() {
        let dir = TempDir::new().unwrap();
        let mut child = std::process::Command::new("/bin/sleep")
            .arg("300")
            .spawn()
            .expect("spawn sleep");
        let records = vec![StreamProcessRecord {
            group_id: "g1".into(),
            pid: child.id(),
            started_at_unix_ms: 1,
            ffmpeg_path: "/bin/sleep".into(),
        }];
        write_records(dir.path(), &records).unwrap();

        let report = kill_recorded_processes(dir.path()).unwrap();
        assert_eq!(report.killed, 1, "recorded process must be killed");

        // The child must actually be dead (reap it).
        let status = child.wait().expect("wait on killed child");
        assert!(!status.success(), "child was killed, not exited cleanly");
    }

    #[test]
    fn dead_pid_is_stale() {
        let dir = TempDir::new().unwrap();
        // Spawn-and-reap a child so its pid is very likely unused.
        let mut child = std::process::Command::new("true")
            .spawn()
            .or_else(|_| {
                std::process::Command::new("cmd")
                    .args(["/C", "exit"])
                    .spawn()
            })
            .expect("spawn trivial child");
        let pid = child.id();
        let _ = child.wait();
        let records = vec![StreamProcessRecord {
            group_id: "g1".into(),
            pid,
            started_at_unix_ms: 1,
            ffmpeg_path: "/usr/bin/ffmpeg".into(),
        }];
        write_records(dir.path(), &records).unwrap();
        let report = kill_recorded_processes(dir.path()).unwrap();
        assert_eq!(report.killed, 0);
        assert_eq!(report.stale, 1);
    }
}
