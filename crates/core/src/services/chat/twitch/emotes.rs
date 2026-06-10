//! Twitch IRC `emotes=` tag parser.
//!
//! Wire format from `IRCv3 tags` on a PRIVMSG (Twitch's "Tags
//! capability" documentation):
//!
//! ```text
//! emotes=25:0-4,12-16/1902:6-10
//!        └┬┘ └─┬─┘    └┬┘ └─┬─┘
//!         │   range    │   range
//!         │            └ second emote
//!         └ first emote id
//! ```
//!
//! Each emote ID maps to one or more `start-end` ranges (inclusive on
//! both ends). Multiple emote IDs are separated by `/`. Multiple
//! ranges for the same ID are separated by `,`.
//!
//! # The codepoint-indexing trap
//!
//! Twitch's published spec says the indices are **"character
//! indexes"**. The historical Twitch IRC `emotes=` tag indexes
//! by **Unicode codepoint**, not bytes and not UTF-16 code units.
//! See `twitchdev/issues#104` — Twitch chose `String.codePointAt`-style
//! semantics, so a message body with a non-BMP emoji (`👋` is one
//! codepoint, two UTF-16 code units, four UTF-8 bytes) bumps every
//! subsequent emote index by exactly **one**.
//!
//! Implication: every consumer must walk `message.chars()` (which
//! yields `char` = one Unicode scalar value) and slice by codepoint
//! offset. Slicing the UTF-8 byte string directly will mis-cut
//! whenever a multi-byte codepoint appears earlier in the message,
//! producing either garbled text or a panic on a non-character boundary.

use serde::{Deserialize, Serialize};

/// Raw range descriptor from a Twitch `emotes=` tag. Indices are
/// Unicode codepoint offsets into the message body, inclusive on
/// both ends — `start_inclusive..=end_inclusive`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawTwitchEmote {
    /// Twitch emote ID (e.g. `"25"` for `Kappa`).
    pub id: String,
    /// First codepoint offset of the emote token in `msg.chars()`.
    pub start: usize,
    /// Last codepoint offset of the emote token in `msg.chars()`.
    /// Inclusive — for a 5-character emote at position 0, this is 4.
    pub end_inclusive: usize,
}

impl RawTwitchEmote {
    /// Extract the emote's source text from the surrounding message
    /// using codepoint-safe slicing. Returns `None` if the range
    /// extends past the message body — a malformed tag we tolerate
    /// rather than panic on. Used by [`super::fragments::walk_body`]
    /// when building `MessageFragment::Emote` runs; tests rely on the
    /// same method so the slice contract is exercised end to end.
    pub fn text(&self, msg_chars: &[char]) -> Option<String> {
        if self.end_inclusive < self.start || self.end_inclusive >= msg_chars.len() {
            return None;
        }
        Some(msg_chars[self.start..=self.end_inclusive].iter().collect())
    }
}

