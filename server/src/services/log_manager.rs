// LogManager Service
// Handles log retention cleanup and reading

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

pub fn prune_logs(log_dir: &Path, retention_days: u32) -> Result<usize, String> {
    if retention_days == 0 {
        return Ok(0);
    }

    if !log_dir.exists() {
        return Ok(0);
    }

    let cutoff = SystemTime::now()
        .checked_sub(Duration::from_secs(retention_days as u64 * 24 * 60 * 60))
        .unwrap_or(SystemTime::UNIX_EPOCH);

    let entries = fs::read_dir(log_dir).map_err(|e| format!("Failed to read log dir: {e}"))?;
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

pub fn read_recent_logs(log_dir: &Path, max_lines: usize) -> Result<Vec<String>, String> {
    let log_file = match find_latest_log_file(log_dir) {
        Some(path) => path,
        None => return Ok(Vec::new()),
    };

    read_log_lines(&log_file, max_lines)
}

fn find_latest_log_file(log_dir: &Path) -> Option<PathBuf> {
    let entries = fs::read_dir(log_dir).ok()?;
    let mut latest: Option<(PathBuf, std::time::SystemTime)> = None;

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("log") {
            continue;
        }
        let modified = entry
            .metadata()
            .and_then(|metadata| metadata.modified())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);

        match latest {
            Some((_, latest_time)) if modified <= latest_time => {}
            _ => latest = Some((path, modified)),
        }
    }

    latest.map(|(path, _)| path)
}

fn read_log_lines(path: &Path, max_lines: usize) -> Result<Vec<String>, String> {
    let bytes = fs::read(path).map_err(|e| format!("Failed to read log file: {e}"))?;
    let content = String::from_utf8_lossy(&bytes);
    let lines: Vec<String> = content
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| line.to_string())
        .collect();

    let start = lines.len().saturating_sub(max_lines);
    Ok(lines[start..].to_vec())
}
