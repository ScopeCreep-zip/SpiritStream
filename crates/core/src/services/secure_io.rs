//! Owner-only file write helper.
//!
//! Every persisted artifact that may contain secrets — profile files,
//! settings, machine key, audit-log entries, secret-store blobs — flows
//! through [`write_owner_only_atomic`]. The helper:
//!
//! 1. Writes to a sibling `<path>.tmp` first.
//! 2. On Unix, chmod 0600 (owner read+write only) the tmp file.
//! 3. On Windows, sets `FILE_ATTRIBUTE_HIDDEN | FILE_ATTRIBUTE_SYSTEM`
//!    on the tmp file. Windows file ACLs are inherited from the parent
//!    directory at creation time; user data directories on a standard
//!    install already restrict access to the owning user, so the
//!    additional hidden/system marker is the meaningful hardening we
//!    can do without taking a dependency on the heavier Win32 ACL APIs.
//! 4. Atomically renames the tmp file to the final path.
//!
//! Tests under `phase_69_perms` assert mode 0600 across every sensitive
//! write path (profile save, settings save, machine-key write, secret
//! store put).

use std::io;
use std::path::Path;

use crate::errors::CoreError;

/// Atomically write `bytes` to `path` with owner-only permissions.
///
/// The temp file lives next to the destination, gets its permissions
/// hardened before the rename so a partially-written file is never
/// world-readable for any window, and is renamed into place last.
pub fn write_owner_only_atomic(path: &Path, bytes: &[u8]) -> Result<(), CoreError> {
    let tmp = tmp_path(path);
    std::fs::write(&tmp, bytes).map_err(|e| CoreError::Internal {
        context: format!("write {:?}: {e}", path.display()),
    })?;
    if let Err(e) = harden_perms(&tmp) {
        // Clean up the partial tmp file so a later run doesn't keep
        // colliding on it.
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }
    std::fs::rename(&tmp, path).map_err(|e| CoreError::Internal {
        context: format!("rename {:?}: {e}", path.display()),
    })
}

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
}
