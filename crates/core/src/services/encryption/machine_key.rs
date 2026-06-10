use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use aes_gcm_siv::Aes256GcmSiv;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use rand::Rng;
use std::path::Path;
use zeroize::{Zeroize, Zeroizing};

use crate::errors::CoreError;

use super::{internal, KEY_LEN, NONCE_LEN, STREAM_KEY_PREFIX_V1, STREAM_KEY_PREFIX_V2};

impl super::Encryption {
    /// Public accessor for the machine key. Used by the audit log to derive
    /// its HMAC chain key via HKDF. Returns a `Zeroizing` wrapper so callers
    /// can't accidentally hold the bytes past their scope.
    pub fn get_or_create_machine_key_public(
        app_data_dir: &Path,
    ) -> Result<Zeroizing<[u8; KEY_LEN]>, CoreError> {
        get_or_create_machine_key(app_data_dir)
    }

    /// Derive a 32-byte purpose-specific subkey from the machine key via
    /// HKDF-SHA256 with a domain-separation `info` string (e.g.
    /// `b"spiritstream/pii-filter/phrase-id/v1"`). Same construction the
    /// audit log uses for its HMAC chain key — one machine key, many
    /// independent subkeys, no cross-purpose reuse.
    pub fn derive_machine_subkey(
        app_data_dir: &Path,
        info: &[u8],
    ) -> Result<Zeroizing<[u8; KEY_LEN]>, CoreError> {
        let machine_key = get_or_create_machine_key(app_data_dir)?;
        let hk = hkdf::Hkdf::<sha2::Sha256>::new(None, &*machine_key);
        let mut out = Zeroizing::new([0u8; KEY_LEN]);
        hk.expand(info, &mut *out).map_err(|e| CoreError::Internal {
            context: format!("HKDF expand failed: {e}"),
        })?;
        Ok(out)
    }

    /// Encrypt a stream key for storage. Always writes V2 (`ENC2::` prefix,
    /// AES-256-GCM-SIV). Already-encrypted values (V1 or V2 prefix) and
    /// empty strings pass through unchanged.
    pub fn encrypt_stream_key(stream_key: &str, app_data_dir: &Path) -> Result<String, CoreError> {
        if stream_key.is_empty() || Self::is_stream_key_encrypted(stream_key) {
            return Ok(stream_key.to_string());
        }

        let machine_key = get_or_create_machine_key(app_data_dir)?;
        let blob = encrypt_with_machine_key_v2(&machine_key, stream_key.as_bytes())?;
        Ok(format!("{}{}", STREAM_KEY_PREFIX_V2, BASE64.encode(&blob)))
    }

    /// Decrypt a stream key from storage. Recognises both V1 (`ENC::`) and
    /// V2 (`ENC2::`) prefixes; plaintext values pass through unchanged.
    pub fn decrypt_stream_key(
        encrypted_key: &str,
        app_data_dir: &Path,
    ) -> Result<String, CoreError> {
        if let Some(encoded) = encrypted_key.strip_prefix(STREAM_KEY_PREFIX_V2) {
            let machine_key = get_or_create_machine_key(app_data_dir)?;
            decode_and_decrypt_v2(&machine_key, encoded)
        } else if let Some(encoded) = encrypted_key.strip_prefix(STREAM_KEY_PREFIX_V1) {
            let machine_key = get_or_create_machine_key(app_data_dir)?;
            decode_and_decrypt_v1(&machine_key, encoded)
        } else {
            Ok(encrypted_key.to_string())
        }
    }

    /// Check if a stream key carries an encryption prefix (either version).
    pub fn is_stream_key_encrypted(stream_key: &str) -> bool {
        stream_key.starts_with(STREAM_KEY_PREFIX_V1) || stream_key.starts_with(STREAM_KEY_PREFIX_V2)
    }

    /// Encrypt a sensitive token for storage (OAuth tokens, API keys, etc.).
    /// Returns base64-encoded encrypted value with `ENC2::` prefix.
    pub fn encrypt_token(token: &str, app_data_dir: &Path) -> Result<String, CoreError> {
        Self::encrypt_stream_key(token, app_data_dir)
    }

    /// Decrypt a sensitive token from storage. Returns the original plaintext.
    pub fn decrypt_token(encrypted: &str, app_data_dir: &Path) -> Result<String, CoreError> {
        Self::decrypt_stream_key(encrypted, app_data_dir)
    }

    /// Check if a value is encrypted (carries V1 `ENC::` or V2 `ENC2::` prefix).
    pub fn is_encrypted(value: &str) -> bool {
        Self::is_stream_key_encrypted(value)
    }

