# Machine Key Rotation - Implementation Plan

**Date**: 2026-01-10
**Status**: 📋 Planning Phase
**Priority**: Medium
**Related Docs**:
- `encryption-zeroization.md`
- `encryption-argon2id-strengthening.md`
- `encryption-platform-protection.md`

## Overview

Allow users to rotate the machine encryption key used for encrypting stream keys at rest. This is a critical security feature that enables:
- Recovery from potential key compromise
- Periodic security maintenance
- Compliance with key rotation policies

## Current System Analysis

### How Machine Keys Work Now

**Key Location**: `$APP_DATA/.stream_key`
- **Unix/Linux**: `~/.local/share/com.spiritstream.app/.stream_key`
- **macOS**: `~/Library/Application Support/com.spiritstream.app/.stream_key`
- **Windows**: `%APPDATA%\com.spiritstream.app\.stream_key`

**Key Generation**:
```rust
let key = Zeroizing::new(rng.gen::<[u8; KEY_LEN]>());  // 32 random bytes
```

**Key Usage**: Encrypts/decrypts stream keys in profiles with prefix `ENC::{base64}`

**Key Protection**:
- Unix/macOS: 0o600 permissions (owner read/write only)
- Windows: Hidden + System attributes

### What Needs to Change

Every stream key in every profile that uses the setting `encrypt_stream_keys: true` will need to be:
1. Decrypted with the **old** machine key
2. Re-encrypted with the **new** machine key
3. Saved back to the profile file

---

## 1. User Flow

### Trigger Point

**Settings View** - New section in "Data & Privacy" card:

```
┌─────────────────────────────────────────────────────┐
│ Data & Privacy                                      │
├─────────────────────────────────────────────────────┤
│                                                     │
│ Profile Storage                                     │
│ ┌─────────────────────────────┐ [Open Folder]      │
│ │ C:\Users\...\profiles       │                     │
│ └─────────────────────────────┘                     │
│                                                     │
│ ☑ Encrypt Stream Keys                              │
│                                                     │
│ Machine Key                                         │
│ Last Rotated: 2026-01-05 14:23:15                  │
│ [Rotate Machine Key]                                │
│                                                     │
│ Export Data | Clear All Data                        │
└─────────────────────────────────────────────────────┘
```

### User Journey

1. **User clicks "Rotate Machine Key" button**
2. **Confirmation modal appears**:
   ```
   ┌────────────────────────────────────────────────────┐
   │ Rotate Machine Key?                          [×]   │
   ├────────────────────────────────────────────────────┤
   │                                                    │
   │ This will:                                         │
   │ • Generate a new encryption key                    │
   │ • Re-encrypt all stream keys in all profiles       │
   │ • Securely delete the old key                      │
   │                                                    │
   │ ⚠️ WARNING:                                         │
   │ • All profiles will be updated                     │
   │ • This cannot be undone                            │
   │ • Ensure no streams are currently active           │
   │                                                    │
   │ Profiles to update: 5                              │
   │ Stream keys to re-encrypt: 12                      │
   │                                                    │
   │ [Cancel]                   [Rotate Key]            │
   └────────────────────────────────────────────────────┘
   ```

3. **Pre-flight checks** (automated):
   - ✅ No active streams
   - ✅ All profiles loadable
   - ✅ Write permissions on profile directory
   - ✅ Sufficient disk space

4. **Progress modal** (non-dismissable):
   ```
   ┌────────────────────────────────────────────────────┐
   │ Rotating Machine Key...                      [—]   │
   ├────────────────────────────────────────────────────┤
   │                                                    │
   │ ⏳ Generating new key...                   ✓ Done  │
   │ ⏳ Creating backup...                      ✓ Done  │
   │ ⏳ Re-encrypting stream keys...            3/12    │
   │                                                    │
   │ ████████████░░░░░░░░░░░░░░░░░░░░░░░░ 25%          │
   │                                                    │
   │ Please wait, do not close the application...      │
   └────────────────────────────────────────────────────┘
   ```

