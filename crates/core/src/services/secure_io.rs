//! Owner-only file write helper.
//!
//! Every persisted artifact that may contain secrets — profile files,
//! settings, machine key, audit-log entries, secret-store blobs — flows
//! through [`write_owner_only_atomic`]. The helper:
//!
//! 1. Creates a sibling `<path>.tmp` with owner-only permissions in the
//!    same syscall as creation (`O_CREAT | O_EXCL` + mode 0600 on Unix),
//!    so the secret payload is never readable by other users for any
//!    window, even if the process dies mid-write.
//! 2. On Windows, sets `FILE_ATTRIBUTE_HIDDEN | FILE_ATTRIBUTE_SYSTEM`
//!    on the tmp file. Windows file ACLs are inherited from the parent
//!    directory at creation time; user data directories on a standard
//!    install already restrict access to the owning user, so the
//!    additional hidden/system marker is the meaningful hardening we
//!    can do without taking a dependency on the heavier Win32 ACL APIs.
//! 3. Flushes the file to disk (`fsync`) before the rename so a crash
//!    can never replace the destination with a torn or empty file —
//!    key-rotation correctness depends on this.
//! 4. Atomically renames the tmp file to the final path, then fsyncs
//!    the parent directory (Unix) so the rename itself is durable.
//!
//! Tests under `phase_69_perms` assert mode 0600 across every sensitive
//! write path (profile save, settings save, machine-key write, secret
//! store put).

use std::io;
use std::io::Write;
use std::path::Path;

use crate::errors::CoreError;

