use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use zeroize::Zeroizing;

use crate::errors::{CoreError, ValidationIssue};

use super::machine_key::{
    decode_and_decrypt_v1, decode_and_decrypt_v2, encrypt_with_machine_key_v2,
    get_or_create_machine_key,
};
use super::{internal, rotation_backup, KEY_LEN, STREAM_KEY_PREFIX_V1, STREAM_KEY_PREFIX_V2};

/// Report returned after successful key rotation
#[derive(Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../../packages/types/src/generated/")]
pub struct RotationReport {
    pub profiles_updated: usize,
    pub keys_reencrypted: usize,
    pub total_profiles: usize,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl super::Encryption {
    /// Rotate the machine encryption key.
    ///
    /// Re-encrypts every machine-key-encrypted secret in every profile file
    /// (stream keys, OBS password, Discord webhook, etc.) under a freshly
    /// generated machine key. `.json` profiles are re-encrypted in place;
    /// `.mgs` (password-protected) profiles are decrypted with the supplied
    /// password from `encrypted_passwords`, their inner secrets re-encrypted
    /// under the new machine key, and the envelope re-sealed with the same
    /// user password.
    ///
    /// Pre-flight validation:
    /// - Every `.mgs` profile on disk must have a corresponding entry in
    ///   `encrypted_passwords`. Missing entries return `ValidationFailed`
    ///   with `code = "missing_password_for_<name>"` BEFORE the old key is
    ///   touched, so a failed rotation leaves disk state unchanged.
    /// - Every supplied password is verified by an actual decrypt of the
    ///   `.mgs` blob. A wrong password returns `PasswordIncorrect` before
    ///   the old key is destroyed.
    pub fn rotate_machine_key(
        app_data_dir: &Path,
        profiles_dir: &Path,
        encrypted_passwords: &std::collections::HashMap<String, String>,
    ) -> Result<RotationReport, CoreError> {
        log::info!("Starting machine key rotation");

        // 1. Enumerate profile files up-front so pre-flight checks can run
        //    before any destructive step.
        let entries = std::fs::read_dir(profiles_dir)
            .map_err(|e| internal("Failed to read profiles directory", e))?;

        let profile_files: Vec<PathBuf> = entries
            .flatten()
            .filter(|e| {
                let path = e.path();
                let ext = path.extension().and_then(|ext| ext.to_str());
                ext == Some("json") || ext == Some("mgs")
            })
            .map(|e| e.path())
            .collect();

        // 2. Pre-flight: every `.mgs` profile must have a password supplied
        //    AND that password must successfully decrypt the envelope. We do
        //    this BEFORE backing up or touching the old key — a missing or
        //    wrong password aborts cleanly with disk state untouched.
        let mut missing: Vec<ValidationIssue> = Vec::new();
        for path in &profile_files {
            if path.extension().and_then(|e| e.to_str()) != Some("mgs") {
                continue;
            }
            let name = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("<unknown>")
                .to_string();
            match encrypted_passwords.get(&name) {
                None => missing.push(ValidationIssue {
                    code: format!("missing_password_for_{name}"),
                    message: format!("password required for encrypted profile '{name}'"),
                    path: Some(format!("/unlockedPasswords/{name}")),
                }),
                Some(pw) => {
                    // Verify the password actually decrypts the envelope.
                    // Surfaces `PasswordIncorrect` directly so the UI can
                    // localize "wrong password for <name>" without first
                    // destroying the old key.
                    let _verify = decrypt_mgs_envelope(path, pw)?;
                }
            }
        }
        if !missing.is_empty() {
            return Err(CoreError::ValidationFailed { reasons: missing });
        }

        // 3. Create backup AFTER pre-flight passes — no point making a backup
        //    of a state we're going to refuse to mutate.
        let backup_path = rotation_backup::backup_profiles_directory(app_data_dir)?;

        // 4. Load old key.
        let old_key = get_or_create_machine_key(app_data_dir)?;

        // 5. Generate the new key and persist it to `.stream_key.new`
        //    BEFORE rewriting any profile. Until this rotation completes,
        //    the new key must exist somewhere durable: a crash after even
        //    one profile is rewritten would otherwise leave secrets
        //    encrypted under a key that exists only in this process's RAM.
        //    `.stream_key.new` doubles as the crash-recovery journal —
        //    see `recover_interrupted_rotation`.
        let new_key = generate_new_machine_key()?;
        crate::services::write_owner_only_atomic(&pending_key_path(app_data_dir), &*new_key)?;

        let total_profiles = profile_files.len();
        let mut profiles_updated = 0;
        let mut keys_reencrypted = 0;

        // 6. Re-encrypt each profile. Per-profile failure rolls everything
        //    back: profiles restored from backup, pending new key removed,
        //    old `.stream_key` untouched.
        for profile_path in &profile_files {
            let result = if profile_path.extension().and_then(|e| e.to_str()) == Some("mgs") {
                let name = profile_path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("<unknown>");
                // Pre-flight (step 2 above) verified every .mgs profile has a
                // matching password, but the password map is passed in by the
                // caller and could in principle be mutated between pre-flight
                // and re-encrypt by a future refactor. Surface that as a
                // structured error rather than panicking mid-rotation — the
                // post-rollback state stays consistent because step 6's
                // rollback runs on every Err path below.
                match encrypted_passwords.get(name) {
                    Some(pw) => reencrypt_mgs_profile(profile_path, pw, &old_key, &new_key),
                    None => Err(CoreError::Internal {
                        context: format!(
                            "rotate: password for encrypted profile '{name}' disappeared between pre-flight and re-encrypt"
                        ),
                    }),
                }
            } else {
                reencrypt_json_profile(profile_path, &old_key, &new_key)
            };
            match result {
                Ok(count) => {
                    profiles_updated += 1;
                    keys_reencrypted += count;
                    log::debug!("Re-encrypted {} keys in {}", count, profile_path.display());
                }
                Err(e) => {
                    log::error!(
                        "Failed to re-encrypt profile {}: {}",
                        profile_path.display(),
                        e
                    );
                    log::error!("Rolling back changes");
                    rotation_backup::restore_from_backup(&backup_path, app_data_dir)?;
                    let _ = std::fs::remove_file(pending_key_path(app_data_dir));
                    return Err(CoreError::Internal {
                        context: format!(
                            "Key rotation failed while updating {}: {}. All changes have been rolled back.",
                            profile_path.display(),
                            e
                        ),
                    });
                }
            }
        }

        // 7. Shred the old key: overwrite then TRUNCATE TO ZERO BYTES —
        //    deliberately not unlinked. The zero-length `.stream_key` is
        //    the journal marker that says "profiles are fully rewritten
        //    under `.stream_key.new`"; recovery promotes the pending key
        //    when it sees this state.
        securely_shred_key_file(app_data_dir)?;

        // 8. Promote the pending key into place atomically.
        promote_pending_key(app_data_dir)?;

        // 9. Clean up old backups (keep last 5).
        rotation_backup::cleanup_old_backups(app_data_dir, 5)?;

        log::info!(
            "Machine key rotation complete: {profiles_updated} profiles updated, {keys_reencrypted} keys re-encrypted"
        );

        Ok(RotationReport {
            profiles_updated,
            keys_reencrypted,
            total_profiles,
            timestamp: chrono::Utc::now(),
        })
    }
}

/// Decrypt a stream key using a specific machine key (for rotation).
/// Recognises both V1 and V2 prefixes — the same machine key is used
/// for either path because the cipher is selected by prefix, not by key.
fn decrypt_stream_key_with_key(
    encrypted_key: &str,
    machine_key: &Zeroizing<[u8; KEY_LEN]>,
) -> Result<String, CoreError> {
    if let Some(encoded) = encrypted_key.strip_prefix(STREAM_KEY_PREFIX_V2) {
        decode_and_decrypt_v2(machine_key, encoded)
    } else if let Some(encoded) = encrypted_key.strip_prefix(STREAM_KEY_PREFIX_V1) {
        decode_and_decrypt_v1(machine_key, encoded)
    } else {
        Ok(encrypted_key.to_string())
    }
}

/// Encrypt a stream key using a specific machine key (for rotation).
/// Always writes V2 — any V1 inputs rotated through this path are
/// automatically upgraded.
fn encrypt_stream_key_with_key(
    stream_key: &str,
    machine_key: &Zeroizing<[u8; KEY_LEN]>,
) -> Result<String, CoreError> {
    if stream_key.is_empty() || super::Encryption::is_stream_key_encrypted(stream_key) {
        return Ok(stream_key.to_string());
    }

    let blob = encrypt_with_machine_key_v2(machine_key, stream_key.as_bytes())?;
    Ok(format!("{}{}", STREAM_KEY_PREFIX_V2, BASE64.encode(&blob)))
}

fn generate_new_machine_key() -> Result<Zeroizing<[u8; KEY_LEN]>, CoreError> {
    let mut rng = rand::thread_rng();
    Ok(Zeroizing::new(rng.gen::<[u8; KEY_LEN]>()))
}

/// Path of the pending (journaled) new machine key during rotation.
pub(super) fn pending_key_path(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join(".stream_key.new")
}

fn machine_key_path(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join(".stream_key")
}

/// Overwrite the old key material (zeros, then random), then truncate
/// to zero bytes. Deliberately NOT unlinked: the zero-length file marks
/// "old key destroyed, profiles live under `.stream_key.new`" for crash
/// recovery. Best-effort sanitization — filesystems with copy-on-write
/// semantics may retain prior blocks regardless.
fn securely_shred_key_file(app_data_dir: &Path) -> Result<(), CoreError> {
    let key_file = machine_key_path(app_data_dir);

    if !key_file.exists() {
        return Ok(());
    }

    let metadata = std::fs::metadata(&key_file)
        .map_err(|e| internal("Failed to read key file metadata", e))?;
    let size = metadata.len() as usize;

    let zeros = vec![0u8; size];
    std::fs::write(&key_file, &zeros).map_err(|e| internal("Failed to overwrite key file", e))?;

    let mut rng = rand::thread_rng();
    let random: Vec<u8> = (0..size).map(|_| rng.gen()).collect();
    std::fs::write(&key_file, &random).map_err(|e| internal("Failed to overwrite key file", e))?;

    std::fs::write(&key_file, b"").map_err(|e| internal("Failed to truncate key file", e))?;

    Ok(())
}

/// Move `.stream_key.new` into place as `.stream_key`.
fn promote_pending_key(app_data_dir: &Path) -> Result<(), CoreError> {
    let pending = pending_key_path(app_data_dir);
    let dest = machine_key_path(app_data_dir);
    // Windows refuses rename-over-existing; the zero-length marker (or a
    // missing dest) is exactly the state recovery handles, so the brief
    // window between remove and rename is covered.
    #[cfg(windows)]
    if dest.exists() {
        std::fs::remove_file(&dest)
            .map_err(|e| internal("Failed to remove shredded key file", e))?;
    }
    std::fs::rename(&pending, &dest)
        .map_err(|e| internal("Failed to promote pending machine key", e))?;
    #[cfg(unix)]
    if let Some(parent) = dest.parent() {
        if let Ok(dir) = std::fs::File::open(parent) {
            let _ = dir.sync_all();
        }
    }
    Ok(())
}

/// Outcome of [`super::Encryption::recover_interrupted_rotation`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RotationRecovery {
    /// No interrupted rotation found.
    Clean,
    /// A rotation died before the old key was shredded. Profiles were
    /// restored from the pre-rotation backup and the pending key was
    /// discarded — the install is back on the old key; the user should
    /// re-run rotation.
    RolledBack,
    /// A rotation died after the old key was shredded but before the
    /// new key was promoted. Every profile was already rewritten under
    /// the pending key, so it was promoted into place — the rotation is
    /// effectively complete.
    Promoted,
}

