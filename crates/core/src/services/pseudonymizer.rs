//! Anonymous-mode pseudonymization.
//!
//! Replaces identifying strings — chat usernames, OAuth account labels
//! — with stable HMAC-SHA256 outputs derived from a per-profile salt.
//! The same plaintext under the same salt always produces the same
//! pseudonym, so a chat session reads coherently. A salt-leaked log
//! reverses back to plaintext only for plaintexts the attacker already
//! has on hand to test against; the salt-protected hash is one-way.
//!
//! # Format
//!
//! `pseudonymize("alice", salt) → "hash:abcd1234ef567890"`
//!
//! 16-char prefix of hex-encoded HMAC-SHA256. The `hash:` literal
//! marks the value as pseudonymised so a future migration / log
//! reader can recognise it without ambiguity (a real chat username
//! never starts with `hash:` followed by 16 hex chars).
//!
//! # Decode
//!
//! There is intentionally no `decode()` function — the construction is
//! one-way. The UI's "show me real usernames" affordance works by
//! re-pseudonymising candidate plaintexts the user already has stored
//! locally (their connected accounts) and comparing against logged
//! hashes via [`matches`].

use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

const HASH_PREFIX: &str = "hash:";
const HASH_PREFIX_LEN: usize = 16; // hex chars of HMAC prefix surfaced

/// Generate a fresh 32-byte salt suitable for [`pseudonymize`]. Returns
/// the salt hex-encoded so it can live on the [`Profile`] struct as a
/// plain string (and survive a JSON round-trip without bigint dance).
pub fn generate_salt() -> String {
    use rand::RngCore;
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    hex::encode(bytes)
}

/// Pseudonymise `value` under the hex-encoded `salt`. Returns the
/// salted hash in the documented `hash:<16hex>` format. Returns the
/// input unchanged if `salt` is empty or malformed — callers that
/// have already decided to apply anonymous mode are responsible for
/// passing a valid salt.
pub fn pseudonymize(value: &str, salt_hex: &str) -> String {
    let Some(salt) = decode_salt(salt_hex) else {
        return value.to_string();
    };
    let mut mac = HmacSha256::new_from_slice(&salt).expect("HMAC accepts any key length");
    mac.update(value.as_bytes());
    let digest = mac.finalize().into_bytes();
    let encoded = hex::encode(&digest[..(HASH_PREFIX_LEN / 2)]);
    format!("{HASH_PREFIX}{encoded}")
}

/// Test whether `candidate` plaintext would produce `pseudonym` under
/// `salt`. The UI uses this to decode hashes back to plaintexts the
/// user already knows about (their connected account usernames).
pub fn matches(candidate: &str, salt_hex: &str, pseudonym: &str) -> bool {
    pseudonymize(candidate, salt_hex) == pseudonym
}

/// Check whether a value is in the pseudonymised format. Lets the UI
/// branch between "show as-is" and "show with decode affordance"
/// without needing the salt.
pub fn looks_pseudonymized(value: &str) -> bool {
    let Some(rest) = value.strip_prefix(HASH_PREFIX) else {
        return false;
    };
    rest.len() == HASH_PREFIX_LEN && rest.chars().all(|c| c.is_ascii_hexdigit())
}

fn decode_salt(salt_hex: &str) -> Option<Vec<u8>> {
    if salt_hex.is_empty() {
        return None;
    }
    hex::decode(salt_hex).ok().filter(|b| !b.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pseudonymize_is_deterministic_under_same_salt() {
        let salt = generate_salt();
        let a = pseudonymize("alice", &salt);
        let b = pseudonymize("alice", &salt);
        assert_eq!(a, b, "same input + same salt must yield same pseudonym");
    }

    #[test]
    fn pseudonymize_differs_across_salts() {
        let s1 = generate_salt();
        let s2 = generate_salt();
        assert_ne!(s1, s2, "salts must be unique per call");
        let a = pseudonymize("alice", &s1);
        let b = pseudonymize("alice", &s2);
        assert_ne!(
            a, b,
            "same name + different salt must yield different pseudonyms"
        );
    }

    #[test]
    fn pseudonymize_differs_across_inputs_under_same_salt() {
        let salt = generate_salt();
        let alice = pseudonymize("alice", &salt);
        let bob = pseudonymize("bob", &salt);
        assert_ne!(
            alice, bob,
            "different inputs must yield different pseudonyms"
        );
    }

    #[test]
    fn pseudonymize_format_is_hash_prefix_plus_16_hex() {
        let salt = generate_salt();
        let out = pseudonymize("user", &salt);
        assert!(out.starts_with("hash:"));
        assert_eq!(out.len(), "hash:".len() + 16);
        assert!(out[5..].chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn empty_salt_returns_input_unchanged() {
        assert_eq!(pseudonymize("alice", ""), "alice");
    }

    #[test]
    fn malformed_salt_returns_input_unchanged() {
        // hex::decode("xyz") fails — not hex.
        assert_eq!(pseudonymize("alice", "xyz"), "alice");
    }

    #[test]
    fn matches_returns_true_for_correct_candidate() {
        let salt = generate_salt();
        let pseudonym = pseudonymize("alice", &salt);
        assert!(matches("alice", &salt, &pseudonym));
    }

    #[test]
    fn matches_returns_false_for_wrong_candidate() {
        let salt = generate_salt();
        let pseudonym = pseudonymize("alice", &salt);
        assert!(!matches("bob", &salt, &pseudonym));
    }

    #[test]
    fn looks_pseudonymized_recognises_real_pseudonyms() {
        let salt = generate_salt();
        let p = pseudonymize("alice", &salt);
        assert!(looks_pseudonymized(&p));
    }

    #[test]
    fn looks_pseudonymized_rejects_plain_usernames() {
        assert!(!looks_pseudonymized("alice"));
        assert!(!looks_pseudonymized("hash:abc")); // too short
        assert!(!looks_pseudonymized("hash:1234567890abcdefXX")); // not hex
    }

    #[test]
    fn generate_salt_is_64_hex_chars() {
        // 32 bytes hex-encoded = 64 chars.
        assert_eq!(generate_salt().len(), 64);
    }
}
