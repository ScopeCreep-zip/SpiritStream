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
//! hashes via [`matches()`].

use hmac::{Hmac, Mac};
use sha2::Sha256;

use crate::errors::CoreError;

type HmacSha256 = Hmac<Sha256>;

const HASH_PREFIX: &str = "hash:";
const HASH_PREFIX_LEN: usize = 16; // hex chars of HMAC prefix surfaced

/// Generate a fresh 32-byte salt suitable for [`pseudonymize`]. Returns
/// the salt hex-encoded so it can live on the `Profile` struct as a
/// plain string (and survive a JSON round-trip without bigint dance).
pub fn generate_salt() -> String {
    use rand::RngCore;
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    hex::encode(bytes)
}

/// Pseudonymise `value` under the hex-encoded `salt`. Returns the
/// salted hash in the documented `hash:<16hex>` format.
///
/// Fails with [`CoreError::AnonymousSaltInvalid`] when `salt` is empty
/// or malformed. It used to return the input unchanged in that case —
/// a silent fallback in the one feature that exists to protect
/// identities: anonymous mode appeared active while real usernames
/// flowed to logs and the event stream. Callers must treat the error
/// as "drop this message", never "pass it through".
pub fn pseudonymize(value: &str, salt_hex: &str) -> Result<String, CoreError> {
    let digest = hmac_digest(value, salt_hex)?;
    let encoded = hex::encode(&digest[..(HASH_PREFIX_LEN / 2)]);
    Ok(format!("{HASH_PREFIX}{encoded}"))
}

/// Friendly, human-readable pseudonym for DISPLAY in the chat feed —
/// `Adjective + Animal + NN` (e.g. `QuietOtter47`). Derived from the same
/// keyed HMAC as [`pseudonymize`], so it's deterministic (same viewer →
/// same label all session) and one-way. It is intentionally lower-entropy
/// than the canonical `hash:` form: two viewers can occasionally share a
/// label, which is harmless for display. The full-entropy `pseudonymize`
/// hash remains the canonical id for logs, moderation correlation, and
/// re-identification — never use this label for those.
pub fn friendly_label(value: &str, salt_hex: &str) -> Result<String, CoreError> {
    let digest = hmac_digest(value, salt_hex)?;
    let adj = ADJECTIVES[u16::from_be_bytes([digest[0], digest[1]]) as usize % ADJECTIVES.len()];
    let animal = ANIMALS[u16::from_be_bytes([digest[2], digest[3]]) as usize % ANIMALS.len()];
    let number = u16::from_be_bytes([digest[4], digest[5]]) % 100;
    Ok(format!("{adj}{animal}{number:02}"))
}

/// Keyed HMAC-SHA256 of `value` under the hex `salt`. Single source of the
/// salted digest shared by [`pseudonymize`] and [`friendly_label`].
fn hmac_digest(value: &str, salt_hex: &str) -> Result<[u8; 32], CoreError> {
    let Some(salt) = decode_salt(salt_hex) else {
        return Err(CoreError::AnonymousSaltInvalid);
    };
    let mut mac = HmacSha256::new_from_slice(&salt).expect("HMAC accepts any key length");
    mac.update(value.as_bytes());
    Ok(mac.finalize().into_bytes().into())
}

/// Curated, unmistakably-not-a-real-name word lists for [`friendly_label`].
/// 32 × 32 × 100 = 102,400 combinations — readable, distinct at a glance,
/// and never offensive (these names get shown on stream).
const ADJECTIVES: &[&str] = &[
    "Quiet", "Brave", "Calm", "Clever", "Cosmic", "Gentle", "Jolly", "Lucky", "Mellow", "Nimble",
    "Plucky", "Quirky", "Rapid", "Sunny", "Swift", "Witty", "Bold", "Breezy", "Cheery", "Dapper",
    "Eager", "Fuzzy", "Glossy", "Happy", "Ivory", "Keen", "Lively", "Merry", "Noble", "Peppy",
    "Royal", "Zesty",
];
const ANIMALS: &[&str] = &[
    "Otter", "Puma", "Finch", "Heron", "Lynx", "Moth", "Newt", "Owl", "Panda", "Quail", "Raven",
    "Seal", "Tapir", "Vole", "Wren", "Yak", "Bison", "Crane", "Dingo", "Egret", "Ferret", "Gecko",
    "Hare", "Ibex", "Koala", "Llama", "Marten", "Numbat", "Osprey", "Possum", "Robin", "Stoat",
];