5. **Success confirmation**:
   ```
   ┌────────────────────────────────────────────────────┐
   │ Key Rotation Complete                        [×]   │
   ├────────────────────────────────────────────────────┤
   │                                                    │
   │ ✅ Machine key successfully rotated                │
   │                                                    │
   │ • 5 profiles updated                               │
   │ • 12 stream keys re-encrypted                      │
   │ • Old key securely deleted                         │
   │ • Backup saved to: profiles/.backup/               │
   │                                                    │
   │ [OK]                                               │
   └────────────────────────────────────────────────────┘
   ```

6. **Error handling** (if any step fails):
   ```
   ┌────────────────────────────────────────────────────┐
   │ Key Rotation Failed                          [×]   │
   ├────────────────────────────────────────────────────┤
   │                                                    │
   │ ❌ Rotation failed during profile update           │
   │                                                    │
   │ Error: Failed to decrypt stream key in profile     │
   │        "Gaming Stream.json"                        │
   │                                                    │
   │ Your profiles have been restored from backup.      │
   │ No changes were made.                              │
   │                                                    │
   │ [View Logs]                       [OK]             │
   └────────────────────────────────────────────────────┘
   ```

---

## 2. Technical Approach

### High-Level Algorithm

```rust
pub fn rotate_machine_key(app_data_dir: &Path) -> Result<RotationReport, String> {
    // 1. Pre-flight checks
    check_no_active_streams()?;
    check_write_permissions(app_data_dir)?;

    // 2. Load old key
    let old_key = get_or_create_machine_key(app_data_dir)?;

    // 3. Generate new key
    let new_key = generate_new_machine_key()?;

    // 4. Create backup
    backup_profiles_directory(app_data_dir)?;

    // 5. Get all profiles
    let profile_names = get_all_profile_names(app_data_dir)?;

    // 6. Re-encrypt each profile (transactional)
    let mut updated_profiles = Vec::new();
    for profile_name in profile_names {
        match reencrypt_profile(&profile_name, &old_key, &new_key, app_data_dir) {
            Ok(stats) => updated_profiles.push(stats),
            Err(e) => {
                // ROLLBACK: Restore from backup
                restore_from_backup(app_data_dir)?;
                return Err(format!("Failed to update {}: {}. Restored from backup.", profile_name, e));
            }
        }
    }

    // 7. Write new key to disk
    write_new_machine_key(&new_key, app_data_dir)?;

    // 8. Zeroize old key
    drop(old_key); // Automatic zeroization via Zeroizing<T>

    // 9. Update metadata
    update_rotation_metadata(app_data_dir)?;

    // 10. Return report
    Ok(RotationReport {
        profiles_updated: updated_profiles.len(),
        keys_reencrypted: updated_profiles.iter().map(|s| s.keys_updated).sum(),
        timestamp: chrono::Utc::now(),
    })
}
```

### Key Functions

#### `reencrypt_profile()`
```rust
fn reencrypt_profile(
    profile_name: &str,
    old_key: &Zeroizing<[u8; KEY_LEN]>,
    new_key: &Zeroizing<[u8; KEY_LEN]>,
    app_data_dir: &Path,
) -> Result<ProfileUpdateStats, String> {
    // 1. Load profile
    let mut profile = load_profile_raw(profile_name, app_data_dir)?;

    // 2. Track changes
    let mut keys_updated = 0;

    // 3. For each output group
    for group in &mut profile.output_groups {
        // 4. For each stream target
        for target in &mut group.stream_targets {
            // 5. If stream key is encrypted
            if is_stream_key_encrypted(&target.stream_key) {
                // 6. Decrypt with old key
                let plaintext = decrypt_with_key(&target.stream_key, old_key)?;

                // 7. Re-encrypt with new key
                target.stream_key = encrypt_with_key(&plaintext, new_key)?;

                keys_updated += 1;
            }
        }
    }

    // 8. Save updated profile
    save_profile_raw(&profile, app_data_dir)?;

    Ok(ProfileUpdateStats {
        profile_name: profile_name.to_string(),
        keys_updated,
    })
}
```

