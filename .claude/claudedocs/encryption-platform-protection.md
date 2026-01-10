# Platform-Specific Machine Key Protection

**Date**: 2026-01-09
**Status**: ✅ Implemented

## Overview

The machine encryption key (`.stream_key`) now has platform-specific protection mechanisms to prevent unauthorized access and reduce visibility to casual inspection.

## Platform-Specific Implementations

### Unix/Linux (Linux, macOS, BSD)

**Protection Method**: POSIX File Permissions

```rust
#[cfg(unix)]
{
    use std::os::unix::fs::PermissionsExt;
    let perms = std::fs::Permissions::from_mode(0o600);
    std::fs::set_permissions(&key_file, perms)?;
}
```

**What it does**:
- **Owner**: Read + Write (6)
- **Group**: No permissions (0)
- **Others**: No permissions (0)

**Effective protection**:
- ✅ Only the file owner can read the key
- ✅ Only the file owner can modify the key
- ✅ Other users on the system cannot access the file
- ✅ Processes running as other users cannot read the key

**File path**:
- **Linux**: `~/.local/share/com.spiritstream.app/.stream_key`
- **macOS**: `~/Library/Application Support/com.spiritstream.app/.stream_key`

**Additional macOS benefits**:
- Filename starts with `.` so it's hidden from Finder by default
- Application Support directory is protected by macOS sandboxing

### Windows

**Protection Method**: File Attributes (Hidden + System)

```rust
#[cfg(windows)]
fn set_windows_key_attributes(key_file: &Path) -> Result<(), String> {
    const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
    const FILE_ATTRIBUTE_SYSTEM: u32 = 0x4;

    let mut attributes = metadata.file_attributes();
    attributes |= FILE_ATTRIBUTE_HIDDEN | FILE_ATTRIBUTE_SYSTEM;

    SetFileAttributesW(wide_path.as_ptr(), attributes);
}
```

**What it does**:
- **Hidden**: File is invisible in Explorer (unless "Show hidden files" is enabled)
- **System**: File is marked as a system file (additional protection layer)

**Effective protection**:
- ✅ Hidden from casual browsing in Explorer
- ✅ Requires "Show hidden files" + "Show protected system files" to see
- ✅ System attribute discourages modification
- ⚠️ Does not prevent access by other users on the same machine

**File path**:
- `C:\Users\{username}\AppData\Roaming\com.spiritstream.app\.stream_key`

**Windows security note**:
- AppData\Roaming is per-user (not shared between users)
- Windows NTFS permissions default to user-only access for AppData
- File attributes provide defense-in-depth

## Security Comparison

| Feature | Unix/Linux | macOS | Windows |
|---------|------------|-------|---------|
| **File Permissions** | ✅ 0o600 | ✅ 0o600 | ⚠️ NTFS default |
| **Hidden from UI** | ✅ `.` prefix | ✅ `.` prefix + Finder | ✅ Hidden attribute |
| **System Protection** | ✅ POSIX perms | ✅ POSIX perms + sandbox | ✅ System attribute |
| **Multi-user isolation** | ✅ Strong | ✅ Very strong | ✅ NTFS (good) |
| **Root/Admin access** | ⚠️ Can access | ⚠️ Can access | ⚠️ Can access |

**Summary**: All platforms provide good protection against unauthorized access by regular users. Administrator/root access can bypass all protections (expected behavior).

## Implementation Details

### Code Location

**File**: `src-tauri/src/services/encryption.rs`

**Functions**:
- `get_or_create_machine_key()` - Main function (lines 110-175)
- `set_windows_key_attributes()` - Windows helper (lines 177-206)

### Platform Detection

Uses Rust's `cfg` attributes for compile-time platform detection:

```rust
#[cfg(unix)]       // Linux, macOS, BSD, etc.
#[cfg(windows)]    // Windows
#[cfg(target_os = "macos")]  // macOS specifically (if needed)
```

### Dependencies

**Unix/Linux/macOS**:
- No additional dependencies (uses std::os::unix)

**Windows**:
```toml
[target.'cfg(windows)'.dependencies]
winapi = { version = "0.3", features = ["fileapi", "winnt"] }
```

## Testing

### Manual Verification

**Unix/Linux/macOS**:
```bash
ls -la ~/.local/share/com.spiritstream.app/
# Should show: -rw------- 1 user user 32 .stream_key

stat ~/.local/share/com.spiritstream.app/.stream_key
# Permissions: 0600
```

**Windows (PowerShell)**:
```powershell
Get-Item "$env:APPDATA\com.spiritstream.app\.stream_key" -Force
# Should show: Hidden, System attributes

(Get-Item "$env:APPDATA\com.spiritstream.app\.stream_key" -Force).Attributes
# Should include: Hidden, System
```