impl super::Encryption {
    /// Repair an interrupted machine-key rotation.
    ///
    /// Must run at startup (`ServiceRegistry::build`) before anything
    /// derives keys. The journal is `.stream_key.new`:
    ///
    /// - absent → nothing to do.
    /// - present + `.stream_key` still holds a valid 32-byte key → the
    ///   crash happened while profiles were being rewritten (mixed
    ///   state). Restore the most recent backup and discard the pending
    ///   key: deterministic rollback to the old key.
    /// - present + `.stream_key` empty/missing → the old key was already
    ///   shredded, which only happens after every profile was rewritten.
    ///   Promote the pending key: the rotation completes.
    ///
    /// Without this, the old behavior was catastrophic: the next launch
    /// minted a brand-new random key and every secret decrypted to
    /// garbage with no explanation.
    pub fn recover_interrupted_rotation(
        app_data_dir: &Path,
    ) -> Result<RotationRecovery, CoreError> {
        let pending = pending_key_path(app_data_dir);
        if !pending.exists() {
            return Ok(RotationRecovery::Clean);
        }

        let pending_len = std::fs::metadata(&pending)
            .map_err(|e| internal("Failed to read pending key metadata", e))?
            .len();
        let dest = machine_key_path(app_data_dir);
        let dest_valid = std::fs::metadata(&dest)
            .map(|m| m.len() == KEY_LEN as u64)
            .unwrap_or(false);

        if dest_valid {
            // Old key intact → profiles may be in mixed state. Roll back.
            log::warn!(
                "Interrupted key rotation detected (pending key present, old key intact) — \
                 restoring profiles from backup and discarding the pending key"
            );
            match rotation_backup::latest_backup(app_data_dir) {
                Some(backup) => rotation_backup::restore_from_backup(&backup, app_data_dir)?,
                None => {
                    return Err(CoreError::Internal {
                        context: "interrupted key rotation detected but no profiles_backup \
                                  snapshot exists to roll back to; refusing to guess. Restore \
                                  the profiles directory manually, then delete .stream_key.new"
                            .into(),
                    });
                }
            }
            std::fs::remove_file(&pending)
                .map_err(|e| internal("Failed to remove pending key after rollback", e))?;
            return Ok(RotationRecovery::RolledBack);
        }

        // Old key shredded/missing → profiles are fully on the pending key.
        if pending_len != KEY_LEN as u64 {
            return Err(CoreError::Internal {
                context: format!(
                    "interrupted key rotation left a corrupt pending key ({pending_len} bytes) \
                     and no valid old key; restore profiles and .stream_key from a backup"
                ),
            });
        }
        log::warn!(
            "Interrupted key rotation detected (old key already shredded) — \
             promoting the pending key to complete the rotation"
        );
        promote_pending_key(app_data_dir)?;
        Ok(RotationRecovery::Promoted)
    }
}