#### `backup_profiles_directory()`
```rust
fn backup_profiles_directory(app_data_dir: &Path) -> Result<PathBuf, String> {
    let profiles_dir = app_data_dir.join("profiles");
    let backup_dir = app_data_dir.join("profiles_backup");
    let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
    let backup_path = backup_dir.join(format!("backup_{}", timestamp));

    // Create backup directory
    fs::create_dir_all(&backup_path)
        .map_err(|e| format!("Failed to create backup directory: {}", e))?;

    // Copy all .json files
    for entry in fs::read_dir(&profiles_dir)
        .map_err(|e| format!("Failed to read profiles directory: {}", e))?
    {
        let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
        let path = entry.path();

        if path.extension().and_then(|s| s.to_str()) == Some("json") {
            let file_name = path.file_name().unwrap();
            let dest = backup_path.join(file_name);
            fs::copy(&path, &dest)
                .map_err(|e| format!("Failed to backup {}: {}", file_name.to_string_lossy(), e))?;
        }
    }

    Ok(backup_path)
}
```

#### `restore_from_backup()`
```rust
fn restore_from_backup(app_data_dir: &Path) -> Result<(), String> {
    let profiles_dir = app_data_dir.join("profiles");
    let backup_dir = app_data_dir.join("profiles_backup");

    // Find most recent backup
    let latest_backup = find_latest_backup(&backup_dir)?;

    // Delete current profiles
    for entry in fs::read_dir(&profiles_dir)
        .map_err(|e| format!("Failed to read profiles directory: {}", e))?
    {
        let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
        let path = entry.path();

        if path.extension().and_then(|s| s.to_str()) == Some("json") {
            fs::remove_file(&path)
                .map_err(|e| format!("Failed to delete {}: {}", path.display(), e))?;
        }
    }

    // Restore from backup
    for entry in fs::read_dir(&latest_backup)
        .map_err(|e| format!("Failed to read backup directory: {}", e))?
    {
        let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
        let path = entry.path();
        let file_name = path.file_name().unwrap();
        let dest = profiles_dir.join(file_name);

        fs::copy(&path, &dest)
            .map_err(|e| format!("Failed to restore {}: {}", file_name.to_string_lossy(), e))?;
    }

    Ok(())
}
```

### Data Structures

```rust
/// Report returned after successful key rotation
#[derive(Debug, Clone, serde::Serialize)]
pub struct RotationReport {
    pub profiles_updated: usize,
    pub keys_reencrypted: usize,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Stats for a single profile update
#[derive(Debug, Clone)]
struct ProfileUpdateStats {
    pub profile_name: String,
    pub keys_updated: usize,
}

/// Metadata stored in .rotation_metadata.json
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct RotationMetadata {
    pub last_rotated: chrono::DateTime<chrono::Utc>,
    pub rotation_count: usize,
}
```

---

## 3. Error Handling

### Pre-Flight Check Failures

| Check | Error Message | User Action |
|-------|---------------|-------------|
| Active streams detected | "Cannot rotate key while streams are active. Stop all streams and try again." | Stop streams, retry |
| Profile directory not writable | "Cannot write to profile directory. Check permissions." | Fix permissions |
| Profile corrupted | "Profile 'X' is corrupted and cannot be loaded. Fix or delete it first." | Repair/delete profile |
| Insufficient disk space | "Not enough disk space for backup. Need at least X MB free." | Free up space |

### Mid-Rotation Failures

**Strategy**: Immediate rollback on any error

| Failure Point | Action |
|---------------|--------|
| Profile decryption fails | Restore all profiles from backup, keep old key |
| Profile save fails | Restore all profiles from backup, keep old key |
| New key write fails | Restore all profiles from backup, keep old key |

**Critical**: If restoration fails, log error and preserve backup directory

### Error Propagation

```rust
// All functions return Result<T, String>
// Errors bubble up with context

Err(format!(
    "Failed to re-encrypt stream key in profile '{}': {}",
    profile_name,
    e
))
```

