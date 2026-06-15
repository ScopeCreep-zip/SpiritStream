//! Self-message detection + outbound echo dedup, shared by the chat connectors
//! that don't have it built in (Kick, Trovo, Facebook, TikTok).
//!
//! The problem: when the local user types in a platform's NATIVE chat, that
//! message arrives over the inbound stream and would render as a normal viewer.
//! We want to mark it as the user's own ("you"). But the platform ALSO echoes
//! back messages the app SENDS — and those are already shown locally as an
//! outbound "You" ([`crate::models::ChatMessage::new_outbound`]). So an inbound
//! self-message must be classified:
//!
//! - [`SelfClass::Other`]  — not from us; render normally.
//! - [`SelfClass::Echo`]   — from us AND matches a recent app-sent message;
//!   it's the echo of an outbound we already display — drop it.
//! - [`SelfClass::Native`] — from us, typed in the platform's chat — mark "you".
//!
//! Twitch and YouTube already implement this inline against their own
//! `recent_outbound` rings; this is the same logic factored out for the rest.

use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Echoes of app-sent messages can lag the send by a few seconds (Pusher fan-out
/// / comment-poll latency). Anything from us within this window matching a sent
/// text is treated as that echo; matches the Twitch/YouTube connectors' window.
const DEDUP_WINDOW: Duration = Duration::from_secs(10);

/// Cap on retained outbound texts — a slow chat could otherwise grow this ring
/// unbounded between matches.
const MAX_RECENT: usize = 100;

/// How an inbound message relates to the local user's own account.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SelfClass {
    /// Not the local user — render normally.
    Other,
    /// The local user, echoing an app-sent message already shown — drop it.
    Echo,
    /// The local user, typed natively on the platform — mark as "you".
    Native,
}

/// Per-connector self-identity matcher + recent-outbound ring.
///
/// Construct once per connection with the user's own identity for that platform
/// (`None` ⇒ never self, so every inbound is [`SelfClass::Other`]). Record each
/// app-sent message via [`record_outbound`](Self::record_outbound); classify
/// every inbound message via [`classify`](Self::classify).
pub(crate) struct SelfEcho {
    /// The local user's identity on this platform (login / numeric id). Already
    /// in the form the inbound author field is compared against.
    identity: Option<String>,
    /// Whether the identity match is ASCII-case-insensitive (usernames) vs an
    /// exact match (stable numeric ids).
    case_insensitive: bool,
    recent: Mutex<VecDeque<(String, Instant)>>,
}

impl SelfEcho {
    pub(crate) fn new(identity: Option<String>, case_insensitive: bool) -> Self {
        let identity = identity.and_then(|s| {
            let t = s.trim();
            if t.is_empty() {
                None
            } else {
                Some(t.to_string())
            }
        });
        Self {
            identity,
            case_insensitive,
            recent: Mutex::new(VecDeque::new()),
        }
    }

    /// Record a message the app just sent, so the platform's echo of it can be
    /// recognized and dropped within [`DEDUP_WINDOW`].
    pub(crate) fn record_outbound(&self, text: &str) {
        let mut recent = self.recent.lock().unwrap_or_else(|e| e.into_inner());
        recent.push_back((text.to_string(), Instant::now()));
        while recent.len() > MAX_RECENT {
            recent.pop_front();
        }
    }

    /// Classify an inbound message by its author identity and text.
    pub(crate) fn classify(&self, author_identity: &str, text: &str) -> SelfClass {
        let Some(identity) = &self.identity else {
            return SelfClass::Other;
        };
        let is_self = if self.case_insensitive {
            author_identity.eq_ignore_ascii_case(identity)
        } else {
            author_identity == identity
        };
        if !is_self {
            return SelfClass::Other;
        }

        let mut recent = self.recent.lock().unwrap_or_else(|e| e.into_inner());
        let now = Instant::now();
        while let Some((_, ts)) = recent.front() {
            if now.duration_since(*ts) > DEDUP_WINDOW {
                recent.pop_front();
            } else {
                break;
            }
        }
        if recent.iter().any(|(t, _)| t == text) {
            SelfClass::Echo
        } else {
            SelfClass::Native
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_identity_is_always_other() {
        let se = SelfEcho::new(None, true);
        assert_eq!(se.classify("anyone", "hi"), SelfClass::Other);
    }

    #[test]
    fn blank_identity_is_treated_as_none() {
        let se = SelfEcho::new(Some("   ".into()), true);
        assert_eq!(se.classify("", "hi"), SelfClass::Other);
    }

    #[test]
    fn other_author_is_other() {
        let se = SelfEcho::new(Some("streamer".into()), true);
        assert_eq!(se.classify("viewer", "hello"), SelfClass::Other);
    }

    #[test]
    fn native_self_message_is_marked() {
        let se = SelfEcho::new(Some("Streamer".into()), true);
        // case-insensitive match, nothing recorded ⇒ native (typed on platform)
        assert_eq!(se.classify("streamer", "hello chat"), SelfClass::Native);
    }

    #[test]
    fn case_sensitive_id_requires_exact_match() {
        let se = SelfEcho::new(Some("UC12345".into()), false);
        assert_eq!(se.classify("uc12345", "x"), SelfClass::Other);
        assert_eq!(se.classify("UC12345", "x"), SelfClass::Native);
    }

    #[test]
    fn app_echo_is_dropped() {
        let se = SelfEcho::new(Some("streamer".into()), true);
        se.record_outbound("sent via app");
        assert_eq!(se.classify("streamer", "sent via app"), SelfClass::Echo);
        // A different text from self is still a native message.
        assert_eq!(se.classify("streamer", "typed natively"), SelfClass::Native);
    }
}