/// Read a `.mgs` file, strip the magic prefix, and decrypt the envelope
/// with the supplied password. Returns the inner profile JSON bytes.
fn decrypt_mgs_envelope(profile_path: &Path, password: &str) -> Result<Vec<u8>, CoreError> {
    use crate::services::profile_manager::{
        ENCRYPTED_MAGIC_LEN, ENCRYPTED_MAGIC_V1, ENCRYPTED_MAGIC_V2,
    };
    let content =
        std::fs::read(profile_path).map_err(|e| internal("Failed to read profile file", e))?;
    if content.len() < ENCRYPTED_MAGIC_LEN {
        return Err(CoreError::Internal {
            context: format!("invalid encrypted profile: {}", profile_path.display()),
        });
    }
    let magic = &content[..ENCRYPTED_MAGIC_LEN];
    let body = &content[ENCRYPTED_MAGIC_LEN..];
    if magic == ENCRYPTED_MAGIC_V2 {
        super::Encryption::decrypt_v2(body, password)
    } else if magic == ENCRYPTED_MAGIC_V1 {
        super::Encryption::decrypt_v1(body, password)
    } else {
        Err(CoreError::Internal {
            context: format!(
                "invalid encrypted profile magic: {}",
                profile_path.display()
            ),
        })
    }
}