---

## 4. Rollback Strategy

### Backup Approach

**Before any changes**:
1. Copy entire `profiles/` directory to `profiles_backup/backup_YYYYMMDD_HHMMSS/`
2. Only proceed if backup succeeds
3. Keep last 5 backups (delete older ones)

### Rollback Triggers

Rollback occurs if **any** of the following fail:
- Decrypting an encrypted stream key with old key
- Re-encrypting a stream key with new key
- Saving an updated profile to disk
- Writing the new machine key file

### Rollback Process

```rust
// Pseudo-code
if error_during_rotation {
    log::error!("Key rotation failed: {}", error);

    // 1. Delete any partially-written files
    cleanup_temp_files()?;

    // 2. Restore all profiles from backup
    restore_from_backup(app_data_dir)?;

    // 3. Delete new key file if it was written
    if new_key_file_exists {
        securely_delete_key_file()?;
    }

    // 4. Verify old key still works
    verify_old_key_works()?;

    // 5. Return error to user
    return Err("Key rotation failed. All changes have been rolled back.");
}
```

### Backup Cleanup

Keep last 5 backups:
```rust
fn cleanup_old_backups(app_data_dir: &Path) -> Result<(), String> {
    let backup_dir = app_data_dir.join("profiles_backup");
    let mut backups = get_backups_sorted_by_date(&backup_dir)?;

    // Keep newest 5, delete rest
    while backups.len() > 5 {
        let oldest = backups.remove(0);
        fs::remove_dir_all(&oldest)?;
    }

    Ok(())
}
```

---

## 5. User Communication

### Progress Updates

**Backend → Frontend events**:

```rust
// Define event payloads
#[derive(Clone, serde::Serialize)]
struct KeyRotationProgress {
    pub step: String,           // "Generating key", "Updating profiles", etc.
    pub current: usize,         // Current item
    pub total: usize,           // Total items
    pub profile_name: Option<String>,
}

// Emit events during rotation
app_handle.emit_all("key_rotation_progress", KeyRotationProgress {
    step: "Re-encrypting stream keys".to_string(),
    current: 3,
    total: 12,
    profile_name: Some("Gaming Stream".to_string()),
})?;
```

**Frontend listener**:
```typescript
import { listen } from '@tauri-apps/api/event';

listen<KeyRotationProgress>('key_rotation_progress', (event) => {
  const { step, current, total, profile_name } = event.payload;
  updateProgressModal({
    step,
    progress: (current / total) * 100,
    profileName: profile_name,
  });
});
```

### Success Message

After completion:
```rust
app_handle.emit_all("key_rotation_complete", RotationReport {
    profiles_updated: 5,
    keys_reencrypted: 12,
    timestamp: chrono::Utc::now(),
})?;
```

### Error Notifications

```rust
app_handle.emit_all("key_rotation_failed", KeyRotationError {
    message: "Failed to decrypt stream key in profile 'Gaming Stream'",
    profile_name: Some("Gaming Stream".to_string()),
    rollback_successful: true,
})?;
```

---

## 6. Security Considerations

### Zeroization

**All sensitive data zeroized**:
```rust
// Old key
let old_key = get_or_create_machine_key(app_data_dir)?;
// ... use old_key ...
drop(old_key); // Zeroizing<T> automatically zeroizes on drop

// Decrypted stream keys
let mut plaintext = decrypt_with_key(&encrypted, &old_key)?;
let reencrypted = encrypt_with_key(&plaintext, &new_key)?;
plaintext.zeroize(); // Explicit zeroization

// New key during generation
let new_key = Zeroizing::new(rng.gen::<[u8; KEY_LEN]>());
```

### Atomic Operations

**Problem**: Partial updates could leave system in inconsistent state

**Solution**: Backup + Rollback strategy
- All profiles backed up before any changes
- Any failure triggers immediate restoration
- New key only written after all profiles succeed

### Timing Attacks

**Not a concern**: Key rotation is user-initiated, not a hot path

