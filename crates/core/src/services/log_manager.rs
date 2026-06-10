// LogManager Service
// Handles log retention cleanup and reading

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use crate::errors::CoreError;

pub fn prune_logs(log_dir: &Path, retention_days: u32) -> Result<usize, CoreError> {
    if retention_days == 0 {
        return Ok(0);
    }

    if !log_dir.exists() {
        return Ok(0);
    }

    let cutoff = SystemTime::now()
        .checked_sub(Duration::from_secs(retention_days as u64 * 24 * 60 * 60))
        .unwrap_or(SystemTime::UNIX_EPOCH);

    let entries = fs::read_dir(log_dir).map_err(|e| CoreError::Internal {
        context: format!("Failed to read log dir: {e}"),
    })?;
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

pub fn read_recent_logs(log_dir: &Path, max_lines: usize) -> Result<Vec<String>, CoreError> {
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

fn read_log_lines(path: &Path, max_lines: usize) -> Result<Vec<String>, CoreError> {
    let bytes = fs::read(path).map_err(|e| CoreError::Internal {
        context: format!("Failed to read log file: {e}"),
    })?;
    let content = String::from_utf8_lossy(&bytes);
    let lines: Vec<String> = content
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| line.to_string())
        .collect();

    let start = lines.len().saturating_sub(max_lines);
    Ok(lines[start..].to_vec())
}

#[cfg(test)]
mod tests {
    use super::{prune_logs, read_recent_logs};
    use filetime::{set_file_mtime, FileTime};
    use std::fs;
    use std::time::{Duration, SystemTime};
    use tempfile::TempDir;

    fn backdate(path: &std::path::Path, days_ago: u64) {
        let when = SystemTime::now() - Duration::from_secs(days_ago * 24 * 60 * 60);
        set_file_mtime(path, FileTime::from_system_time(when)).expect("set mtime");
    }

    #[test]
    fn prune_with_zero_retention_is_a_noop() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("a.log"), b"x").unwrap();
        assert_eq!(prune_logs(dir.path(), 0).unwrap(), 0);
        assert!(dir.path().join("a.log").exists());
    }

    #[test]
    fn prune_missing_dir_is_a_noop() {
        let dir = TempDir::new().unwrap();
        let missing = dir.path().join("does-not-exist");
        assert_eq!(prune_logs(&missing, 7).unwrap(), 0);
    }

    #[test]
    fn prune_removes_only_old_log_files() {
        let dir = TempDir::new().unwrap();
        let old = dir.path().join("old.log");
        let fresh = dir.path().join("fresh.log");
        let other = dir.path().join("keep.txt");
        fs::write(&old, b"old").unwrap();
        fs::write(&fresh, b"fresh").unwrap();
        fs::write(&other, b"not a log").unwrap();
        backdate(&old, 30);
        backdate(&other, 30);

        let removed = prune_logs(dir.path(), 7).unwrap();
        assert_eq!(removed, 1);
        assert!(!old.exists(), "stale .log should be pruned");
        assert!(fresh.exists(), "recent .log should be kept");
        assert!(other.exists(), "non-.log files are never pruned");
    }

    #[test]
    fn read_recent_logs_empty_when_no_log_files() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("notes.txt"), b"ignored").unwrap();
        assert!(read_recent_logs(dir.path(), 10).unwrap().is_empty());
    }

    #[test]
    fn read_recent_logs_returns_tail_of_latest_file_skipping_blanks() {
        let dir = TempDir::new().unwrap();
        let older = dir.path().join("older.log");
        let newer = dir.path().join("newer.log");
        fs::write(&older, "should-not-appear\n").unwrap();
        fs::write(&newer, "l1\n\n   \nl2\nl3\n").unwrap();
        backdate(&older, 5);

        let tail = read_recent_logs(dir.path(), 2).unwrap();
        assert_eq!(tail, vec!["l2".to_string(), "l3".to_string()]);
    }
}
