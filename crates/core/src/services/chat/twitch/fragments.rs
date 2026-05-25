//! Twitch PRIVMSG → structured [`ChatMessage`] builder.
//!
//! Walks the message body once, interleaving emote runs (from
//! `twitch_irc::PrivmsgMessage::emotes`, which already exposes
//! codepoint-safe ranges) with plain-text spans, mention detection
//! (`@user`), and URL extraction. Returns a fully-populated
//! `ChatMessage` with `fragments`, `flags`, `author`, `bits_total`,
//! `reply`, `elevated_tier`, plus the legacy `username` / `message` /
//! `timestamp` / `color` / `badges` fields preserved so older
//! chat-log JSONL files still round-trip.
//!
//! 3rd-party emotes (BTTV / FFZ / 7TV), badge URL resolution, and
//! cheermote tier-art assembly land in **Phase C** — this builder
//! emits Twitch native emotes only, and leaves resolved-`Badge`
//! fragment emission to that phase. Badge codes live on
//! `author.badges_raw` in the meantime.

use super::emotes::{parse_twitch_emotes_tag, RawTwitchEmote};
use crate::models::{
    ChatAuthor, ChatMessage, ChatMessageDirection, ChatPlatform as ChatPlatformEnum, ElevatedTier,
    EmoteProvider, FragmentColor, MessageFlags, MessageFragment, ReplyContext, TextStyle,
};
use twitch_irc::message::{PrivmsgMessage, RGBColor};

/// Twitch emote-CDN URL template. `id` is the emote ID; `size` is
/// `"1.0"`, `"2.0"`, or `"3.0"` (the Twitch v2 CDN scheme). Static
/// vs animated fallback is keyed on the format the CDN serves —
/// `default` returns either a PNG (static) or GIF (animated) based
/// on the emote's recorded format.
const TWITCH_EMOTE_CDN_PREFIX: &str = "https://static-cdn.jtvnw.net/emoticons/v2";

fn twitch_emote_url(id: &str, size: &str) -> String {
    format!("{}/{}/default/dark/{}", TWITCH_EMOTE_CDN_PREFIX, id, size)
}

fn rgb_to_hex(c: &RGBColor) -> String {
    format!("#{:02X}{:02X}{:02X}", c.r, c.g, c.b)
}

/// Plan: `pinned-chat-paid-level` tag value → [`ElevatedTier`]. The
/// nine documented Hype Chat tiers map 1:1; unknown values fall
/// through to `None`.
fn parse_elevated_tier(raw: &str) -> Option<ElevatedTier> {
    match raw {
        "ONE_MINUTE" => Some(ElevatedTier::OneMin),
        "FIVE_MINUTES" => Some(ElevatedTier::FiveMin),
        "TEN_MINUTES" => Some(ElevatedTier::TenMin),
        "THIRTY_MINUTES" => Some(ElevatedTier::ThirtyMin),
        "ONE_HOUR" => Some(ElevatedTier::OneHour),
        "TWO_HOURS" => Some(ElevatedTier::TwoHour),
        "THREE_HOURS" => Some(ElevatedTier::ThreeHour),
        "FOUR_HOURS" => Some(ElevatedTier::FourHour),
        "FIVE_HOURS" => Some(ElevatedTier::FiveHour),
        _ => None,
    }
}

/// Pull a raw IRC tag value off the underlying `IRCMessage`. The
/// `twitch_irc` library exposes high-level fields on
/// `PrivmsgMessage` for the common tags but leaves the rest on
/// `source.tags` (a `HashMap<String, Option<String>>`-like wrapper).
// Returns a slice borrowed from `msg.source.tags`, so the output
// lifetime is tied to `msg` (not `name`). Elision would resolve to
// the same thing here — Rust picks the `&self`-equivalent input
// when there's exactly one — but the explicit form documents the
// intent for the next reader.
#[allow(clippy::needless_lifetimes)]
fn raw_tag<'a>(msg: &'a PrivmsgMessage, name: &str) -> Option<&'a str> {
    msg.source.tags.0.get(name).and_then(|v| v.as_deref())
}

/// Pre-format the server timestamp as `HH:MM` in the user's local
/// time. The frontend renders the fragment verbatim — no locale
/// processing happens on the JS side.
fn format_timestamp(ts: chrono::DateTime<chrono::Utc>) -> String {
    use chrono::Local;
    ts.with_timezone(&Local).format("%H:%M").to_string()
}