### File System Security

**Backup protection**:
```rust
// Apply same permissions to backup directory
#[cfg(unix)]
{
    use std::os::unix::fs::PermissionsExt;
    let perms = std::fs::Permissions::from_mode(0o700); // Owner only
    std::fs::set_permissions(&backup_dir, perms)?;
}
```

### Secure Deletion

**Old key file**:
```rust
fn securely_delete_key_file(key_file: &Path) -> Result<(), String> {
    // 1. Read file
    let mut data = fs::read(key_file)?;

    // 2. Overwrite with zeros
    data.zeroize();
    fs::write(key_file, &data)?;

    // 3. Overwrite with random
    let random: Vec<u8> = (0..data.len()).map(|_| rand::random()).collect();
    fs::write(key_file, &random)?;

    // 4. Delete
    fs::remove_file(key_file)?;

    Ok(())
}
```

**Note**: On SSDs, this may not be effective due to wear leveling. Document limitation.

---

## 7. API Design

### Tauri Command

```rust
#[tauri::command]
pub async fn rotate_machine_key(
    app_handle: tauri::AppHandle,
) -> Result<RotationReport, String> {
    let app_data_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data directory: {}", e))?;

    rotate_machine_key_impl(&app_data_dir, &app_handle)
}
```

### Frontend API

```typescript
// services/settings.ts
export async function rotateMachineKey(): Promise<RotationReport> {
  return invoke<RotationReport>('rotate_machine_key');
}

// Component usage
async function handleRotateKey() {
  try {
    setIsRotating(true);
    const report = await rotateMachineKey();
    showSuccess(`Rotated key successfully! ${report.profiles_updated} profiles updated.`);
  } catch (error) {
    showError(`Key rotation failed: ${error}`);
  } finally {
    setIsRotating(false);
  }
}
```

### Metadata API

```rust
#[tauri::command]
pub async fn get_key_rotation_metadata(
    app_handle: tauri::AppHandle,
) -> Result<Option<RotationMetadata>, String> {
    let app_data_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data directory: {}", e))?;

    read_rotation_metadata(&app_data_dir)
}
```

---