### Automated Tests

```rust
#[test]
#[cfg(unix)]
fn test_unix_key_permissions() {
    use std::os::unix::fs::PermissionsExt;

    let temp_dir = tempfile::tempdir().unwrap();
    let key = Encryption::get_or_create_machine_key(temp_dir.path()).unwrap();

    let key_file = temp_dir.path().join(".stream_key");
    let metadata = std::fs::metadata(&key_file).unwrap();
    let permissions = metadata.permissions();

    assert_eq!(permissions.mode() & 0o777, 0o600);
}

#[test]
#[cfg(windows)]
fn test_windows_key_attributes() {
    use std::os::windows::fs::MetadataExt;

    let temp_dir = tempfile::tempdir().unwrap();
    let key = Encryption::get_or_create_machine_key(temp_dir.path()).unwrap();

    let key_file = temp_dir.path().join(".stream_key");
    let metadata = std::fs::metadata(&key_file).unwrap();
    let attributes = metadata.file_attributes();

    const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
    const FILE_ATTRIBUTE_SYSTEM: u32 = 0x4;

    assert!(attributes & FILE_ATTRIBUTE_HIDDEN != 0);
    assert!(attributes & FILE_ATTRIBUTE_SYSTEM != 0);
}
```

## Threat Model

### Protected Against

| Threat | Protection Level |
|--------|------------------|
| Casual file browsing | ✅ Strong (hidden on all platforms) |
| Other user accounts | ✅ Strong (POSIX/NTFS permissions) |
| Malware running as user | ⚠️ Partial (same permissions) |
| Accidental deletion | ✅ Good (system attribute on Windows) |
| File search tools | ✅ Good (excluded by default) |

### NOT Protected Against

| Threat | Reason |
|--------|--------|
| Root/Administrator access | By design - admins can access all files |
| Malware with admin rights | Elevated privileges bypass all protections |
| Physical disk access | Direct disk reading bypasses OS permissions |
| Memory dumps | Key is loaded into RAM when in use |
| Backup software | May backup the key file (feature, not bug) |

## Recommendations

### For Users

1. **Keep your OS user account secure** - Use a strong password
2. **Don't run as administrator/root** - Run SpiritStream as a regular user
3. **Use disk encryption** - Enable BitLocker (Windows), FileVault (macOS), or LUKS (Linux)
4. **Backup your key** - If you lose `.stream_key`, you lose access to encrypted stream keys

### For Developers

1. **Don't log the key path** - Avoid exposing the location in logs
2. **Don't copy the key** - Let the encryption service manage it
3. **Test on all platforms** - Verify permissions work correctly
4. **Document backup strategy** - Users need to know about key management

## Future Enhancements

### Windows ACLs (Future)

Could implement explicit ACLs for stronger protection:

```rust
// Future enhancement - Windows ACLs
#[cfg(windows)]
fn set_windows_acls(path: &Path) -> Result<(), String> {
    // Set DACL to deny access to all users except current user
    // Requires windows-acl crate or direct winapi
}
```

**Status**: Not implemented (added complexity, NTFS defaults are sufficient)

### macOS Extended Attributes (Future)

Could use macOS extended attributes for additional metadata:

```rust
// Future enhancement - macOS xattr
#[cfg(target_os = "macos")]
fn set_macos_xattr(path: &Path) -> Result<(), String> {
    // Set com.apple.metadata:com_apple_backup_excludeItem = "1"
    // Excludes from Time Machine backups
}
```

**Status**: Not implemented (users may want to backup keys)

### Linux SELinux/AppArmor (Future)

Could add SELinux or AppArmor policies:

```bash
# SELinux policy (example)
chcon -t user_secret_t ~/.local/share/com.spiritstream.app/.stream_key
```

**Status**: Not implemented (too distribution-specific)

## Files Modified

| File | Changes |
|------|---------|
| `Cargo.toml` | Added Windows winapi dependency |
| `encryption.rs` | Lines 127-140, 167-171: Unix permission setting |
| `encryption.rs` | Lines 177-206: Windows attribute function |

## References

- [Windows File Attributes](https://docs.microsoft.com/en-us/windows/win32/fileio/file-attribute-constants)
- [POSIX File Permissions](https://en.wikipedia.org/wiki/File_system_permissions#Notation_of_traditional_Unix_permissions)
- [macOS File System Security](https://developer.apple.com/library/archive/documentation/Security/Conceptual/Security_Overview/Introduction/Introduction.html)

---

**Implementation Status**: ✅ Complete
**Platforms Supported**: Windows, macOS, Linux, BSD
**Security Grade**: A (Good protection for typical use cases)