    // =========================================================================
    // Byte-level machine-key encryption.
    // Used by `EncryptedFileSecretStore` to encrypt arbitrary binary
    // secret payloads at rest. Layout: `[nonce:12][V2 GCM-SIV ciphertext]`.
    // There is no V1 path here — everything that uses this API writes V2.
    // =========================================================================

    /// Encrypt arbitrary bytes under the per-machine key (V2 GCM-SIV).
    pub fn encrypt_bytes_with_machine_key(
        data: &[u8],
        app_data_dir: &Path,
    ) -> Result<Vec<u8>, CoreError> {
        let machine_key = get_or_create_machine_key(app_data_dir)?;
        encrypt_with_machine_key_v2(&machine_key, data)
    }

    /// Decrypt bytes produced by [`Self::encrypt_bytes_with_machine_key`].
    pub fn decrypt_bytes_with_machine_key(
        data: &[u8],
        app_data_dir: &Path,
    ) -> Result<Vec<u8>, CoreError> {
        if data.len() < NONCE_LEN {
            return Err(CoreError::Internal {
                context: "encrypted bytes too short".into(),
            });
        }
        let machine_key = get_or_create_machine_key(app_data_dir)?;
        let (nonce_bytes, ciphertext) = data.split_at(NONCE_LEN);
        let cipher = Aes256GcmSiv::new_from_slice(&*machine_key)
            .map_err(|e| internal("Failed to create cipher", e))?;
        let nonce = aes_gcm_siv::Nonce::from_slice(nonce_bytes);
        cipher
            .decrypt(nonce, ciphertext)
            .map_err(|e| internal("bytes decryption failed", e))
    }
}

/// Read or create the per-machine key. On disk at `app_data_dir/.stream_key`,
/// 32 bytes, 0600 on Unix / hidden+system on Windows. First call creates the
/// file atomically; subsequent calls re-tighten permissions in case ownership
/// drifted.
pub(super) fn get_or_create_machine_key(
    app_data_dir: &Path,
) -> Result<Zeroizing<[u8; KEY_LEN]>, CoreError> {
    let key_file = app_data_dir.join(".stream_key");

    // A pending `.stream_key.new` means a rotation was interrupted and
    // `recover_interrupted_rotation` has not run yet. Minting or reading
    // a key in that state is how the pre-fix code destroyed installs: a
    // fresh random key silently replaced the journaled one and every
    // secret decrypted to garbage. Fail loud instead.
    if super::rotation::pending_key_path(app_data_dir).exists() {
        return Err(CoreError::Internal {
            context: "interrupted machine-key rotation detected (.stream_key.new present); \
                      run startup recovery before using the machine key"
                .into(),
        });
    }

    // TOCTOU-safe: try to atomically create the file (O_EXCL on Unix,
    // CREATE_NEW on Windows). On `AlreadyExists`, fall through to the
    // read path. Pre-fix this was `if key_file.exists() { read } else
    // { write }` — two concurrent first-launches of the same install
    // (double-clicked app, etc) both passed the exists() check, both
    // generated different random keys, both wrote, second-writer-wins.
    // Every profile encrypted under the loser's key was then orphaned.
    loop {
        let create_result = {
            let mut opts = std::fs::OpenOptions::new();
            opts.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                // Create with 0600 in one syscall so there's no window
                // where the file is world-readable.
                opts.mode(0o600);
            }
            opts.open(&key_file)
        };

        match create_result {
            Ok(mut f) => {
                use std::io::Write;
                let mut rng = rand::thread_rng();
                let mut key_bytes: [u8; KEY_LEN] = rng.gen();
                f.write_all(&key_bytes)
                    .map_err(|e| internal("Failed to write machine key", e))?;
                // Drop the FD before fiddling with Windows attributes —
                // some attribute-set APIs are unhappy with open handles.
                drop(f);
                #[cfg(windows)]
                {
                    set_windows_key_attributes(&key_file)?;
                }
                let mut key = Zeroizing::new([0u8; KEY_LEN]);
                key.copy_from_slice(&key_bytes);
                key_bytes.zeroize();
                return Ok(key);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                // Race-lost — another process beat us to creating the
                // file. Fall through to read its key.
            }
            Err(e) => return Err(internal("Failed to create machine key file", e)),
        }

        match std::fs::read(&key_file) {
            Ok(mut key_data) => {
                if key_data.len() != KEY_LEN {
                    key_data.zeroize();
                    return Err(CoreError::Internal {
                        context: "Invalid machine key file".into(),
                    });
                }
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let perms = std::fs::Permissions::from_mode(0o600);
                    std::fs::set_permissions(&key_file, perms)
                        .map_err(|e| internal("Failed to set key file permissions", e))?;
                }
                #[cfg(windows)]
                {
                    set_windows_key_attributes(&key_file)?;
                }
                let mut key = Zeroizing::new([0u8; KEY_LEN]);
                key.copy_from_slice(&key_data);
                key_data.zeroize();
                return Ok(key);
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                // Extremely unlikely: the file we just lost the race to
                // create has been deleted between our failed create_new
                // and our read. Restart the loop.
                continue;
            }
            Err(e) => return Err(internal("Failed to read machine key", e)),
        }
    }
}