/// Build a structured [`ChatMessage`] from a `twitch_irc`
/// `PrivmsgMessage`. See the module-level docs for the field-by-field
/// contract.
pub fn build_chat_message_from_privmsg(msg: &PrivmsgMessage) -> ChatMessage {
    let chars: Vec<char> = msg.message_text.chars().collect();
    let unix_ms = msg.server_timestamp.timestamp_millis();
    let formatted_ts = format_timestamp(msg.server_timestamp);

    // --- Fragments ---------------------------------------------------------

    let mut fragments: Vec<MessageFragment> = Vec::with_capacity(4 + msg.emotes.len() * 2);

    // 1. Reply preview (first fragment when this message is a reply).
    let reply_parent_id = raw_tag(msg, "reply-parent-msg-id").map(|s| s.to_string());
    let reply_parent_login = raw_tag(msg, "reply-parent-user-login").map(|s| s.to_string());
    let reply_parent_display = raw_tag(msg, "reply-parent-display-name").map(|s| s.to_string());
    let reply_parent_body = raw_tag(msg, "reply-parent-msg-body").map(|s| s.to_string());

    if let (Some(pid), Some(plogin), Some(pdisp), Some(pbody)) = (
        reply_parent_id.as_ref(),
        reply_parent_login.as_ref(),
        reply_parent_display.as_ref(),
        reply_parent_body.as_ref(),
    ) {
        fragments.push(MessageFragment::ReplyPreview {
            parent_message_id: pid.clone(),
            parent_login: plogin.clone(),
            parent_display_name: pdisp.clone(),
            parent_text_preview: truncate_preview(pbody, 80),
        });
    }

    // 2. Timestamp fragment — the renderer leans on backend-formatted
    //    strings so locale/timezone decisions live in core.
    fragments.push(MessageFragment::Timestamp {
        unix_ms,
        formatted: formatted_ts,
    });

    // 3. Body walk: interleave Emote / Mention / Link / Text runs.
    //    The raw IRC `emotes=` tag is the source of truth — parsed via
    //    `super::emotes::parse_twitch_emotes_tag` with codepoint-safe
    //    indices (see that module for the load-bearing docs).
    let raw_emote_tag = raw_tag(msg, "emotes").unwrap_or("");
    let twitch_emotes = parse_twitch_emotes_tag(raw_emote_tag, &chars);
    fragments.extend(walk_body(&chars, &twitch_emotes));

    // --- Flags -------------------------------------------------------------

    let mut flags = MessageFlags::empty();
    if msg.is_action {
        flags |= MessageFlags::ACTION;
    }
    if msg.bits.is_some() {
        flags |= MessageFlags::CHEER_MESSAGE;
    }
    if reply_parent_id.is_some() {
        flags |= MessageFlags::REPLY_MESSAGE;
    }
    if raw_tag(msg, "first-msg").is_some_and(|v| v == "1") {
        flags |= MessageFlags::FIRST_MESSAGE;
    }
    if raw_tag(msg, "pinned-chat-paid-amount").is_some() {
        flags |= MessageFlags::ELEVATED_MESSAGE;
    }
    if raw_tag(msg, "custom-reward-id").is_some() {
        flags |= MessageFlags::REDEEMED_CHANNEL_POINT_REWARD;
    }
    if raw_tag(msg, "msg-id").is_some_and(|v| v == "highlighted-message") {
        flags |= MessageFlags::HIGHLIGHTED;
    }

    // --- Author ------------------------------------------------------------

    let color = msg.name_color.as_ref().and_then(|c| FragmentColor::from_hex(&rgb_to_hex(c)));
    let legacy_color = color
        .as_ref()
        .map(|c| c.hex.clone())
        .unwrap_or_else(|| super::TWITCH_DEFAULT_USER_COLOR.to_string());

    let badges_raw: Vec<String> = msg
        .badges
        .iter()
        .map(|b| format!("{}/{}", b.name, b.version))
        .collect();

    let author = ChatAuthor {
        user_id: msg.sender.id.clone(),
        login: msg.sender.login.clone(),
        display_name: msg.sender.name.clone(),
        color: color.clone(),
        badges_raw: badges_raw.clone(),
    };

    // --- Optional structured fields ----------------------------------------

    let bits_total = msg.bits.and_then(|b| u32::try_from(b).ok());
    let elevated_tier = raw_tag(msg, "pinned-chat-paid-level").and_then(parse_elevated_tier);

    let reply = reply_parent_id.as_ref().map(|pid| ReplyContext {
        parent_message_id: pid.clone(),
        // Twitch's `reply-thread-parent-msg-id` is the root-of-thread
        // anchor; falls back to the direct parent for single-level replies.
        thread_root_id: raw_tag(msg, "reply-thread-parent-msg-id")
            .map(|s| s.to_string())
            .unwrap_or_else(|| pid.clone()),
    });

    // --- Assemble ----------------------------------------------------------

    ChatMessage {
        id: format!("twitch:{}", msg.message_id),
        platform: ChatPlatformEnum::Twitch,
        account_id: Some(msg.sender.id.clone()),
        channel_id: Some(msg.channel_id.clone()),
        platforms: None,
        username: msg.sender.name.clone(),
        message: msg.message_text.clone(),
        timestamp: unix_ms,
        server_received_at_ms: Some(unix_ms),
        author: Some(author),
        fragments,
        flags,
        raw_text: Some(msg.message_text.clone()),
        bits_total,
        highlight_color: None,
        elevated_tier,
        reply,
        event: None,
        direction: ChatMessageDirection::Inbound,
        source_id: Some(msg.message_id.clone()),
        // Legacy mirrors for log-JSONL round-trip:
        color: Some(legacy_color),
        badges: if badges_raw.is_empty() {
            None
        } else {
            Some(badges_raw)
        },
    }
}

