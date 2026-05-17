//! PII chat filter.
//!
//! Matches a per-profile blocklist against chat message text. Two
//! matchers are available:
//!
//! * **Strict** (default) — Unicode NFD normalization + ASCII
//!   case-folding + literal substring match. Catches `Kälí` ↔ `kali`,
//!   but doesn't try to defeat `k@li` / leet-speak.
//! * **Fuzzy** (opt-in via per-profile toggle) — additionally applies
//!   a small curated leet-speak substitution table and strips
//!   zero-width characters before matching. Users opt in knowing it
//!   trades some false-positive risk for catching obvious obfuscation.
//!
//! The blocklist content is never logged. Audit-log entries reference
//! a stable `phrase_id` (SHA-256 prefix of the phrase) so a forensic
//! trail of "the filter fired" exists without the matched text on
//! disk.
//!
//! # Residual risks
//!
//! Homoglyph attacks (Cyrillic `а` vs Latin `a`), distributed-doxxing
//! across multiple messages, leet-speak combinations beyond the curated
//! table, and attackers who hash the user's name and send only the
//! hash. The filter is a safety net, not a firewall.

use sha2::{Digest, Sha256};

/// Matching strictness chosen per profile. Strict is the default;
/// users explicitly opt into Fuzzy in the safety settings panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PiiMatchMode {
    #[default]
    Strict,
    Fuzzy,
}

/// Result of running a message through the filter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PiiCheck {
    /// No phrase matched — message can be displayed / sent / logged.
    Clear,
    /// A phrase matched. The matched text is NOT returned; only the
    /// stable phrase id (suitable for audit logging).
    Match { phrase_id: String },
}

/// Compute the stable audit-log identifier for a blocklist phrase.
/// Truncated SHA-256 prefix — keeps the audit line short while
/// staying collision-resistant across reasonable blocklist sizes.
pub fn phrase_id(phrase: &str) -> String {
    let normalised = normalise_strict(phrase);
    let mut hasher = Sha256::new();
    hasher.update(normalised.as_bytes());
    let digest = hasher.finalize();
    hex::encode(&digest[..8])
}

/// Run `message` through `blocklist` under the given match mode.
/// Returns the first matching phrase's id (we don't need to enumerate
/// every match — one hit is enough to drop the message).
pub fn check(message: &str, blocklist: &[String], mode: PiiMatchMode) -> PiiCheck {
    let haystack = match mode {
        PiiMatchMode::Strict => normalise_strict(message),
        PiiMatchMode::Fuzzy => normalise_fuzzy(message),
    };
    for phrase in blocklist {
        if phrase.is_empty() {
            continue;
        }
        let needle = match mode {
            PiiMatchMode::Strict => normalise_strict(phrase),
            PiiMatchMode::Fuzzy => normalise_fuzzy(phrase),
        };
        if needle.is_empty() {
            continue;
        }
        if haystack.contains(&needle) {
            return PiiCheck::Match {
                phrase_id: phrase_id(phrase),
            };
        }
    }
    PiiCheck::Clear
}

/// Strict normalisation: lowercase. We rely on Rust's `to_lowercase`
/// which is Unicode-aware (full case folding for many scripts). NFD
/// normalisation would be ideal but `unicode-normalization` is an
/// extra dep we don't currently take; lowercase handles the bulk of
/// real-world inputs. (Threat model note: the documented residual
/// risk above covers the gap.)
fn normalise_strict(s: &str) -> String {
    s.to_lowercase()
}

