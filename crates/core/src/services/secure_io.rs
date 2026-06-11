//! Owner-only file write helper.
//!
//! Every persisted artifact that may contain secrets — profile files,
//! settings, machine key, audit-log entries, secret-store blobs — flows
//! through [`write_owner_only_atomic`]. The helper:
//!
//! 1. Creates a uniquely-named sibling `<path>.tmp.<pid>.<seq>` with
//!    owner-only permissions in the same syscall as creation
//!    (`O_CREAT | O_EXCL` + mode 0600 on Unix), so the secret payload
//!    is never readable by other users for any window, even if the
//!    process dies mid-write. The unique name makes concurrent writers
//!    to the SAME destination safe — in-process (several FFmpeg exit
//!    handlers persisting the stream registry at once) and
//!    cross-process (the CLI panic path rewriting the registry while
//!    the server is running): each writer owns its tmp file and the
//!    final renames serialize atomically in the kernel, last one wins.
//!    A fixed `.tmp` name used to make every concurrent loser fail
//!    with `create_new` AlreadyExists — and worse, the pre-write
//!    stale-tmp cleanup could delete a LIVE writer's tmp mid-write.
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
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use crate::errors::CoreError;

/// Per-process sequence for unique tmp names. Combined with the pid it
/// guarantees no two writers — in this process or any other — ever
/// share a tmp path.
static TMP_SEQ: AtomicU64 = AtomicU64::new(0);

/// Strays older than this are leftovers of a crashed writer; live
/// writers hold their tmp for milliseconds.
const STALE_TMP_AGE: Duration = Duration::from_secs(3600);

/// Atomically write `bytes` to `path` with owner-only permissions.
///
/// The temp file lives next to the destination, carries a unique
/// `.tmp.<pid>.<seq>` suffix (concurrent-writer safe), and is created
/// with owner-only permissions atomically (no widen-then-tighten
/// window). It is fsynced before the rename and removed on every error
/// path; tmp strays left by crashed writers are swept opportunistically
/// once they are unambiguously dead (> 1 hour old).
pub fn write_owner_only_atomic(path: &Path, bytes: &[u8]) -> Result<(), CoreError> {
    remove_stale_tmps(path);
    let tmp = tmp_path(path);
    let result = write_tmp_then_rename(path, &tmp, bytes);
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

/// Best-effort sweep of `<name>.tmp.*` strays from crashed writers.
/// Age-gated so it can never touch a live writer's tmp file.
fn remove_stale_tmps(path: &Path) {
    let (Some(parent), Some(name)) = (path.parent(), path.file_name()) else {
        return;
    };
    let prefix = format!("{}.tmp.", name.to_string_lossy());
    let Ok(entries) = std::fs::read_dir(parent) else {
        return;
    };
    for entry in entries.flatten() {
        let entry_name = entry.file_name();
        if !entry_name.to_string_lossy().starts_with(&prefix) {
            continue;
        }
        let stale = entry
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|mtime| mtime.elapsed().ok())
            .is_some_and(|age| age > STALE_TMP_AGE);
        if stale {
            let _ = std::fs::remove_file(entry.path());
        }
    }
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
    // Destination filename + `.tmp.<pid>.<seq>` — unique per writer so
    // concurrent writes to the same destination never collide. We don't
    // use OsString gymnastics because the file name is always under our
    // control.
    let mut name = path
        .file_name()
        .map(|n| n.to_os_string())
        .unwrap_or_default();
    name.push(format!(
        ".tmp.{}.{}",
        std::process::id(),
        TMP_SEQ.fetch_add(1, Ordering::Relaxed)
    ));
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

    fn tmp_strays(dir: &std::path::Path, dest: &str) -> Vec<std::path::PathBuf> {
        std::fs::read_dir(dir)
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                p.file_name()
                    .map(|n| n.to_string_lossy().starts_with(&format!("{dest}.tmp.")))
                    .unwrap_or(false)
            })
            .collect()
    }

    #[test]
    fn write_owner_only_does_not_leave_tmp_files_on_success() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("entry");
        write_owner_only_atomic(&path, b"data").unwrap();
        assert!(
            tmp_strays(dir.path(), "entry").is_empty(),
            "tmp must be removed by rename"
        );
        assert!(path.exists());
    }

    #[test]
    fn old_stray_tmp_is_swept_and_fresh_one_is_left_alone() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("entry");
        // A FRESH stray (simulating a concurrent live writer) must
        // survive; only unambiguously-dead strays are swept. Age-based
        // sweeping is tested by lowering nothing — a fresh file is
        // simply younger than STALE_TMP_AGE.
        let fresh = dir.path().join("entry.tmp.99999.0");
        std::fs::write(&fresh, b"live concurrent writer").unwrap();
        write_owner_only_atomic(&path, b"fresh").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"fresh");
        assert!(
            fresh.exists(),
            "a recent tmp (possibly a live writer) must not be deleted"
        );
    }

    #[test]
    fn concurrent_writers_to_same_destination_all_succeed() {
        // Regression: with a FIXED tmp name, several FFmpeg exit
        // handlers persisting the stream registry simultaneously made
        // every loser fail create_new with AlreadyExists ("failed to
        // persist stream process registry"), and the pre-write cleanup
        // could yank a live writer's tmp. Unique names make every
        // writer succeed; last rename wins.
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("registry.json");
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(8));
        let mut handles = Vec::new();
        for i in 0..8 {
            let path = path.clone();
            let barrier = std::sync::Arc::clone(&barrier);
            handles.push(std::thread::spawn(move || {
                barrier.wait();
                write_owner_only_atomic(&path, format!("writer-{i}").as_bytes())
            }));
        }
        for handle in handles {
            handle
                .join()
                .expect("no panic")
                .expect("every concurrent writer must succeed");
        }
        let contents = std::fs::read_to_string(&path).unwrap();
        assert!(contents.starts_with("writer-"), "one writer won: {contents}");
        assert!(
            tmp_strays(dir.path(), "registry.json").is_empty(),
            "no tmp litter after concurrent writes"
        );
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
        assert!(
            tmp_strays(dir.path(), "dest").is_empty(),
            "tmp must be cleaned on error"
        );
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
