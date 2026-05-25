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
use super::{internal, KEY_LEN, STREAM_KEY_PREFIX_V1, STREAM_KEY_PREFIX_V2};

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
        let backup_path = backup_profiles_directory(app_data_dir)?;

        // 4. Load old key.
        let old_key = get_or_create_machine_key(app_data_dir)?;

        // 5. Generate new key.
        let new_key = generate_new_machine_key()?;

        let total_profiles = profile_files.len();
        let mut profiles_updated = 0;
        let mut keys_reencrypted = 0;

        // 6. Re-encrypt each profile. Per-profile failure rolls everything
        //    back (the on-disk old `.stream_key` is still present and the
        //    backup directory contains the pre-rotation profile files).
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
                // restore_from_backup runs on every Err path below.
                let pw = encrypted_passwords.get(name).ok_or_else(|| CoreError::Internal {
                    context: format!(
                        "rotate: password for encrypted profile '{name}' disappeared between pre-flight and re-encrypt"
                    ),
                })?;
                reencrypt_mgs_profile(profile_path, pw, &old_key, &new_key)
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
                    restore_from_backup(&backup_path, app_data_dir)?;
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

        // 7. Securely delete old key.
        securely_delete_key_file(app_data_dir)?;

        // 8. Write new key.
        write_machine_key(&new_key, app_data_dir)?;

        // 9. Clean up old backups (keep last 5).
        cleanup_old_backups(app_data_dir, 5)?;

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

fn write_machine_key(
    key: &Zeroizing<[u8; KEY_LEN]>,
    app_data_dir: &Path,
) -> Result<(), CoreError> {
    let key_file = app_data_dir.join(".stream_key");
    crate::services::write_owner_only_atomic(&key_file, &**key)
}

fn securely_delete_key_file(app_data_dir: &Path) -> Result<(), CoreError> {
    let key_file = app_data_dir.join(".stream_key");

    if !key_file.exists() {
        return Ok(());
    }

    let metadata = std::fs::metadata(&key_file)
        .map_err(|e| internal("Failed to read key file metadata", e))?;
    let size = metadata.len() as usize;

    // Overwrite with zeros, then random, then unlink. Best-effort
    // sanitization on top of unlink; filesystems with copy-on-write
    // semantics may retain prior blocks regardless.
    let zeros = vec![0u8; size];
    std::fs::write(&key_file, &zeros).map_err(|e| internal("Failed to overwrite key file", e))?;

    let mut rng = rand::thread_rng();
    let random: Vec<u8> = (0..size).map(|_| rng.gen()).collect();
    std::fs::write(&key_file, &random)
        .map_err(|e| internal("Failed to overwrite key file", e))?;

    std::fs::remove_file(&key_file).map_err(|e| internal("Failed to delete key file", e))?;

    Ok(())
}

fn backup_profiles_directory(app_data_dir: &Path) -> Result<PathBuf, CoreError> {
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

fn restore_from_backup(backup_path: &Path, app_data_dir: &Path) -> Result<(), CoreError> {
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

fn cleanup_old_backups(app_data_dir: &Path, keep_count: usize) -> Result<(), CoreError> {
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
            context: format!("invalid encrypted profile magic: {}", profile_path.display()),
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
    let mut profile: Profile = serde_json::from_str(&json_str)
        .map_err(|e| internal("Failed to parse profile JSON", e))?;

    let keys_updated = rotate_inner_secrets(&mut profile, old_key, new_key)?;

    let new_json = serde_json::to_string_pretty(&profile)
        .map_err(|e| internal("Failed to serialize profile", e))?;
    crate::services::write_owner_only_atomic(profile_path, new_json.as_bytes())?;

    Ok(keys_updated)
}

/// Swap every machine-key-encrypted field on a deserialized `Profile`
/// over to the new machine key. Stream keys live on each target; the
/// other sensitive-settings fields (OBS password, Discord webhook,
/// backend token, YouTube API key, OAuth access/refresh tokens) live
/// on `profile.settings`.
fn rotate_inner_secrets(
    profile: &mut crate::models::Profile,
    old_key: &Zeroizing<[u8; KEY_LEN]>,
    new_key: &Zeroizing<[u8; KEY_LEN]>,
) -> Result<usize, CoreError> {
    let mut keys_updated = 0;

    for group in &mut profile.output_groups {
        for target in &mut group.stream_targets {
            if super::Encryption::is_stream_key_encrypted(&target.stream_key) {
                let plaintext = decrypt_stream_key_with_key(&target.stream_key, old_key)?;
                target.stream_key = encrypt_stream_key_with_key(&plaintext, new_key)?;
                keys_updated += 1;
            }
        }
    }

    // Sensitive profile settings — same encryption scheme as stream keys.
    let settings_fields: [&mut String; 4] = [
        &mut profile.settings.obs.password,
        &mut profile.settings.discord.webhook_url,
        &mut profile.settings.backend.token,
        &mut profile.settings.chat.youtube_api_key,
    ];
    for field in settings_fields {
        if !field.is_empty() && super::Encryption::is_stream_key_encrypted(field) {
            let plaintext = decrypt_stream_key_with_key(field, old_key)?;
            *field = encrypt_stream_key_with_key(&plaintext, new_key)?;
            keys_updated += 1;
        }
    }

    // OAuth tokens — twitch + youtube.
    for account in [
        &mut profile.settings.oauth.twitch,
        &mut profile.settings.oauth.youtube,
    ] {
        if !account.access_token.is_empty()
            && super::Encryption::is_stream_key_encrypted(&account.access_token)
        {
            let plaintext = decrypt_stream_key_with_key(&account.access_token, old_key)?;
            account.access_token = encrypt_stream_key_with_key(&plaintext, new_key)?;
            keys_updated += 1;
        }
        if !account.refresh_token.is_empty()
            && super::Encryption::is_stream_key_encrypted(&account.refresh_token)
        {
            let plaintext = decrypt_stream_key_with_key(&account.refresh_token, old_key)?;
            account.refresh_token = encrypt_stream_key_with_key(&plaintext, new_key)?;
            keys_updated += 1;
        }
    }

    Ok(keys_updated)
}