fn truncate_preview(body: &str, max_chars: usize) -> String {
    let mut out = String::new();
    for (count, ch) in body.chars().enumerate() {
        if count >= max_chars {
            out.push('…');
            break;
        }
        if ch == '\n' || ch == '\r' {
            out.push(' ');
        } else {
            out.push(ch);
        }
    }
    out
}

/// Walk the message body's codepoint slice, emitting fragments in
/// message order. Emote ranges (already codepoint-safe per
/// [`parse_twitch_emotes_tag`]) get sliced first; the gaps between
/// them are scanned for `@mention` and bare URLs, with everything
/// else emitted as `Text` runs.
fn walk_body(chars: &[char], emotes: &[RawTwitchEmote]) -> Vec<MessageFragment> {
    let mut out: Vec<MessageFragment> = Vec::new();
    let mut sorted: Vec<&RawTwitchEmote> = emotes.iter().collect();
    sorted.sort_by_key(|e| e.start);

    let mut cursor: usize = 0;
    for emote in sorted {
        let start = emote.start;
        let end_excl = emote.end_inclusive + 1;
        if start < cursor || start >= chars.len() || end_excl > chars.len() {
            // Overlap with the previous emote we already consumed, or
            // out-of-bounds after the (already-validating) parser —
            // skip rather than panic on malformed concurrent ranges.
            continue;
        }
        if start > cursor {
            emit_text_run(&chars[cursor..start], &mut out);
        }
        // Use `RawTwitchEmote::text` so the codepoint-safe slicing
        // contract lives in exactly one place (the emote-parser
        // module) and is exercised end-to-end here. Defensive `?`
        // shouldn't trigger after the bounds check above; fall back
        // to the raw id so the fragment is still renderable.
        let name = emote
            .text(chars)
            .unwrap_or_else(|| emote.id.clone());
        out.push(MessageFragment::Emote {
            provider: EmoteProvider::Twitch,
            id: emote.id.clone(),
            name,
            // Animated/zero-width need a Helix `chat/emotes` lookup
            // (Phase C — emote-service cache); defaulted to false here.
            animated: false,
            zero_width: false,
            url_1x: twitch_emote_url(&emote.id, "1.0"),
            url_2x: twitch_emote_url(&emote.id, "2.0"),
            url_4x: Some(twitch_emote_url(&emote.id, "3.0")),
        });
        cursor = end_excl;
    }
    if cursor < chars.len() {
        emit_text_run(&chars[cursor..], &mut out);
    }
    out
}

