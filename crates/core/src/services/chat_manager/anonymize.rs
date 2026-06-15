//! Anonymous-mode message rewriting.
//!
//! When anonymous mode is on, EVERY identity-bearing field of an inbound
//! message must be pseudonymised before it reaches the event stream or the
//! on-disk log — not just the legacy `username`. The old policy rewrote only
//! `ChatMessage::username`, so any platform that populated `author` (Twitch)
//! leaked the real display name in plaintext while username-only platforms
//! (YouTube) showed a raw `hash:` string.
//!
//! Two pseudonym forms, from the same keyed HMAC (see [`pseudonymizer`]):
//! - **Display names** (what the feed renders) → a friendly `Adjective +
//!   Animal + NN` label, so the feed is readable, not a wall of hashes.
//! - **Correlation ids** (`login` / `user_id`) → the canonical `hash:` form,
//!   so moderation (timeout dimming) and re-identification still work and
//!   keep full entropy.
//!
//! The friendly label keys off a STABLE per-viewer identifier (`user_id` >
//! `login` > `username`) so it doesn't change when a viewer edits their
//! display name mid-session.

use crate::errors::CoreError;
use crate::models::{ChatEvent, ChatMessage, MessageFragment};
use crate::services::pseudonymizer::{friendly_label, pseudonymize};

/// Pseudonymise every identity field of `message` under `salt`. Fails loud
/// (caller drops the message) if the salt can't pseudonymise — never passes
/// a real identity through.
pub(super) fn anonymize_message(
    mut message: ChatMessage,
    salt: &str,
) -> Result<ChatMessage, CoreError> {
    // The author's friendly label keys off the most stable id available.
    let author_key = match message.author.as_ref() {
        Some(a) => stable_key(&a.user_id, &a.login, &message.username),
        None => message.username.clone(),
    };
    let author_label = friendly_label(&author_key, salt)?;

    message.username = author_label.clone();
    if let Some(author) = message.author.as_mut() {
        author.display_name = author_label;
        hash_if_present(&mut author.login, salt)?;
        hash_if_present(&mut author.user_id, salt)?;
    }

    for fragment in message.fragments.iter_mut() {
        anonymize_fragment(fragment, salt)?;
    }

    if let Some(event) = message.event.as_mut() {
        anonymize_event(event, salt)?;
    }

    Ok(message)
}

/// First non-empty of (user_id, login, username) — the stable key for a
/// viewer's friendly label.
fn stable_key(user_id: &str, login: &str, username: &str) -> String {
    [user_id, login, username]
        .into_iter()
        .find(|s| !s.is_empty())
        .unwrap_or(username)
        .to_string()
}

/// Replace `field` with its canonical hash, leaving an empty field empty
/// (nothing to hide).
fn hash_if_present(field: &mut String, salt: &str) -> Result<(), CoreError> {
    if !field.is_empty() {
        *field = pseudonymize(field, salt)?;
    }
    Ok(())
}

/// Friendly label keyed off `login` when present, else the display name.
fn friendly_for(login: &str, display: &str, salt: &str) -> Result<String, CoreError> {
    let key = if login.is_empty() { display } else { login };
    friendly_label(key, salt)
}