/// Parse a Twitch IRC `emotes=` tag value into a flat list of
/// emote-range records. The output preserves the original ranges
/// (one `RawTwitchEmote` per range, with the emote ID duplicated
/// across ranges that share an ID) — callers that need to render
/// emote runs in message order can sort by `start`.
///
/// `msg_chars` must be the message body materialised as a `Vec<char>`
/// (via `message.chars().collect()`) so callers can reuse the
/// codepoint vector for the rest of their parsing pipeline (mention
/// detection, link extraction, fragment assembly).
///
/// Malformed segments — empty IDs, missing dashes, non-numeric
/// indices, ranges that exceed the message length — are skipped
/// silently. Twitch's documented format is well-formed but IRC
/// servers occasionally emit garbage tags during state-resets; the
/// connector should never crash on a single bad emote.
pub fn parse_twitch_emotes_tag(tag: &str, msg_chars: &[char]) -> Vec<RawTwitchEmote> {
    if tag.trim().is_empty() {
        return Vec::new();
    }
    let mut emotes: Vec<RawTwitchEmote> = Vec::new();
    let msg_len = msg_chars.len();
    for emote_spec in tag.split('/') {
        let Some((id, ranges)) = emote_spec.split_once(':') else {
            continue;
        };
        let id = id.trim();
        if id.is_empty() {
            continue;
        }
        for range in ranges.split(',') {
            let Some((start_str, end_str)) = range.split_once('-') else {
                continue;
            };
            let (Ok(start), Ok(end)) = (
                start_str.trim().parse::<usize>(),
                end_str.trim().parse::<usize>(),
            ) else {
                continue;
            };
            if end < start || end >= msg_len {
                continue;
            }
            emotes.push(RawTwitchEmote {
                id: id.to_string(),
                start,
                end_inclusive: end,
            });
        }
    }
    emotes
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chars(s: &str) -> Vec<char> {
        s.chars().collect()
    }

    #[test]
    fn empty_tag_returns_empty_vec() {
        assert!(parse_twitch_emotes_tag("", &chars("anything")).is_empty());
        assert!(parse_twitch_emotes_tag("   ", &chars("anything")).is_empty());
    }

    #[test]
    fn single_emote_single_range() {
        let msg = "Kappa";
        let parsed = parse_twitch_emotes_tag("25:0-4", &chars(msg));
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].id, "25");
        assert_eq!(parsed[0].start, 0);
        assert_eq!(parsed[0].end_inclusive, 4);
        assert_eq!(parsed[0].text(&chars(msg)).as_deref(), Some("Kappa"));
    }

    #[test]
    fn single_emote_multi_range() {
        let msg = "Kappa hi Kappa";
        let parsed = parse_twitch_emotes_tag("25:0-4,9-13", &chars(msg));
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].id, "25");
        assert_eq!(parsed[1].id, "25");
        assert_eq!(parsed[1].start, 9);
        assert_eq!(parsed[1].end_inclusive, 13);
    }

    #[test]
    fn multiple_emote_ids() {
        let msg = "Kappa PogChamp";
        let parsed = parse_twitch_emotes_tag("25:0-4/1902:6-13", &chars(msg));
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].id, "25");
        assert_eq!(parsed[1].id, "1902");
        assert_eq!(parsed[1].text(&chars(msg)).as_deref(), Some("PogChamp"));
    }

    /// The load-bearing codepoint-vs-byte test. `👋` is one Unicode
    /// codepoint but four UTF-8 bytes. A naive byte-indexed parser
    /// would think `PogChamp` starts at byte offset 13 instead of
    /// codepoint offset 11 and either panic on a non-char boundary
    /// or extract the wrong substring.
    #[test]
    fn codepoint_indices_survive_non_bmp_emoji() {
        // "hi Kappa 👋 PogChamp 🌈"
        //    0  1  2  3  4  5  6  7  8  9  10 11 12 13 14 15 16 17 18 19
        //    h  i     K  a  p  p  a     👋    P  o  g  C  h  a  m  p     🌈
        let msg = "hi Kappa 👋 PogChamp 🌈";
        let chars: Vec<char> = msg.chars().collect();
        // Sanity: codepoint count is 21, byte count is much higher.
        assert_eq!(chars.len(), 21);
        assert!(msg.len() > 21);

        let parsed = parse_twitch_emotes_tag("25:3-7/1902:11-18", &chars);
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].text(&chars).as_deref(), Some("Kappa"));
        assert_eq!(parsed[1].text(&chars).as_deref(), Some("PogChamp"));
    }

    /// Two non-BMP emojis sandwiching an emote — every codepoint
    /// after the first emoji shifts by 1 in codepoint terms but by
    /// 3 in byte terms. Byte-indexed parsing would offset by 9 bytes
    /// for the two emojis combined and land in the middle of a
    /// codepoint. This test pins the contract.
    #[test]
    fn codepoint_indices_handle_emoji_on_both_sides() {
        // "🌈 Kappa 👋"
        //  0  1  2  3  4  5  6  7  8
        //  🌈    K  a  p  p  a     👋
        let msg = "🌈 Kappa 👋";
        let chars: Vec<char> = msg.chars().collect();
        assert_eq!(chars.len(), 9);
        let parsed = parse_twitch_emotes_tag("25:2-6", &chars);
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].text(&chars).as_deref(), Some("Kappa"));
    }

    #[test]
    fn malformed_segments_are_skipped_not_fatal() {
        let msg = "Kappa";
        // Mix of valid + broken parts: missing colon, empty id, bad range,
        // out-of-range end, valid trailing.
        let parsed = parse_twitch_emotes_tag(
            "garbage/bad:notanumber-4/:0-4/9999:99-100/25:0-4",
            &chars(msg),
        );
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].id, "25");
    }

    #[test]
    fn end_less_than_start_is_rejected() {
        let msg = "Kappa";
        let parsed = parse_twitch_emotes_tag("25:4-0", &chars(msg));
        assert!(parsed.is_empty());
    }

    #[test]
    fn range_past_message_end_is_rejected() {
        let msg = "hi";
        let parsed = parse_twitch_emotes_tag("25:0-99", &chars(msg));
        assert!(parsed.is_empty());
    }

    #[test]
    fn text_returns_none_for_out_of_bounds() {
        let raw = RawTwitchEmote {
            id: "25".into(),
            start: 0,
            end_inclusive: 100,
        };
        assert_eq!(raw.text(&chars("hi")), None);
    }
}