/// Re-encrypt a `.mgs` profile: decrypt the envelope with the user's
/// password, swap every machine-key-encrypted secret over to the new
/// machine key, then re-seal the envelope under the same password and
/// write to disk atomically. Always writes V2 envelopes; an existing
/// V1 profile is upgraded as a side effect.
fn reencrypt_mgs_profile(
    profile_path: &Path,
    password: &str,
    old_key: &Zeroizing<[u8; KEY_LEN]>,
    new_key: &Zeroizing<[u8; KEY_LEN]>,
) -> Result<usize, CoreError> {
    use crate::models::Profile;
    use crate::services::profile_manager::{ENCRYPTED_MAGIC_LEN, ENCRYPTED_MAGIC_V2};

    let plaintext_bytes = decrypt_mgs_envelope(profile_path, password)?;
    let json_str = String::from_utf8(plaintext_bytes)
        .map_err(|e| internal("Invalid UTF-8 in decrypted profile", e))?;
    let mut profile: Profile = serde_json::from_str(&json_str)
        .map_err(|e| internal("Failed to parse decrypted profile JSON", e))?;

    let keys_updated = rotate_inner_secrets(&mut profile, old_key, new_key)?;

    let new_json = serde_json::to_string_pretty(&profile)
        .map_err(|e| internal("Failed to serialize profile", e))?;
    let encrypted = super::Encryption::encrypt(new_json.as_bytes(), password)?;
    let mut data = Vec::with_capacity(ENCRYPTED_MAGIC_LEN + encrypted.len());
    data.extend_from_slice(ENCRYPTED_MAGIC_V2);
    data.extend_from_slice(&encrypted);
    crate::services::write_owner_only_atomic(profile_path, &data)?;

    Ok(keys_updated)
}