/// Scan a slice of message text for `@mention` and bare URL tokens,
/// emitting `Mention` / `Link` / `Text` fragments. The frontend
/// renders the fragments verbatim — it never re-parses the body.
fn emit_text_run(run: &[char], out: &mut Vec<MessageFragment>) {
    if run.is_empty() {
        return;
    }
    let mut buffer = String::new();
    let mut i = 0;
    while i < run.len() {
        let c = run[i];
        // URL detection — accept `http://` and `https://` prefixes only,
        // terminated by whitespace or end-of-run. Twitch IRC bodies can
        // contain anything in the URL after the scheme, but real-world
        // links don't include spaces; this stays conservative.
        if (c == 'h' || c == 'H') && url_scheme_matches(&run[i..]) {
            // Flush pending text.
            if !buffer.is_empty() {
                out.push(MessageFragment::Text {
                    content: std::mem::take(&mut buffer),
                    color: None,
                    style: TextStyle::Normal,
                });
            }
            let url_end = i + url_end_offset(&run[i..]);
            let url_str: String = run[i..url_end].iter().collect();
            out.push(MessageFragment::Link {
                url: url_str.clone(),
                display: url_str,
                is_safe_browsing_flagged: false,
            });
            i = url_end;
            continue;
        }
        // Mention detection — only at a word boundary (start-of-run or
        // preceded by whitespace). Login chars are ASCII alnum + `_`,
        // 4-25 chars per Twitch policy.
        if c == '@' && (i == 0 || run[i - 1].is_whitespace()) {
            let mention_end = i + 1 + mention_length(&run[i + 1..]);
            if mention_end > i + 1 {
                if !buffer.is_empty() {
                    out.push(MessageFragment::Text {
                        content: std::mem::take(&mut buffer),
                        color: None,
                        style: TextStyle::Normal,
                    });
                }
                let display: String = run[i + 1..mention_end].iter().collect();
                let login = display.to_ascii_lowercase();
                out.push(MessageFragment::Mention {
                    login,
                    display_name: display,
                    user_color: None,
                });
                i = mention_end;
                continue;
            }
        }
        buffer.push(c);
        i += 1;
    }
    if !buffer.is_empty() {
        out.push(MessageFragment::Text {
            content: buffer,
            color: None,
            style: TextStyle::Normal,
        });
    }
}

fn url_scheme_matches(run: &[char]) -> bool {
    let s: String = run.iter().take(8).collect();
    s.starts_with("https://") || s.starts_with("http://")
}

fn url_end_offset(run: &[char]) -> usize {
    run.iter()
        .position(|c| c.is_whitespace())
        .unwrap_or(run.len())
}

fn mention_length(run: &[char]) -> usize {
    let mut n = 0;
    for c in run {
        if c.is_ascii_alphanumeric() || *c == '_' {
            n += 1;
        } else {
            break;
        }
    }
    n
}


#[cfg(test)]
mod tests {
    use super::*;
    use twitch_irc::message::IRCMessage;

    /// Build a complete PRIVMSG line for tests. `twitch_irc 5`'s parser
    /// requires `room-id`, `display-name`, `id`, `tmi-sent-ts`, and
    /// `user-id`; everything else gets defaulted/merged into `extra`.
    fn make_privmsg(extra_tags: &str, body: &str) -> PrivmsgMessage {
        // Mirror the canonical tag set the upstream `twitch-irc 5.x`
        // parser requires (see its own test fixtures); `try_get_*`
        // calls demand the bare minimum even with empty values.
        let base = "badge-info=;badges=;color=;display-name=Alice;emotes=;flags=;\
            id=z;mod=0;room-id=42;subscriber=0;tmi-sent-ts=1700000000000;\
            turbo=0;user-id=1;user-type=";
        let tag_block = if extra_tags.is_empty() {
            base.to_string()
        } else {
            format!("{};{}", base, extra_tags)
        };
        let raw = format!(
            "@{} :alice!alice@alice.tmi.twitch.tv PRIVMSG #channel :{}",
            tag_block, body
        );
        let irc = IRCMessage::parse(&raw).expect("valid IRC line");
        PrivmsgMessage::try_from(irc).expect("PRIVMSG parse")
    }

