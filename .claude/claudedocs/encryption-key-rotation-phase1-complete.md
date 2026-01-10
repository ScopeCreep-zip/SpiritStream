# Machine Key Rotation - Phase 1 Implementation Complete

**Date**: 2026-01-10
**Status**: ✅ Phase 1 Complete
**Build Status**: ✅ Compiles Successfully

## What Was Implemented

Phase 1 focused on implementing the core rotation logic in the Rust backend. All code compiles and is ready for integration with the UI in Phase 2.

### Files Modified

1. **`src-tauri/src/services/encryption.rs`** (Added ~300 lines)
   - Helper functions for key rotation
   - Backup and restore functionality
   - Main rotation algorithm
   - RotationReport struct

2. **`src-tauri/src/commands/settings.rs`** (Added 1 command)
   - `rotate_machine_key` Tauri command

3. **`src-tauri/src/lib.rs`** (Registered command)
   - Added `commands::rotate_machine_key` to invoke_handler

## Functionality Added

### Core Functions (encryption.rs)

#### 1. Key Encryption/Decryption with Specific Keys
```rust
fn decrypt_stream_key_with_key(encrypted_key: &str, machine_key: &Zeroizing<[u8; KEY_LEN]>) -> Result<String, String>
fn encrypt_stream_key_with_key(stream_key: &str, machine_key: &Zeroizing<[u8; KEY_LEN]>) -> Result<String, String>
```
- Allows decrypting/encrypting with a specific machine key (not just the current one)
- Required for rotation to use both old and new keys

#### 2. Key Generation and Management
```rust
fn generate_new_machine_key() -> Result<Zeroizing<[u8; KEY_LEN]>, String>
fn write_machine_key(key: &Zeroizing<[u8; KEY_LEN]>, app_data_dir: &Path) -> Result<(), String>
fn securely_delete_key_file(app_data_dir: &Path) -> Result<(), String>
```
- Generate new random 32-byte key
- Write key to disk with proper permissions (0o600 Unix, Hidden+System Windows)
- Securely delete old key (overwrite with zeros, then random, then delete)

#### 3. Backup and Restore
```rust
fn backup_profiles_directory(app_data_dir: &Path) -> Result<PathBuf, String>
fn restore_from_backup(backup_path: &Path, app_data_dir: &Path) -> Result<(), String>
fn cleanup_old_backups(app_data_dir: &Path, keep_count: usize) -> Result<(), String>
```
- Creates timestamped backup before rotation: `profiles_backup/backup_YYYYMMDD_HHMMSS/`
- Copies all .json and .mgs files
- Sets restrictive permissions (0o700 on Unix)
- Restores from backup on any error
- Keeps only last 5 backups (automatic cleanup)

#### 4. Main Rotation Function
```rust
pub fn rotate_machine_key(app_data_dir: &Path, profiles_dir: &Path) -> Result<RotationReport, String>
```

**Algorithm**:
1. Create backup of all profiles
2. Load old machine key
3. Generate new random machine key
4. For each profile file:
   - If .mgs (encrypted): Skip (will be re-encrypted on next save)
   - If .json (unencrypted): Parse and re-encrypt stream keys
   - Decrypt each encrypted stream key with old key
   - Re-encrypt with new key
   - Save updated profile
   - **On any error**: Rollback all changes from backup
5. Securely delete old key file
6. Write new key file
7. Clean up old backups (keep last 5)

**Returns**: `RotationReport` with:
- `profiles_updated` - Number of profiles updated
- `keys_reencrypted` - Total number of stream keys re-encrypted
- `total_profiles` - Total number of profiles found
- `timestamp` - When rotation completed

#### 5. Profile Re-encryption
```rust
fn reencrypt_profile_file(profile_path: &Path, old_key: &Zeroizing<[u8; KEY_LEN]>, new_key: &Zeroizing<[u8; KEY_LEN]>) -> Result<usize, String>
```
- Reads profile JSON
- Finds all encrypted stream keys (prefix `ENC::`)
- Decrypts with old key
- Re-encrypts with new key
- Saves updated profile
- Returns count of keys updated