/// Re-encrypt a `.json` (plaintext-envelope) profile. The profile JSON
/// itself sits on disk in plaintext, but the stream keys / OBS password
/// / Discord webhook fields are individually machine-key-encrypted.
fn reencrypt_json_profile(
    profile_path: &Path,
    old_key: &Zeroizing<[u8; KEY_LEN]>,
    new_key: &Zeroizing<[u8; KEY_LEN]>,
) -> Result<usize, CoreError> {
    use crate::models::Profile;

    let content =
        std::fs::read(profile_path).map_err(|e| internal("Failed to read profile file", e))?;
    let json_str =
        String::from_utf8(content).map_err(|e| internal("Invalid UTF-8 in profile", e))?;
    let mut profile: Profile =
        serde_json::from_str(&json_str).map_err(|e| internal("Failed to parse profile JSON", e))?;

    let keys_updated = rotate_inner_secrets(&mut profile, old_key, new_key)?;

    let new_json = serde_json::to_string_pretty(&profile)
        .map_err(|e| internal("Failed to serialize profile", e))?;
    crate::services::write_owner_only_atomic(profile_path, new_json.as_bytes())?;

    Ok(keys_updated)
}

/// Swap every machine-key-encrypted field on a deserialized `Profile`
/// over to the new machine key.
///
/// The field inventory is the shared walker in
/// `profile/secret_fields.rs` — the same one the save/load encryption
/// boundary uses. Rotation re-encrypting a strict subset of what save
/// encrypts is the data-loss bug this fixes: any field skipped here is
/// destroyed the moment the old key is shredded (pre-fix that was the
/// PII blocklist and the kick/facebook OAuth tokens).
fn rotate_inner_secrets(
    profile: &mut crate::models::Profile,
    old_key: &Zeroizing<[u8; KEY_LEN]>,
    new_key: &Zeroizing<[u8; KEY_LEN]>,
) -> Result<usize, CoreError> {
    let mut keys_updated = 0;
    crate::services::profile::secret_fields::visit_secret_fields(profile, |_, field| {
        if super::Encryption::is_stream_key_encrypted(field) {
            let plaintext = decrypt_stream_key_with_key(field, old_key)?;
            *field = encrypt_stream_key_with_key(&plaintext, new_key)?;
            keys_updated += 1;
        }
        Ok(())
    })?;
    Ok(keys_updated)
}