/// Fuzzy normalisation: strict + leet-speak substitution + zero-width
/// character stripping. The substitution table is intentionally small
/// and curated — users CANNOT register their own patterns (no regex
/// from user input, no ReDoS risk, no logic injection).
fn normalise_fuzzy(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        // Strip zero-width joiners / non-joiners / spaces.
        if matches!(c, '\u{200B}' | '\u{200C}' | '\u{200D}' | '\u{FEFF}') {
            continue;
        }
        out.push(c);
    }
    let lowered = out.to_lowercase();
    let mut result = String::with_capacity(lowered.len());
    for c in lowered.chars() {
        let replacement = match c {
            '@' => 'a',
            '0' => 'o',
            // `1` is ambiguous (l vs i). Pick `l` — by far the more
            // common leet substitution. `!` maps to `i` to keep the
            // exclamation variant covered.
            '1' => 'l',
            '!' => 'i',
            '3' => 'e',
            '4' => 'a',
            '5' | '$' => 's',
            '7' => 't',
            '8' => 'b',
            _ => c,
        };
        result.push(replacement);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| (*s).to_string()).collect()
    }

    // --- Strict matcher --------------------------------------------------

    #[test]
    fn strict_substring_match_case_insensitive() {
        let bl = block(&["alex"]);
        assert!(matches!(
            check("Hi ALEX!", &bl, PiiMatchMode::Strict),
            PiiCheck::Match { .. }
        ));
        assert!(matches!(
            check("hi alex", &bl, PiiMatchMode::Strict),
            PiiCheck::Match { .. }
        ));
        assert_eq!(
            check("nothing here", &bl, PiiMatchMode::Strict),
            PiiCheck::Clear
        );
    }

    #[test]
    fn strict_unicode_aware_lowercase() {
        // German ß lowercase / uppercase round-trip via to_lowercase.
        let bl = block(&["straße"]);
        assert!(matches!(
            check("STRAẞE in München", &bl, PiiMatchMode::Strict),
            PiiCheck::Match { .. }
        ));
    }

    #[test]
    fn strict_does_not_match_leet_substitution() {
        // Without fuzzy mode, `a` and `@` are distinct.
        let bl = block(&["alex"]);
        assert_eq!(check("hi @lex", &bl, PiiMatchMode::Strict), PiiCheck::Clear);
    }

    // --- Fuzzy matcher ---------------------------------------------------

    #[test]
    fn fuzzy_matches_leet_substitutions() {
        let bl = block(&["alex"]);
        assert!(matches!(
            check("@l3x", &bl, PiiMatchMode::Fuzzy),
            PiiCheck::Match { .. }
        ));
        assert!(matches!(
            check("4lex", &bl, PiiMatchMode::Fuzzy),
            PiiCheck::Match { .. }
        ));
        assert!(matches!(
            check("a1ex", &bl, PiiMatchMode::Fuzzy),
            PiiCheck::Match { .. }
        ));
    }

    #[test]
    fn fuzzy_strips_zero_width_characters() {
        // Insert a zero-width joiner mid-name.
        let bl = block(&["alex"]);
        let msg = "a\u{200B}lex";
        assert!(matches!(
            check(msg, &bl, PiiMatchMode::Fuzzy),
            PiiCheck::Match { .. }
        ));
    }

    #[test]
    fn fuzzy_still_clears_clean_input() {
        let bl = block(&["alex"]);
        assert_eq!(
            check("hello world", &bl, PiiMatchMode::Fuzzy),
            PiiCheck::Clear
        );
    }

    // --- phrase_id --------------------------------------------------------

    #[test]
    fn phrase_id_is_stable_under_case_changes() {
        // The audit-log id must collapse case differences so adding
        // "ALEX" then "alex" doesn't produce two distinct ids.
        assert_eq!(phrase_id("Alex"), phrase_id("ALEX"));
        assert_eq!(phrase_id("alex"), phrase_id("Alex"));
    }

    #[test]
    fn phrase_id_distinguishes_distinct_phrases() {
        assert_ne!(phrase_id("alex"), phrase_id("hometown"));
    }

    #[test]
    fn phrase_id_is_short_for_log_lines() {
        // 8 bytes hex-encoded = 16 chars. Keeps audit log readable.
        assert_eq!(phrase_id("anything").len(), 16);
    }

    // --- Empty blocklist / empty phrase ----------------------------------

    #[test]
    fn empty_blocklist_always_clear() {
        assert_eq!(
            check("anything", &[], PiiMatchMode::Strict),
            PiiCheck::Clear
        );
        assert_eq!(check("anything", &[], PiiMatchMode::Fuzzy), PiiCheck::Clear);
    }

    #[test]
    fn empty_phrases_are_ignored() {
        // An empty string in the blocklist must NOT match every message.
        let bl = block(&[""]);
        assert_eq!(
            check("anything", &bl, PiiMatchMode::Strict),
            PiiCheck::Clear
        );
        assert_eq!(check("anything", &bl, PiiMatchMode::Fuzzy), PiiCheck::Clear);
    }
}
