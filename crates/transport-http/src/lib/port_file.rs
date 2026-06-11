//! Bind-port resolution + the cross-process port discovery file.
//!
//! The server may bind port 0 ("ask the OS for any free port") — the
//! default whenever the profile's Remote Access toggle is off, because
//! a localhost-only API has no reason to claim a fixed port that other
//! software on the machine may want. Anything that needs to find the
//! running server (the Tauri shell health-checking its sidecar, a
//! debugging operator) reads `{DATA_DIR}/run/server.port`, written
//! atomically AFTER the listener is bound so the recorded port is the
//! real one, never a guess.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use spiritstream_core::services::write_owner_only_atomic;

/// On-disk shape of `run/server.port`. The pid lets readers reject a
/// stale file left by a crashed predecessor (the Tauri shell deletes
/// the file before spawning and then accepts only its own child's pid).
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PortFile {
    pub port: u16,
    pub pid: u32,
}

/// Resolve the port the listener should request.
///
/// Precedence: an explicit `SPIRITSTREAM_PORT` (including an explicit
/// `0`) always wins. Without it, the profile's configured port is used
/// only when Remote Access is on — a remote-reachable server needs a
/// stable, user-chosen port for firewall rules and client bookmarks.
/// With Remote Access off (the localhost-only default) the OS assigns
/// a free port, so SpiritStream can never collide with other software.
pub(crate) fn resolve_bind_port(
    env_port: Option<u16>,
    remote_enabled: bool,
    profile_port: u16,
) -> u16 {
    match env_port {
        Some(port) => port,
        None if remote_enabled => profile_port,
        None => 0,
    }
}

fn port_file_path(data_dir: &Path) -> PathBuf {
    data_dir.join("run").join("server.port")
}

/// Persist the bound port for cross-process discovery. Called exactly
/// once, after `TcpListener::bind` succeeds, with the REAL port from
/// `local_addr()`. Atomic + owner-only via the shared secure-write
/// helper; `run/` is created on demand like the other run-state files.
pub(crate) fn write_port_file(
    data_dir: &Path,
    port: u16,
) -> Result<(), Box<dyn std::error::Error>> {
    let run_dir = data_dir.join("run");
    std::fs::create_dir_all(&run_dir)
        .map_err(|e| format!("creating {}: {e}", run_dir.display()))?;
    let record = PortFile {
        port,
        pid: std::process::id(),
    };
    let json = serde_json::to_string(&record)?;
    write_owner_only_atomic(&port_file_path(data_dir), json.as_bytes())
        .map_err(|e| format!("writing server.port: {e}"))?;
    Ok(())
}

/// Best-effort removal on orderly shutdown. A crash leaves the file
/// behind; readers detect staleness via the recorded pid.
pub(crate) fn remove_port_file(data_dir: &Path) {
    let path = port_file_path(data_dir);
    if let Err(e) = std::fs::remove_file(&path) {
        if e.kind() != std::io::ErrorKind::NotFound {
            log::warn!("Failed to remove {}: {e}", path.display());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_port_wins_regardless_of_remote_setting() {
        assert_eq!(resolve_bind_port(Some(9000), false, 8008), 9000);
        assert_eq!(resolve_bind_port(Some(9000), true, 8008), 9000);
        // An explicit 0 is a deliberate "OS-assigned" request.
        assert_eq!(resolve_bind_port(Some(0), true, 8008), 0);
    }

    #[test]
    fn remote_enabled_uses_profile_port() {
        assert_eq!(resolve_bind_port(None, true, 8123), 8123);
    }

    #[test]
    fn remote_disabled_asks_the_os() {
        assert_eq!(resolve_bind_port(None, false, 8123), 0);
    }

    #[test]
    fn port_file_round_trips_and_removes() {
        let dir = tempfile::tempdir().unwrap();
        write_port_file(dir.path(), 54321).unwrap();
        let raw = std::fs::read_to_string(dir.path().join("run").join("server.port")).unwrap();
        let parsed: PortFile = serde_json::from_str(&raw).unwrap();
        assert_eq!(parsed.port, 54321);
        assert_eq!(parsed.pid, std::process::id());

        remove_port_file(dir.path());
        assert!(!dir.path().join("run").join("server.port").exists());
        // Second removal is a quiet no-op.
        remove_port_file(dir.path());
    }
}