    /// Pins the structured-fragment + flag + author wiring end to end.
    #[test]
    fn builds_fragments_emote_and_flags_from_real_irc_line() {
        let msg = make_privmsg("color=#FF6699;emotes=25:9-13;first-msg=1", "Hi all Kappa folks");
        let built = build_chat_message_from_privmsg(&msg);

        assert_eq!(built.platform, ChatPlatformEnum::Twitch);
        assert_eq!(built.account_id.as_deref(), Some("1"));
        assert_eq!(built.channel_id.as_deref(), Some("42"));
        let author = built.author.as_ref().expect("author");
        assert_eq!(author.login, "alice");
        assert_eq!(author.color.as_ref().map(|c| c.hex.as_str()), Some("#FF6699"));
        assert!(built.flags.contains(MessageFlags::FIRST_MESSAGE));
        let has_kappa = built.fragments.iter().any(|f| matches!(f, MessageFragment::Emote { id, .. } if id == "25"));
        assert!(has_kappa, "expected Kappa emote fragment: {:?}", built.fragments);
        let ts_first = built
            .fragments
            .iter()
            .find(|f| matches!(f, MessageFragment::Timestamp { .. }));
        assert!(ts_first.is_some(), "missing Timestamp fragment");
    }

    #[test]
    fn detects_action_message_via_msg_is_action() {
        let msg = make_privmsg("", "\u{0001}ACTION hugs you\u{0001}");
        let built = build_chat_message_from_privmsg(&msg);
        assert!(built.flags.contains(MessageFlags::ACTION));
    }

    #[test]
    fn parses_bits_into_bits_total_and_flag() {
        let msg = make_privmsg("bits=500", "Cheer500 hi");
        let built = build_chat_message_from_privmsg(&msg);
        assert_eq!(built.bits_total, Some(500));
        assert!(built.flags.contains(MessageFlags::CHEER_MESSAGE));
    }

    #[test]
    fn hype_chat_tier_round_trips_and_sets_elevated_flag() {
        let msg = make_privmsg(
            "pinned-chat-paid-amount=500;pinned-chat-paid-currency=USD;pinned-chat-paid-level=THIRTY_MINUTES",
            "pinned",
        );
        let built = build_chat_message_from_privmsg(&msg);
        assert!(built.flags.contains(MessageFlags::ELEVATED_MESSAGE));
        assert_eq!(built.elevated_tier, Some(ElevatedTier::ThirtyMin));
    }

    #[test]
    fn reply_tags_populate_reply_context_and_preview_fragment() {
        let msg = make_privmsg(
            "reply-parent-msg-id=PARENT;reply-parent-user-login=streamer;\
             reply-parent-display-name=Streamer;reply-parent-msg-body=hello\\schat",
            "@streamer hi back",
        );
        let built = build_chat_message_from_privmsg(&msg);
        assert!(built.flags.contains(MessageFlags::REPLY_MESSAGE));
        let r = built.reply.as_ref().expect("reply context");
        assert_eq!(r.parent_message_id, "PARENT");
        let has_preview = built
            .fragments
            .iter()
            .any(|f| matches!(f, MessageFragment::ReplyPreview { parent_message_id, .. } if parent_message_id == "PARENT"));
        assert!(has_preview, "expected ReplyPreview fragment");
    }

    #[test]
    fn mentions_become_mention_fragments_only_at_word_boundaries() {
        let msg = make_privmsg("", "hey @bob check foo@bar later");
        let built = build_chat_message_from_privmsg(&msg);
        let mention_count = built
            .fragments
            .iter()
            .filter(|f| matches!(f, MessageFragment::Mention { login, .. } if login == "bob"))
            .count();
        assert_eq!(mention_count, 1, "expected exactly one Mention fragment: {:?}", built.fragments);
        let text_has_foo_at_bar = built.fragments.iter().any(|f| {
            matches!(f, MessageFragment::Text { content, .. } if content.contains("foo@bar"))
        });
        assert!(text_has_foo_at_bar, "foo@bar should remain in Text fragment");
    }

    #[test]
    fn https_urls_become_link_fragments() {
        let msg = make_privmsg("", "check https://example.com/path?q=1 nice");
        let built = build_chat_message_from_privmsg(&msg);
        let has_link = built.fragments.iter().any(|f| {
            matches!(f, MessageFragment::Link { url, .. } if url == "https://example.com/path?q=1")
        });
        assert!(has_link, "expected Link fragment: {:?}", built.fragments);
    }

    #[test]
    fn legacy_fields_preserved_for_log_roundtrip() {
        let msg = make_privmsg("color=#AABBCC", "hello world");
        let built = build_chat_message_from_privmsg(&msg);
        assert_eq!(built.username, "Alice");
        assert_eq!(built.message, "hello world");
        assert_eq!(built.color.as_deref(), Some("#AABBCC"));
        assert!(built.author.is_some());
        assert!(!built.fragments.is_empty());
    }
}
