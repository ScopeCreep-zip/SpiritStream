// LogManager Service
// Handles log retention cleanup

use std::fs;
use std::time::{Duration, SystemTime};
use tauri::{AppHandle, Manager};

pub fn prune_logs(app_handle: &AppHandle, retention_days: u32) -> Result<usize, String> {
    if retention_days == 0 {
        return Ok(0);
    }

    let log_dir = app_handle
        .path()
        .app_log_dir()
        .map_err(|e| format!("Failed to resolve log directory: {e}"))?;
    if !log_dir.exists() {
        return Ok(0);
    }

    let cutoff = SystemTime::now()
        .checked_sub(Duration::from_secs(retention_days as u64 * 24 * 60 * 60))
        .unwrap_or(SystemTime::UNIX_EPOCH);

    let entries = fs::read_dir(&log_dir).map_err(|e| format!("Failed to read log dir: {e}"))?;
    let mut removed = 0;

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("log") {
            continue;
        }

        let modified = entry
            .metadata()
            .and_then(|metadata| metadata.modified())
            .unwrap_or(SystemTime::UNIX_EPOCH);
        if modified < cutoff && fs::remove_file(&path).is_ok() {
            removed += 1;
        }
    }

    Ok(removed)
}