fn anonymize_fragment(fragment: &mut MessageFragment, salt: &str) -> Result<(), CoreError> {
    match fragment {
        MessageFragment::Mention {
            login,
            display_name,
            ..
        } => {
            *display_name = friendly_for(login, display_name, salt)?;
            hash_if_present(login, salt)?;
        }
        MessageFragment::ReplyPreview {
            parent_login,
            parent_display_name,
            ..
        } => {
            *parent_display_name = friendly_for(parent_login, parent_display_name, salt)?;
            hash_if_present(parent_login, salt)?;
        }
        MessageFragment::LayeredEmote { base, overlays } => {
            anonymize_fragment(base, salt)?;
            for overlay in overlays.iter_mut() {
                anonymize_fragment(overlay, salt)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn anonymize_event(event: &mut ChatEvent, salt: &str) -> Result<(), CoreError> {
    match event {
        ChatEvent::NewSponsor(p) => {
            p.sponsor_display_name = friendly_for(&p.sponsor_login, &p.sponsor_display_name, salt)?;
            hash_if_present(&mut p.sponsor_login, salt)?;
        }
        ChatEvent::MemberMilestone(p) => {
            p.display_name = friendly_label(&p.display_name, salt)?;
        }
        ChatEvent::SubGifted(p) => {
            p.gifter_display_name = friendly_for(&p.gifter_login, &p.gifter_display_name, salt)?;
            hash_if_present(&mut p.gifter_login, salt)?;
            for recipient in p.recipient_logins.iter_mut() {
                hash_if_present(recipient, salt)?;
            }
        }
        ChatEvent::Raid(p) => {
            p.raider_display_name = friendly_for(&p.raider_login, &p.raider_display_name, salt)?;
            hash_if_present(&mut p.raider_login, salt)?;
        }
        ChatEvent::UserBanned(p) => {
            // Hash both so the ban correlates with the (also-hashed) author
            // login and the timeout dims that viewer's past messages.
            hash_if_present(&mut p.user_login, salt)?;
            hash_if_present(&mut p.user_id, salt)?;
        }
        // No personal display names: SuperChat / SuperSticker / HypeChat /
        // Cheer / Poll / RoomState / MessageDeleted / ChatEnded / Tombstone.
        _ => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{ChatAuthor, ChatPlatform, GiftedPayload, RaidPayload};
    use crate::services::pseudonymizer::{generate_salt, looks_pseudonymized};

    fn author(user_id: &str, login: &str, display: &str) -> ChatAuthor {
        ChatAuthor {
            user_id: user_id.into(),
            login: login.into(),
            display_name: display.into(),
            color: None,
            badges_raw: vec![],
        }
    }

    #[test]
    fn display_name_becomes_friendly_login_becomes_hash() {
        let salt = generate_salt();
        let mut msg = ChatMessage::new(ChatPlatform::Twitch, "CoolViewer".into(), "hi".into());
        msg.author = Some(author("12345", "coolviewer", "CoolViewer"));

        let out = anonymize_message(msg, &salt).unwrap();
        let a = out.author.unwrap();

        // The feed shows a friendly label on BOTH username and author —
        // the plaintext "CoolViewer" must not survive anywhere.
        assert!(!out.username.contains("CoolViewer"));
        assert!(!out.username.starts_with("hash:"));
        assert_eq!(out.username, a.display_name, "username mirrors display name");
        // Correlation ids keep the canonical hash form.
        assert!(looks_pseudonymized(&a.login));
        assert!(looks_pseudonymized(&a.user_id));
    }

    #[test]
    fn same_viewer_gets_one_stable_label_across_messages() {
        let salt = generate_salt();
        let make = || {
            let mut m = ChatMessage::new(ChatPlatform::Twitch, "CoolViewer".into(), "x".into());
            m.author = Some(author("12345", "coolviewer", "CoolViewer"));
            anonymize_message(m, &salt).unwrap().username
        };
        assert_eq!(make(), make(), "stable per viewer within a session");
    }

    #[test]
    fn event_display_names_are_pseudonymised() {
        let salt = generate_salt();
        let mut raid = ChatMessage::new(ChatPlatform::Twitch, "RaiderName".into(), "".into());
        raid.event = Some(ChatEvent::Raid(RaidPayload {
            raider_login: "raidername".into(),
            raider_display_name: "RaiderName".into(),
            viewer_count: 50,
        }));
        let out = anonymize_message(raid, &salt).unwrap();
        if let Some(ChatEvent::Raid(p)) = out.event {
            assert!(!p.raider_display_name.contains("RaiderName"));
            assert!(looks_pseudonymized(&p.raider_login));
        } else {
            panic!("expected a raid event");
        }

        let mut gift = ChatMessage::new(ChatPlatform::Twitch, "Gifter".into(), "".into());
        gift.event = Some(ChatEvent::SubGifted(GiftedPayload {
            gifter_login: "gifter".into(),
            gifter_display_name: "Gifter".into(),
            count: 5,
            recipient_logins: vec!["alice".into(), "bob".into()],
            tier: "1000".into(),
        }));
        let out = anonymize_message(gift, &salt).unwrap();
        if let Some(ChatEvent::SubGifted(p)) = out.event {
            assert!(!p.gifter_display_name.contains("Gifter"));
            assert!(p.recipient_logins.iter().all(|r| looks_pseudonymized(r)));
        } else {
            panic!("expected a sub-gift event");
        }
    }

    #[test]
    fn bad_salt_fails_loud() {
        let msg = ChatMessage::new(ChatPlatform::YouTube, "viewer".into(), "hi".into());
        assert!(anonymize_message(msg, "").is_err());
    }
}
