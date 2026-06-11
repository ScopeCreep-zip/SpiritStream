//! Shell-side reader for the server's `run/server.port` discovery file.
//!
//! The server binds its listener (port 0 = OS-assigned unless the
//! profile enables Remote Access) and only THEN writes the real port
//! here. The shell never mirrors the server's port-resolution logic —
//! the old mirror silently diverged for password-encrypted profiles.
//! Instead it deletes any stale file, spawns the sidecar, and waits for
//! a file whose recorded pid matches the freshly spawned child; after
//! the delete, a matching pid can only have been written by that child.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Deserialize;
use tauri::{AppHandle, Manager, Runtime};

use crate::server::ShellError;

/// Mirror of the server-side `PortFile` wire shape (camelCase JSON).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortFile {
    pub port: u16,
    pub pid: u32,
}

/// The data dir the SERVER will use, resolved the same way
/// `spawn_server` decides whether to inject `SPIRITSTREAM_DATA_DIR`:
/// an explicit env override wins (and propagates to the child), else
/// the platform app-local dir. Keeping this in one place means a dev
/// override can never point the shell and the sidecar at different
/// `run/` directories.
pub fn effective_data_dir<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf, ShellError> {
    if let Ok(dir) = std::env::var("SPIRITSTREAM_DATA_DIR") {
        let trimmed = dir.trim();
        if !trimmed.is_empty() {
            return Ok(PathBuf::from(trimmed));
        }
    }
    app.path()
        .app_local_data_dir()
        .map_err(|e| ShellError::DataDir(e.to_string()))
}

fn path_in(data_dir: &Path) -> PathBuf {
    data_dir.join("run").join("server.port")
}

/// Read whatever port file is on disk — used pre-spawn to learn the
/// STALE port (if any) so `kill_stale_sidecar` can verify its release.
pub fn read(data_dir: &Path) -> Option<PortFile> {
    let text = std::fs::read_to_string(path_in(data_dir)).ok()?;
    serde_json::from_str(&text).ok()
}

/// Delete the port file. Called immediately before spawning the
/// sidecar so the subsequent pid-matched poll can't accept a leftover.
pub fn remove(data_dir: &Path) {
    let path = path_in(data_dir);
    if let Err(e) = std::fs::remove_file(&path) {
        if e.kind() != std::io::ErrorKind::NotFound {
            log::warn!("Failed to remove stale {}: {e}", path.display());
        }
    }
}

/// Poll until the server writes a port file recording `expected_pid`,
/// returning the negotiated port. ~10s budget at 100ms intervals —
/// generous against a cold-start server that still has to construct
/// its service registry before binding. A file carrying a DIFFERENT
/// pid is ignored (it can only be a corrupt leftover; the pre-spawn
/// delete removed anything stale).
pub async fn await_with_pid(
    data_dir: &Path,
    expected_pid: u32,
    timeout: Duration,
) -> Result<u16, ShellError> {
    let deadline = std::time::Instant::now() + timeout;
    loop {
        if let Some(record) = read(data_dir) {
            if record.pid == expected_pid {
                return Ok(record.port);
            }
        }
        if std::time::Instant::now() >= deadline {
            return Err(ShellError::PortFile(format!(
                "server (pid {expected_pid}) did not publish {} within {:?} — \
                 the backend likely failed before binding; check its stderr above",
                path_in(data_dir).display(),
                timeout
            )));
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_record(dir: &Path, port: u16, pid: u32) {
        let run = dir.join("run");
        std::fs::create_dir_all(&run).unwrap();
        std::fs::write(
            run.join("server.port"),
            format!("{{\"port\":{port},\"pid\":{pid}}}"),
        )
        .unwrap();
    }

    #[test]
    fn read_parses_and_remove_clears() {
        let dir = tempfile::tempdir().unwrap();
        assert!(read(dir.path()).is_none());
        write_record(dir.path(), 50000, 42);
        let record = read(dir.path()).expect("parsed");
        assert_eq!((record.port, record.pid), (50000, 42));
        remove(dir.path());
        assert!(read(dir.path()).is_none());
        remove(dir.path()); // second removal is a quiet no-op
    }

    #[tokio::test]
    async fn await_with_pid_rejects_wrong_pid_and_times_out() {
        let dir = tempfile::tempdir().unwrap();
        write_record(dir.path(), 50000, 999);
        let err = await_with_pid(dir.path(), 1, Duration::from_millis(300))
            .await
            .expect_err("wrong pid must not satisfy the wait");
        assert!(matches!(err, ShellError::PortFile(_)));
    }

    #[tokio::test]
    async fn await_with_pid_returns_port_once_published() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_path_buf();
        let writer = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(150)).await;
            write_record(&path, 51515, 7);
        });
        let port = await_with_pid(dir.path(), 7, Duration::from_secs(2))
            .await
            .expect("port published");
        assert_eq!(port, 51515);
        writer.await.unwrap();
    }
}