#[cfg(windows)]
fn set_windows_key_attributes(key_file: &Path) -> Result<(), CoreError> {
    use std::os::windows::fs::MetadataExt;

    let metadata =
        std::fs::metadata(key_file).map_err(|e| internal("Failed to read key file metadata", e))?;
    let mut attributes = metadata.file_attributes();

    const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
    const FILE_ATTRIBUTE_SYSTEM: u32 = 0x4;

    attributes |= FILE_ATTRIBUTE_HIDDEN | FILE_ATTRIBUTE_SYSTEM;

    use std::os::windows::ffi::OsStrExt;
    let wide_path: Vec<u16> = key_file.as_os_str().encode_wide().chain(Some(0)).collect();

    unsafe {
        if winapi::um::fileapi::SetFileAttributesW(wide_path.as_ptr(), attributes) == 0 {
            return Err(CoreError::Internal {
                context: "Failed to set Windows file attributes".into(),
            });
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Machine-key inner helpers — V1Gcm read path, V2GcmSiv write+read path.
// Take the raw 32-byte key (Zeroizing-wrapped) so they can be shared by both
// the machine-key encryption API above and the rotation flow in `rotation.rs`.
// ---------------------------------------------------------------------------

pub(super) fn encrypt_with_machine_key_v2(
    machine_key: &Zeroizing<[u8; KEY_LEN]>,
    plaintext: &[u8],
) -> Result<Vec<u8>, CoreError> {
    let mut rng = rand::thread_rng();
    let nonce_bytes: [u8; NONCE_LEN] = rng.gen();

    let cipher = Aes256GcmSiv::new_from_slice(&**machine_key)
        .map_err(|e| internal("Failed to create cipher", e))?;
    let nonce = aes_gcm_siv::Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, plaintext)
        .map_err(|e| internal("Stream key encryption failed", e))?;

    let mut combined = Vec::with_capacity(NONCE_LEN + ciphertext.len());
    combined.extend_from_slice(&nonce_bytes);
    combined.extend_from_slice(&ciphertext);
    Ok(combined)
}

pub(super) fn decode_and_decrypt_v2(
    machine_key: &Zeroizing<[u8; KEY_LEN]>,
    encoded_body: &str,
) -> Result<String, CoreError> {
    let mut combined = BASE64
        .decode(encoded_body)
        .map_err(|e| internal("Failed to decode encrypted stream key", e))?;

    if combined.len() < NONCE_LEN {
        combined.zeroize();
        return Err(CoreError::Internal {
            context: "Invalid encrypted stream key".into(),
        });
    }

    let (nonce_bytes, ciphertext) = combined.split_at(NONCE_LEN);
    let cipher = Aes256GcmSiv::new_from_slice(&**machine_key)
        .map_err(|e| internal("Failed to create cipher", e))?;
    let nonce = aes_gcm_siv::Nonce::from_slice(nonce_bytes);

    let mut plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| internal("Stream key decryption failed", e))?;

    let result = String::from_utf8(plaintext.clone())
        .map_err(|e| internal("Invalid UTF-8 in decrypted stream key", e));

    plaintext.zeroize();
    combined.zeroize();
    result
}

pub(super) fn decode_and_decrypt_v1(
    machine_key: &Zeroizing<[u8; KEY_LEN]>,
    encoded_body: &str,
) -> Result<String, CoreError> {
    let mut combined = BASE64
        .decode(encoded_body)
        .map_err(|e| internal("Failed to decode encrypted stream key", e))?;

    if combined.len() < NONCE_LEN {
        combined.zeroize();
        return Err(CoreError::Internal {
            context: "Invalid encrypted stream key".into(),
        });
    }

    let (nonce_bytes, ciphertext) = combined.split_at(NONCE_LEN);
    let cipher = Aes256Gcm::new_from_slice(&**machine_key)
        .map_err(|e| internal("Failed to create cipher", e))?;
    let nonce = Nonce::from_slice(nonce_bytes);

    let mut plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| internal("Stream key decryption failed", e))?;

    let result = String::from_utf8(plaintext.clone())
        .map_err(|e| internal("Invalid UTF-8 in decrypted stream key", e));

    plaintext.zeroize();
    combined.zeroize();
    result
}
