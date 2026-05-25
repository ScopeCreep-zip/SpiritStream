use argon2::{Algorithm, Argon2, Params, Version};
use zeroize::Zeroizing;

use crate::errors::CoreError;

use super::{internal, KEY_LEN};

/// Derive a 32-byte AES key from a password using Argon2id.
///
/// Parameters: m=64 MiB, t=3, p=4 — RFC 9106 memory-constrained
/// recommendation, exceeds OWASP 2025's interactive baseline. The
/// returned key is wrapped in `Zeroizing` so the caller can't
/// accidentally hold the bytes past their scope.
pub(super) fn derive_key(
    password: &str,
    salt: &[u8],
) -> Result<Zeroizing<[u8; KEY_LEN]>, CoreError> {
    let mut key = Zeroizing::new([0u8; KEY_LEN]);

    let params = Params::new(
        65536, // m_cost: 64 MiB
        3,     // t_cost: 3 iterations
        4,     // p_cost: 4 parallel threads
        None,  // output length (Argon2id default)
    )
    .map_err(|e| internal("Failed to create Argon2 params", e))?;

    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

    argon2
        .hash_password_into(password.as_bytes(), salt, &mut *key)
        .map_err(|e| internal("Key derivation failed", e))?;

    Ok(key)
}