## 8. Testing Strategy

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_reencrypt_profile() {
        let temp_dir = tempdir().unwrap();
        let old_key = Zeroizing::new([1u8; KEY_LEN]);
        let new_key = Zeroizing::new([2u8; KEY_LEN]);

        // Create test profile with encrypted keys
        let profile = create_test_profile_with_encrypted_keys(&old_key);
        save_profile_raw(&profile, temp_dir.path()).unwrap();

        // Re-encrypt
        let stats = reencrypt_profile(
            &profile.name,
            &old_key,
            &new_key,
            temp_dir.path()
        ).unwrap();

        assert_eq!(stats.keys_updated, 3);

        // Verify decryption with new key works
        let loaded = load_profile_raw(&profile.name, temp_dir.path()).unwrap();
        let plaintext = decrypt_with_key(
            &loaded.output_groups[0].stream_targets[0].stream_key,
            &new_key
        ).unwrap();
        assert_eq!(plaintext, "test_stream_key");
    }

    #[test]
    fn test_backup_and_restore() {
        let temp_dir = tempdir().unwrap();

        // Create test profiles
        create_test_profiles(temp_dir.path(), 3);

        // Backup
        let backup_path = backup_profiles_directory(temp_dir.path()).unwrap();
        assert!(backup_path.exists());

        // Delete originals
        delete_all_profiles(temp_dir.path()).unwrap();

        // Restore
        restore_from_backup(temp_dir.path()).unwrap();

        // Verify all profiles restored
        let profiles = get_all_profile_names(temp_dir.path()).unwrap();
        assert_eq!(profiles.len(), 3);
    }

    #[test]
    fn test_rollback_on_error() {
        let temp_dir = tempdir().unwrap();
        let old_key = Zeroizing::new([1u8; KEY_LEN]);
        let new_key = Zeroizing::new([2u8; KEY_LEN]);

        // Create profiles, one with invalid encryption
        create_test_profiles_with_corruption(temp_dir.path(), &old_key);

        // Attempt rotation
        let result = rotate_machine_key_impl(temp_dir.path(), &mock_app_handle());

        // Should fail
        assert!(result.is_err());

        // Verify rollback occurred
        let profiles = get_all_profile_names(temp_dir.path()).unwrap();
        assert_eq!(profiles.len(), 3); // All profiles still there

        // Verify old key still works
        let profile = load_profile_raw(&profiles[0], temp_dir.path()).unwrap();
        let plaintext = decrypt_with_key(
            &profile.output_groups[0].stream_targets[0].stream_key,
            &old_key
        ).unwrap();
        assert_eq!(plaintext, "test_stream_key");
    }
}
```

### Integration Tests

```rust
#[test]
fn test_full_key_rotation_flow() {
    let temp_dir = tempdir().unwrap();

    // 1. Create profiles with encrypted keys
    setup_test_environment(temp_dir.path(), 5);

    // 2. Generate initial machine key
    let old_key = get_or_create_machine_key(temp_dir.path()).unwrap();

    // 3. Perform rotation
    let report = rotate_machine_key_impl(temp_dir.path(), &mock_app_handle()).unwrap();

    // 4. Verify report
    assert_eq!(report.profiles_updated, 5);
    assert_eq!(report.keys_reencrypted, 15); // 3 keys per profile

    // 5. Load new key
    let new_key = get_or_create_machine_key(temp_dir.path()).unwrap();

    // 6. Verify keys are different
    assert_ne!(&*old_key, &*new_key);

    // 7. Verify all profiles can be decrypted with new key
    for profile_name in get_all_profile_names(temp_dir.path()).unwrap() {
        let profile = load_profile_raw(&profile_name, temp_dir.path()).unwrap();

        for group in &profile.output_groups {
            for target in &group.stream_targets {
                if is_stream_key_encrypted(&target.stream_key) {
                    let plaintext = decrypt_with_key(&target.stream_key, &new_key).unwrap();
                    assert!(!plaintext.is_empty());
                }
            }
        }
    }

    // 8. Verify metadata updated
    let metadata = read_rotation_metadata(temp_dir.path()).unwrap().unwrap();
    assert_eq!(metadata.rotation_count, 1);
}
```

### Manual Testing Checklist

- [ ] Rotate key with 0 profiles (should succeed trivially)
- [ ] Rotate key with 1 profile, 1 encrypted key
- [ ] Rotate key with multiple profiles, multiple keys
- [ ] Rotate key with active stream (should be blocked)
- [ ] Rotate key with corrupted profile (should rollback)
- [ ] Rotate key with insufficient disk space (should fail gracefully)
- [ ] Verify backup directory created with correct permissions
- [ ] Verify old backups cleaned up (keep only 5)
- [ ] Verify metadata updated after successful rotation
- [ ] Kill app mid-rotation, verify backup preserved
- [ ] Check "Last Rotated" timestamp in UI updates
- [ ] Verify stream keys still work after rotation
- [ ] Verify zeroization using memory debugger

---

## 9. Implementation Phases

### Phase 1: Core Functionality (MVP)
**Files to modify**:
- `src-tauri/src/services/encryption.rs` - Add rotation functions
- `src-tauri/src/commands/settings.rs` - Add Tauri command
- `src-tauri/src/models/settings.rs` - Add RotationMetadata

**Estimated time**: 4-6 hours

**Deliverables**:
- `rotate_machine_key()` function
- `reencrypt_profile()` function
- `backup_profiles_directory()` function
- `restore_from_backup()` function
- Basic error handling

### Phase 2: UI Integration
**Files to create/modify**:
- `src-frontend/lib/tauri.ts` - Add API wrapper
- `src-frontend/components/settings/KeyRotationSection.tsx` - New component
- `src-frontend/components/modals/KeyRotationModal.tsx` - Confirmation modal
- `src-frontend/views/Settings.tsx` - Add rotation section

**Estimated time**: 3-4 hours

**Deliverables**:
- Settings UI with "Rotate Key" button
- Confirmation modal
- Progress modal
- Success/error notifications

### Phase 3: Progress & Polish
**Files to modify**:
- `src-tauri/src/services/encryption.rs` - Add event emissions
- `src-frontend/components/modals/KeyRotationProgressModal.tsx` - New component

**Estimated time**: 2-3 hours

**Deliverables**:
- Real-time progress updates
- Profile-by-profile progress indicator
- Cancellation handling (optional)

### Phase 4: Testing & Documentation
**Files to create/modify**:
- `src-tauri/src/services/encryption.rs` - Add unit tests
- `.claude/claudedocs/encryption-key-rotation.md` - User-facing docs

**Estimated time**: 2-3 hours

**Deliverables**:
- Comprehensive unit tests
- Integration tests
- User documentation
- Testing checklist completed

---

## 10. Open Questions

### Cancellation Support?
**Question**: Should users be able to cancel rotation mid-process?

**Options**:
1. **No cancellation** (simpler, safer)
   - Once started, must complete or fail
   - Prevents partial state

2. **Cancellation allowed** (more flexible, risky)
   - Must still rollback on cancel
   - Adds complexity

**Recommendation**: Start with no cancellation (safer)

### Automatic Rotation?
**Question**: Should the app suggest rotation after X days?

**Options**:
1. **Manual only** (current plan)
2. **Reminder notification** (e.g., "Last rotated 90 days ago")
3. **Automatic rotation** (risky, not recommended)

**Recommendation**: Manual only for v1, add reminder in future

### Key Rotation History?
**Question**: Should we keep a log of all rotations?

**Options**:
1. **Metadata only** (just last rotation timestamp)
2. **Full history log** (all rotations with timestamps)

**Recommendation**: Metadata only for v1

### Password-Protected Profiles?
**Question**: How does rotation interact with password-protected profiles?

**Answer**: Password-protected profiles store encrypted JSON with a different key (derived from user password). Stream keys inside are **still** encrypted with the machine key, so rotation still works:

1. Load password-protected profile (requires password)
2. Access decrypted JSON in memory
3. Re-encrypt stream keys with new machine key
4. Re-encrypt entire profile with password-derived key
5. Save updated profile

**No special handling needed** - rotation works transparently.

---

## 11. Success Criteria

- [ ] All stream keys re-encrypted successfully
- [ ] All profiles remain loadable after rotation
- [ ] Backup created before any modifications
- [ ] Rollback works correctly on any failure
- [ ] Old key zeroized from memory
- [ ] New key has same protection as old key (permissions/attributes)
- [ ] Metadata updated with rotation timestamp
- [ ] UI shows rotation progress and result
- [ ] No data loss under any failure scenario
- [ ] Test coverage >80% for rotation code

---

## 12. Security Audit Checklist

Before marking this feature complete:

- [ ] All Zeroizing wrappers used correctly
- [ ] No key material logged anywhere
- [ ] Backup directory has correct permissions
- [ ] Old key file securely deleted
- [ ] Timing attack surface minimized
- [ ] Error messages don't leak key data
- [ ] Rollback tested with deliberate failures
- [ ] Memory dump won't reveal keys
- [ ] No race conditions in file operations
- [ ] Tested on all platforms (Windows, macOS, Linux)

---

## 13. Future Enhancements

### V2 Features
1. **Scheduled Rotation Reminders** - Notify user after 90 days
2. **Cancellation Support** - Allow user to cancel mid-rotation
3. **Rotation History** - Keep log of all rotations
4. **Bulk Operations** - Rotate + backup in one operation
5. **Key Export/Import** - For advanced users (encrypted with password)

### V3 Features
1. **Key Rotation API** - Allow external automation
2. **Multi-Key Support** - Use different keys for different profiles
3. **Hardware Security Module (HSM) Integration** - For enterprise users

---

**End of Plan**

**Next Steps**: Review plan, get approval, begin Phase 1 implementation.