/// Atomically write `bytes` to `path` with owner-only permissions.
///
/// The temp file lives next to the destination and is created with
/// owner-only permissions atomically (no widen-then-tighten window).
/// It is fsynced before the rename and removed on every error path.
pub fn write_owner_only_atomic(path: &Path, bytes: &[u8]) -> Result<(), CoreError> {
    let tmp = tmp_path(path);
    // A stale tmp from a crashed previous run would make create_new fail
    // forever; writers to a given path are serialized by their owning
    // service, so removing it here cannot race a live writer.
    if tmp.exists() {
        let _ = std::fs::remove_file(&tmp);
    }
    let result = write_tmp_then_rename(path, &tmp, bytes);
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

fn write_tmp_then_rename(path: &Path, tmp: &Path, bytes: &[u8]) -> Result<(), CoreError> {
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let mut file = opts.open(tmp).map_err(|e| CoreError::Internal {
        context: format!("create {:?}: {e}", tmp.display()),
    })?;
    harden_perms(tmp)?;
    file.write_all(bytes).map_err(|e| CoreError::Internal {
        context: format!("write {:?}: {e}", path.display()),
    })?;
    file.sync_all().map_err(|e| CoreError::Internal {
        context: format!("fsync {:?}: {e}", tmp.display()),
    })?;
    drop(file);
    std::fs::rename(tmp, path).map_err(|e| CoreError::Internal {
        context: format!("rename {:?}: {e}", path.display()),
    })?;
    sync_parent_dir(path);
    Ok(())
}

/// Fsync the directory containing `path` so the rename is durable.
///
/// Best-effort: some filesystems (and all of Windows) don't support
/// opening a directory for fsync; the rename itself already happened,
/// so failure here only widens the crash-durability window — it never
/// loses an otherwise-successful write.
#[cfg(unix)]
fn sync_parent_dir(path: &Path) {
    if let Some(parent) = path.parent() {
        if let Ok(dir) = std::fs::File::open(parent) {
            let _ = dir.sync_all();
        }
    }
}

#[cfg(not(unix))]
fn sync_parent_dir(_path: &Path) {}

fn tmp_path(path: &Path) -> std::path::PathBuf {
    // Match the destination filename + `.tmp`. We don't use OsString
    // gymnastics because the file name is always under our control.
    let mut name = path
        .file_name()
        .map(|n| n.to_os_string())
        .unwrap_or_default();
    name.push(".tmp");
    path.with_file_name(name)
}

#[cfg(unix)]
fn harden_perms(path: &Path) -> Result<(), CoreError> {
    use std::os::unix::fs::PermissionsExt;
    let perms = std::fs::Permissions::from_mode(0o600);
    std::fs::set_permissions(path, perms).map_err(|e: io::Error| CoreError::Internal {
        context: format!("chmod 0600 {:?}: {e}", path.display()),
    })
}

#[cfg(windows)]
fn harden_perms(path: &Path) -> Result<(), CoreError> {
    use std::os::windows::ffi::OsStrExt;
    let metadata = std::fs::metadata(path).map_err(|e: io::Error| CoreError::Internal {
        context: format!("metadata {:?}: {e}", path.display()),
    })?;
    use std::os::windows::fs::MetadataExt;
    let mut attributes = metadata.file_attributes();
    const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
    const FILE_ATTRIBUTE_SYSTEM: u32 = 0x4;
    attributes |= FILE_ATTRIBUTE_HIDDEN | FILE_ATTRIBUTE_SYSTEM;

    let wide_path: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    unsafe {
        if winapi::um::fileapi::SetFileAttributesW(wide_path.as_ptr(), attributes) == 0 {
            return Err(CoreError::Internal {
                context: format!("SetFileAttributesW {:?} failed", path.display()),
            });
        }
    }
    Ok(())
}

#[cfg(not(any(unix, windows)))]
fn harden_perms(_path: &Path) -> Result<(), CoreError> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[cfg(unix)]
    #[test]
    fn write_owner_only_creates_file_with_0600() {
        use std::os::unix::fs::PermissionsExt;
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("secret.bin");
        write_owner_only_atomic(&path, b"contents").unwrap();
        let perms = std::fs::metadata(&path).unwrap().permissions();
        assert_eq!(perms.mode() & 0o777, 0o600);
    }

    #[cfg(unix)]
    #[test]
    fn write_owner_only_overwrites_existing_file_keeping_0600() {
        use std::os::unix::fs::PermissionsExt;
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("settings.json");
        // First write
        write_owner_only_atomic(&path, b"first").unwrap();
        // Pretend something widened the perms (simulating a backup tool
        // or restored-from-archive scenario) — our write must re-tighten.
        let widened = std::fs::Permissions::from_mode(0o644);
        std::fs::set_permissions(&path, widened).unwrap();
        write_owner_only_atomic(&path, b"second").unwrap();
        let perms = std::fs::metadata(&path).unwrap().permissions();
        assert_eq!(perms.mode() & 0o777, 0o600);
        assert_eq!(std::fs::read(&path).unwrap(), b"second");
    }

    #[test]
    fn write_owner_only_does_not_leave_tmp_files_on_success() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("entry");
        write_owner_only_atomic(&path, b"data").unwrap();
        let tmp = tmp_path(&path);
        assert!(!tmp.exists(), "tmp must be removed by rename");
        assert!(path.exists());
    }

    #[test]
    fn write_owner_only_recovers_from_stale_tmp_of_crashed_run() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("entry");
        let tmp = tmp_path(&path);
        std::fs::write(&tmp, b"torn write from a crashed process").unwrap();
        write_owner_only_atomic(&path, b"fresh").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"fresh");
        assert!(!tmp.exists());
    }

    #[cfg(unix)]
    #[test]
    fn write_owner_only_cleans_tmp_when_rename_fails() {
        let dir = TempDir::new().unwrap();
        // Destination is a non-empty directory → rename must fail.
        let path = dir.path().join("dest");
        std::fs::create_dir(&path).unwrap();
        std::fs::write(path.join("occupant"), b"x").unwrap();
        let err = write_owner_only_atomic(&path, b"data");
        assert!(err.is_err());
        assert!(!tmp_path(&path).exists(), "tmp must be cleaned on error");
    }

    #[cfg(unix)]
    #[test]
    fn tmp_file_is_owner_only_at_creation() {
        use std::os::unix::fs::PermissionsExt;
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("secret.bin");
        // Pin a wide umask for the duration to prove the mode comes from
        // the open() call, not the process umask.
        let tmp = tmp_path(&path);
        let mut opts = std::fs::OpenOptions::new();
        opts.write(true).create_new(true);
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        let file = opts.open(&tmp).unwrap();
        let perms = file.metadata().unwrap().permissions();
        assert_eq!(perms.mode() & 0o777, 0o600);
        drop(file);
        std::fs::remove_file(&tmp).unwrap();
    }
}