/// Test whether `candidate` plaintext would produce `pseudonym` under
/// `salt`. The UI uses this to decode hashes back to plaintexts the
/// user already knows about (their connected account usernames). The
/// comparison is constant-time on the byte representation so the time
/// taken doesn't leak how many bytes of the pseudonym a candidate
/// matched — `String == String` short-circuits at the first differing
/// byte, which over many candidate calls is a character-by-character
/// timing oracle for an attacker who can measure the response.
pub fn matches(candidate: &str, salt_hex: &str, pseudonym: &str) -> bool {
    use subtle::ConstantTimeEq;
    // An invalid salt can't have produced any pseudonym — no match.
    let Ok(derived) = pseudonymize(candidate, salt_hex) else {
        return false;
    };
    // `pseudonymize` always returns a fixed-shape string (HASH_PREFIX +
    // HASH_PREFIX_LEN hex chars), so a length mismatch means the
    // candidate input couldn't have produced this pseudonym at all —
    // bail before ct_eq to avoid mixing different lengths.
    if derived.len() != pseudonym.len() {
        return false;
    }
    derived.as_bytes().ct_eq(pseudonym.as_bytes()).into()
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
        let a = pseudonymize("alice", &salt).unwrap();
        let b = pseudonymize("alice", &salt).unwrap();
        assert_eq!(a, b, "same input + same salt must yield same pseudonym");
    }

    #[test]
    fn pseudonymize_differs_across_salts() {
        let s1 = generate_salt();
        let s2 = generate_salt();
        assert_ne!(s1, s2, "salts must be unique per call");
        let a = pseudonymize("alice", &s1).unwrap();
        let b = pseudonymize("alice", &s2).unwrap();
        assert_ne!(
            a, b,
            "same name + different salt must yield different pseudonyms"
        );
    }

    #[test]
    fn pseudonymize_differs_across_inputs_under_same_salt() {
        let salt = generate_salt();
        let alice = pseudonymize("alice", &salt).unwrap();
        let bob = pseudonymize("bob", &salt).unwrap();
        assert_ne!(
            alice, bob,
            "different inputs must yield different pseudonyms"
        );
    }

    #[test]
    fn pseudonymize_format_is_hash_prefix_plus_16_hex() {
        let salt = generate_salt();
        let out = pseudonymize("user", &salt).unwrap();
        assert!(out.starts_with("hash:"));
        assert_eq!(out.len(), "hash:".len() + 16);
        assert!(out[5..].chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn empty_salt_fails_loud() {
        assert!(matches!(
            pseudonymize("alice", ""),
            Err(CoreError::AnonymousSaltInvalid)
        ));
    }

    #[test]
    fn malformed_salt_fails_loud() {
        // hex::decode("xyz") fails — not hex.
        assert!(matches!(
            pseudonymize("alice", "xyz"),
            Err(CoreError::AnonymousSaltInvalid)
        ));
    }

    #[test]
    fn matches_returns_true_for_correct_candidate() {
        let salt = generate_salt();
        let pseudonym = pseudonymize("alice", &salt).unwrap();
        assert!(matches("alice", &salt, &pseudonym));
    }

    #[test]
    fn matches_returns_false_for_wrong_candidate() {
        let salt = generate_salt();
        let pseudonym = pseudonymize("alice", &salt).unwrap();
        assert!(!matches("bob", &salt, &pseudonym));
    }

    #[test]
    fn matches_returns_false_for_invalid_salt() {
        assert!(!matches("alice", "", "hash:1234567890abcdef"));
    }

    #[test]
    fn looks_pseudonymized_recognises_real_pseudonyms() {
        let salt = generate_salt();
        let p = pseudonymize("alice", &salt).unwrap();
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

    #[test]
    fn friendly_label_is_deterministic_and_readable() {
        let salt = generate_salt();
        let a = friendly_label("alice", &salt).unwrap();
        let b = friendly_label("alice", &salt).unwrap();
        assert_eq!(a, b, "same viewer + salt must yield the same label");
        // Adjective+Animal+NN, no `hash:` prefix, ends in two digits.
        assert!(!a.starts_with("hash:"));
        assert!(a[a.len() - 2..].chars().all(|c| c.is_ascii_digit()));
        assert!(a.chars().next().unwrap().is_ascii_uppercase());
    }

    #[test]
    fn friendly_label_differs_across_inputs_and_salts() {
        let salt = generate_salt();
        // Different viewers almost always differ (102k label space).
        assert_ne!(
            friendly_label("alice", &salt).unwrap(),
            friendly_label("bob", &salt).unwrap()
        );
        // Same viewer under a different salt differs.
        let salt2 = generate_salt();
        assert_ne!(
            friendly_label("alice", &salt).unwrap(),
            friendly_label("alice", &salt2).unwrap()
        );
    }

    #[test]
    fn friendly_label_fails_loud_on_bad_salt() {
        assert!(matches!(
            friendly_label("alice", ""),
            Err(CoreError::AnonymousSaltInvalid)
        ));
    }
}