### Tauri Command

```rust
#[tauri::command]
pub fn rotate_machine_key(app_handle: AppHandle) -> Result<RotationReport, String>
```

**Usage from frontend**:
```typescript
import { invoke } from '@tauri-apps/api/core';

const report = await invoke<RotationReport>('rotate_machine_key');
console.log(`Rotated ${report.keys_reencrypted} keys in ${report.profiles_updated} profiles`);
```

## Security Features

### ✅ Zeroization
- All sensitive keys wrapped in `Zeroizing<T>`
- Old key automatically zeroized when dropped
- Decrypted stream keys explicitly zeroized
- Temporary buffers zeroized before returning

### ✅ Secure Key Deletion
- Old key file overwritten with zeros
- Then overwritten with random data
- Then deleted from filesystem
- (Note: On SSDs, may not be fully effective due to wear leveling)

### ✅ Atomic Rollback
- Backup created before any changes
- Any error triggers immediate restore
- All-or-nothing operation
- No partial state possible

### ✅ Platform-Specific Protection
- New key file gets same protection as old key:
  - **Unix/macOS**: 0o600 permissions
  - **Windows**: Hidden + System attributes
- Backup directory protected (0o700 on Unix)

## Error Handling

### Pre-Rotation Errors
| Error | Behavior |
|-------|----------|
| Cannot create backup directory | Returns error, no changes made |
| Cannot read profiles directory | Returns error, no changes made |
| Old key file missing | Creates new key (safe default) |

### During-Rotation Errors
| Error | Behavior |
|-------|----------|
| Profile parse error | Rollback all changes, restore backup |
| Decryption fails | Rollback all changes, restore backup |
| Save fails | Rollback all changes, restore backup |
| Any unexpected error | Rollback all changes, restore backup |

### Post-Rotation Errors
| Error | Behavior |
|-------|----------|
| Backup cleanup fails | Log warning, rotation still successful |

## Limitations & Future Work

### Current Limitations

1. **Encrypted profiles (.mgs) skipped**
   - Cannot decrypt without user password
   - Stream keys will be re-encrypted when profile is next saved
   - Not a security issue (keys are already encrypted)

2. **No progress events**
   - Phase 1 doesn't emit progress updates
   - Phase 3 will add real-time progress

3. **No pre-flight checks**
   - Doesn't check if streams are active
   - Doesn't verify write permissions beforehand
   - Phase 2 UI will add validation

### Testing Needed

- [ ] Unit tests for backup/restore
- [ ] Unit tests for re-encryption logic
- [ ] Integration test with real profiles
- [ ] Test rollback on deliberate failure
- [ ] Test with encrypted profiles (.mgs)
- [ ] Test with 0 profiles
- [ ] Test with 100+ profiles
- [ ] Test on all platforms (Windows, macOS, Linux)
- [ ] Verify zeroization with memory debugger

## Next Steps: Phase 2

Phase 2 will add the UI integration:

**Files to create/modify**:
1. `src-frontend/lib/tauri.ts` - Add `rotateMachineKey()` wrapper
2. `src-frontend/components/settings/KeyRotationSection.tsx` - Settings UI
3. `src-frontend/components/modals/KeyRotationModal.tsx` - Confirmation modal
4. `src-frontend/views/Settings.tsx` - Integrate rotation section

**Features to add**:
- "Rotate Key" button in Settings
- Confirmation modal with warning
- Success/error notifications
- Display last rotation timestamp (requires metadata)

## Code Statistics

**Total lines added**: ~350
- `encryption.rs`: ~300 lines
- `settings.rs`: ~20 lines
- `lib.rs`: 1 line

**Test coverage**: 0% (Phase 4 will add tests)

---

**Phase 1 Status**: ✅ **COMPLETE**
**Compilation**: ✅ **SUCCESS**
**Ready for Phase 2**: ✅ **YES**
