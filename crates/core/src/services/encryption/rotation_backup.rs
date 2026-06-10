//! Backup / restore / retention helpers for machine-key rotation.
//!
//! Rotation snapshots every profile file into
//! `app_data_dir/profiles_backup/backup_<timestamp>/` before touching
//! the old key, restores that snapshot on any per-profile failure, and
//! keeps the last five snapshots. Startup crash-recovery
//! (`recover_interrupted_rotation`) also restores from the most recent
//! snapshot when a rotation died mid-flight.

use std::path::{Path, PathBuf};

use crate::errors::CoreError;

use super::internal;

pub(super) fn backup_profiles_directory(app_data_dir: &Path) -> Result<PathBuf, CoreError> {
    let profiles_dir = app_data_dir.join("profiles");
    let backup_dir = app_data_dir.join("profiles_backup");
    let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
    let backup_path = backup_dir.join(format!("backup_{timestamp}"));

    log::info!("Creating backup at: {}", backup_path.display());

    std::fs::create_dir_all(&backup_path)
        .map_err(|e| internal("Failed to create backup directory", e))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o700); // Owner only
        std::fs::set_permissions(&backup_dir, perms.clone())
            .map_err(|e| internal("Failed to set backup directory permissions", e))?;
        std::fs::set_permissions(&backup_path, perms)
            .map_err(|e| internal("Failed to set backup directory permissions", e))?;
    }

    let entries = std::fs::read_dir(&profiles_dir)
        .map_err(|e| internal("Failed to read profiles directory", e))?;

    for entry in entries.flatten() {
        let path = entry.path();
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            if ext == "json" || ext == "mgs" {
                if let Some(file_name) = path.file_name() {
                    let dest = backup_path.join(file_name);
                    std::fs::copy(&path, &dest).map_err(|e| {
                        internal(
                            &format!("Failed to backup {}", file_name.to_string_lossy()),
                            e,
                        )
                    })?;
                    log::debug!("Backed up: {}", file_name.to_string_lossy());
                }
            }
        }
    }

    log::info!("Backup created successfully");
    Ok(backup_path)
}

pub(super) fn restore_from_backup(backup_path: &Path, app_data_dir: &Path) -> Result<(), CoreError> {
    let profiles_dir = app_data_dir.join("profiles");

    log::warn!("Restoring from backup: {}", backup_path.display());

    let entries = std::fs::read_dir(&profiles_dir)
        .map_err(|e| internal("Failed to read profiles directory", e))?;

    for entry in entries.flatten() {
        let path = entry.path();
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            if ext == "json" || ext == "mgs" {
                std::fs::remove_file(&path)
                    .map_err(|e| internal(&format!("Failed to delete {}", path.display()), e))?;
            }
        }
    }

    let backup_entries = std::fs::read_dir(backup_path)
        .map_err(|e| internal("Failed to read backup directory", e))?;

    for entry in backup_entries.flatten() {
        let path = entry.path();
        if let Some(file_name) = path.file_name() {
            let dest = profiles_dir.join(file_name);
            std::fs::copy(&path, &dest).map_err(|e| {
                internal(
                    &format!("Failed to restore {}", file_name.to_string_lossy()),
                    e,
                )
            })?;
        }
    }

    log::info!("Backup restored successfully");
    Ok(())
}

/// Most recent rotation backup snapshot, if any. Snapshot directory
/// names embed a UTC timestamp, so lexicographic order is creation
/// order.
pub(super) fn latest_backup(app_data_dir: &Path) -> Option<PathBuf> {
    let backup_dir = app_data_dir.join("profiles_backup");
    let entries = std::fs::read_dir(&backup_dir).ok()?;
    let mut backups: Vec<PathBuf> = entries
        .flatten()
        .filter(|e| e.path().is_dir())
        .map(|e| e.path())
        .collect();
    backups.sort();
    backups.pop()
}

pub(super) fn cleanup_old_backups(app_data_dir: &Path, keep_count: usize) -> Result<(), CoreError> {
    let backup_dir = app_data_dir.join("profiles_backup");

    if !backup_dir.exists() {
        return Ok(());
    }

    let entries = std::fs::read_dir(&backup_dir)
        .map_err(|e| internal("Failed to read backup directory", e))?;

    let mut backups: Vec<PathBuf> = entries
        .flatten()
        .filter(|e| e.path().is_dir())
        .map(|e| e.path())
        .collect();

    backups.sort();

    while backups.len() > keep_count {
        if let Some(oldest) = backups.first() {
            log::info!("Deleting old backup: {}", oldest.display());
            std::fs::remove_dir_all(oldest)
                .map_err(|e| internal("Failed to delete old backup", e))?;
            backups.remove(0);
        }
    }

    Ok(())
}
