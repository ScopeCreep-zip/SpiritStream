//! Twitch IRC system events → [`ChatMessage`] with [`ChatEvent`] payload.
//!
//! Phase B4: USERNOTICE / CLEARCHAT / CLEARMSG carry "rich events"
//! (subs, raids, gifts, bans, deletes). Each is mapped to one of the
//! [`ChatEvent`] variants the model declares, wrapped in a synthetic
//! `ChatMessage` with `event = Some(...)`, `flags |= SYSTEM`, and an
//! otherwise-empty `fragments` list. The frontend matches on
//! `chat_message.event.kind` to render system rows distinctly.
//!
//! # ROOMSTATE
//!
//! Twitch's `ROOMSTATE` reports channel-level state (slow mode, sub-only,
//! emote-only, R9K). The plan's `ChatEvent` enum has no variant for it
//! today, so Phase B4 **does not** emit a synthetic message for
//! ROOMSTATE — adding `RoomStateChanged` to the enum would be scope
//! creep beyond what B4 budgets. The connector still logs the state at
//! `info!` level so operators see it in stderr; consumers that need
//! the state surface should track that as a Phase-C-or-later concern.
//!
//! # USERNOTICE variants not mapped
//!
//! `Ritual`, `BitsBadgeTier`, `GiftPaidUpgrade`, `AnonGiftPaidUpgrade`,
//! and `Unknown` have no current [`ChatEvent`] mapping. They're logged
//! at `info!` and dropped. Future phases can extend the enum and the
//! mapping in one change.

use crate::models::{
    BanPayload, ChatAuthor, ChatEvent, ChatMessage, ChatMessageDirection,
    ChatPlatform as ChatPlatformEnum, FragmentColor, GiftedPayload, MessageFlags, MessageFragment,
    MilestonePayload, RaidPayload, RoomStatePayload,
};
use twitch_irc::message::{
    ClearChatAction, ClearChatMessage, ClearMsgMessage, FollowersOnlyMode, RGBColor,
    RoomStateMessage, UserNoticeEvent, UserNoticeMessage,
};

/// "AnonymousGifter" is Twitch's documented dummy login for anonymous
/// gift-sub events — the lib leaves it in `sender` but flags the
/// anonymity with `is_sender_anonymous`. Use the constant so the
/// renderer can suppress profile-picture lookups for it consistently.
const ANON_GIFTER_LOGIN: &str = "ananonymousgifter";
const ANON_GIFTER_DISPLAY: &str = "AnonymousGifter";

fn rgb_to_hex(c: &RGBColor) -> String {
    format!("#{:02X}{:02X}{:02X}", c.r, c.g, c.b)
}

fn timestamp_fragment(ts: chrono::DateTime<chrono::Utc>) -> MessageFragment {
    use chrono::Local;
    MessageFragment::Timestamp {
        unix_ms: ts.timestamp_millis(),
        formatted: ts.with_timezone(&Local).format("%H:%M").to_string(),
    }
}

/// Build a SYSTEM-flagged `ChatMessage` shell. Callers fill `event`
/// and any extra flags before returning.
fn system_shell(
    channel_id: &str,
    sender_id: &str,
    sender_login: &str,
    sender_name: &str,
    sender_color: Option<&RGBColor>,
    server_timestamp: chrono::DateTime<chrono::Utc>,
    message_id: String,
) -> ChatMessage {
    let unix_ms = server_timestamp.timestamp_millis();
    let color = sender_color.and_then(|c| FragmentColor::from_hex(&rgb_to_hex(c)));
    let legacy_color = color
        .as_ref()
        .map(|c| c.hex.clone())
        .unwrap_or_else(|| super::TWITCH_DEFAULT_USER_COLOR.to_string());

    ChatMessage {
        id: message_id.clone(),
        platform: ChatPlatformEnum::Twitch,
        account_id: Some(sender_id.to_string()),
        channel_id: Some(channel_id.to_string()),
        platforms: None,
        username: sender_name.to_string(),
        message: String::new(),
        timestamp: unix_ms,
        server_received_at_ms: Some(unix_ms),
        author: Some(ChatAuthor {
            user_id: sender_id.to_string(),
            login: sender_login.to_string(),
            display_name: sender_name.to_string(),
            color: color.clone(),
            badges_raw: Vec::new(),
        }),
        fragments: vec![timestamp_fragment(server_timestamp)],
        flags: MessageFlags::SYSTEM,
        raw_text: None,
        bits_total: None,
        highlight_color: None,
        elevated_tier: None,
        reply: None,
        event: None,
        direction: ChatMessageDirection::Inbound,
        source_id: Some(message_id),
        color: Some(legacy_color),
        badges: None,
    }
}

/// `USERNOTICE` → optional synthetic `ChatMessage` with an event
/// payload. Returns `None` for variants that have no [`ChatEvent`]
/// mapping (Ritual, BitsBadgeTier, gift-paid-upgrade, Unknown).
pub fn build_from_user_notice(msg: &UserNoticeMessage) -> Option<ChatMessage> {
    let mut shell = system_shell(
        &msg.channel_id,
        &msg.sender.id,
        &msg.sender.login,
        &msg.sender.name,
        msg.name_color.as_ref(),
        msg.server_timestamp,
        format!("twitch:{}", msg.message_id),
    );

    let (event, extra_flags): (ChatEvent, MessageFlags) = match &msg.event {
        UserNoticeEvent::Raid {
            viewer_count,
            profile_image_url: _,
        } => (
            ChatEvent::Raid(RaidPayload {
                raider_login: msg.sender.login.clone(),
                raider_display_name: msg.sender.name.clone(),
                viewer_count: u32::try_from(*viewer_count).unwrap_or(u32::MAX),
            }),
            MessageFlags::empty(),
        ),

        UserNoticeEvent::SubGift {
            is_sender_anonymous,
            recipient,
            sub_plan,
            num_gifted_months,
            ..
        } => {
            let (gifter_login, gifter_display) = if *is_sender_anonymous {
                (
                    ANON_GIFTER_LOGIN.to_string(),
                    ANON_GIFTER_DISPLAY.to_string(),
                )
            } else {
                (msg.sender.login.clone(), msg.sender.name.clone())
            };
            (
                ChatEvent::SubGifted(GiftedPayload {
                    gifter_login,
                    gifter_display_name: gifter_display,
                    count: u32::try_from(*num_gifted_months).unwrap_or(1).max(1),
                    recipient_logins: vec![recipient.login.clone()],
                    tier: sub_plan.clone(),
                }),
                MessageFlags::SUBSCRIPTION,
            )
        }

        UserNoticeEvent::SubMysteryGift {
            mass_gift_count,
            sub_plan,
            ..
        } => (
            ChatEvent::SubGifted(GiftedPayload {
                gifter_login: msg.sender.login.clone(),
                gifter_display_name: msg.sender.name.clone(),
                count: u32::try_from(*mass_gift_count).unwrap_or(0),
                recipient_logins: Vec::new(),
                tier: sub_plan.clone(),
            }),
            MessageFlags::SUBSCRIPTION,
        ),

        UserNoticeEvent::AnonSubMysteryGift {
            mass_gift_count,
            sub_plan,
        } => (
            ChatEvent::SubGifted(GiftedPayload {
                gifter_login: ANON_GIFTER_LOGIN.to_string(),
                gifter_display_name: ANON_GIFTER_DISPLAY.to_string(),
                count: u32::try_from(*mass_gift_count).unwrap_or(0),
                recipient_logins: Vec::new(),
                tier: sub_plan.clone(),
            }),
            MessageFlags::SUBSCRIPTION,
        ),

        UserNoticeEvent::SubOrResub {
            cumulative_months,
            is_resub: _,
            ..
        } => (
            ChatEvent::MemberMilestone(MilestonePayload {
                months: u32::try_from(*cumulative_months).unwrap_or(1).max(1),
                display_name: msg.sender.name.clone(),
                message: msg
                    .message_text
                    .clone()
                    .unwrap_or_default()
                    .trim()
                    .to_string(),
            }),
            MessageFlags::SUBSCRIPTION,
        ),

        UserNoticeEvent::Ritual { .. }
        | UserNoticeEvent::BitsBadgeTier { .. }
        | UserNoticeEvent::GiftPaidUpgrade { .. }
        | UserNoticeEvent::AnonGiftPaidUpgrade { .. } => return None,

        // The lib's #[doc(hidden)] Unknown variant — match-all so we
        // forward-compat with future USERNOTICE event types instead of
        // refusing to compile when twitch-irc adds one.
        _ => return None,
    };

    shell.event = Some(event);
    shell.flags |= extra_flags;
    Some(shell)
}

/// `CLEARMSG` → `ChatMessage` carrying [`ChatEvent::MessageDeleted`].
/// The deleted message's id is platform-prefixed to match the id
/// shape `fragments::build_chat_message_from_privmsg` emits, so the
/// frontend can find the past row by exact-string id match.
pub fn build_from_clear_msg(msg: &ClearMsgMessage) -> ChatMessage {
    // CLEARMSG isn't tied to a single sender id we can resolve — the
    // lib gives us only `sender_login`. Fill `sender_id` with the
    // login (the renderer never displays the synthetic id; it's
    // route-only metadata). Tombstone-style shell.
    let target_id = format!("twitch:{}", msg.message_id);
    let mut shell = system_shell(
        // CLEARMSG carries no room-id field in twitch-irc 5; the
        // legacy ChatMessage.channel_id stays unset for these events.
        "",
        &msg.sender_login,
        &msg.sender_login,
        &msg.sender_login,
        None,
        msg.server_timestamp,
        format!("twitch:clearmsg:{}", msg.message_id),
    );
    shell.channel_id = None;
    shell.event = Some(ChatEvent::MessageDeleted { id: target_id });
    shell
}

/// `CLEARCHAT` → `Option<ChatMessage>`:
///  - `UserBanned` / `UserTimedOut` → [`ChatEvent::UserBanned`]
///  - `ChatCleared` → `None` (no plan variant; renderer would have to
///    walk the full backlog, which is a Phase-later concern).
pub fn build_from_clear_chat(msg: &ClearChatMessage) -> Option<ChatMessage> {
    let (user_login, user_id, duration_secs) = match &msg.action {
        ClearChatAction::UserBanned {
            user_login,
            user_id,
        } => (user_login.clone(), user_id.clone(), None),
        ClearChatAction::UserTimedOut {
            user_login,
            user_id,
            timeout_length,
        } => (
            user_login.clone(),
            user_id.clone(),
            Some(u32::try_from(timeout_length.as_secs()).unwrap_or(u32::MAX)),
        ),
        ClearChatAction::ChatCleared => return None,
    };

    let mut shell = system_shell(
        &msg.channel_id,
        &user_id,
        &user_login,
        &user_login,
        None,
        msg.server_timestamp,
        format!("twitch:clearchat:{}:{}", msg.channel_id, user_id),
    );
    shell.event = Some(ChatEvent::UserBanned(BanPayload {
        user_id,
        user_login,
        duration_secs,
        reason: None,
    }));
    shell.flags |= MessageFlags::TIMED_OUT_AUTHOR;
    Some(shell)
}

/// `ROOMSTATE` → optional synthetic `ChatMessage` carrying
/// [`ChatEvent::RoomStateChanged`]. Returns `None` only when every
/// field on the upstream message is `None` (the empty-delta case the
/// lib emits on some join sequences — nothing to surface).
///
/// `FollowersOnlyMode::Disabled` is mapped to
/// `followers_only_disabled = Some(true)` so the renderer can show
/// "Followers-only OFF" distinctly from "any-follower OK" — the two
/// `FollowersOnlyMode` variants would otherwise collapse into the
/// same `Option<u32>`.
pub fn build_from_room_state(msg: &RoomStateMessage) -> Option<ChatMessage> {
    let (followers_only_minutes, followers_only_disabled) = match &msg.follwers_only {
        Some(FollowersOnlyMode::Disabled) => (None, Some(true)),
        Some(FollowersOnlyMode::Enabled(d)) => {
            let mins = u32::try_from(d.as_secs() / 60).unwrap_or(u32::MAX);
            (Some(mins), Some(false))
        }
        None => (None, None),
    };

    let slow_mode_secs = msg
        .slow_mode
        .as_ref()
        .map(|d| u32::try_from(d.as_secs()).unwrap_or(u32::MAX));

    let payload = RoomStatePayload {
        emote_only: msg.emote_only,
        subscribers_only: msg.subscribers_only,
        r9k: msg.r9k,
        slow_mode_secs,
        followers_only_minutes,
        followers_only_disabled,
    };

    // Empty-delta guard — every field None means nothing to surface.
    if payload.emote_only.is_none()
        && payload.subscribers_only.is_none()
        && payload.r9k.is_none()
        && payload.slow_mode_secs.is_none()
        && payload.followers_only_minutes.is_none()
        && payload.followers_only_disabled.is_none()
    {
        return None;
    }

    // ROOMSTATE has no sender — use a synthetic "twitch:tmi" anchor.
    // `tmi.twitch.tv` is the documented service login Twitch uses for
    // system-level messages.
    let now = chrono::Utc::now();
    let mut shell = system_shell(
        &msg.channel_id,
        "twitch-tmi",
        "tmi",
        "Twitch",
        None,
        now,
        format!(
            "twitch:roomstate:{}:{}",
            msg.channel_id,
            now.timestamp_millis()
        ),
    );
    shell.event = Some(ChatEvent::RoomStateChanged(payload));
    Some(shell)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use twitch_irc::message::{ClearChatAction, IRCMessage};

    fn parse_user_notice(raw: &str) -> UserNoticeMessage {
        let irc = IRCMessage::parse(raw).expect("valid IRC line");
        UserNoticeMessage::try_from(irc).expect("USERNOTICE parse")
    }

    fn parse_clear_msg(raw: &str) -> ClearMsgMessage {
        let irc = IRCMessage::parse(raw).expect("valid IRC line");
        ClearMsgMessage::try_from(irc).expect("CLEARMSG parse")
    }

    fn parse_clear_chat(raw: &str) -> ClearChatMessage {
        let irc = IRCMessage::parse(raw).expect("valid IRC line");
        ClearChatMessage::try_from(irc).expect("CLEARCHAT parse")
    }

    fn parse_room_state(raw: &str) -> RoomStateMessage {
        let irc = IRCMessage::parse(raw).expect("valid IRC line");
        RoomStateMessage::try_from(irc).expect("ROOMSTATE parse")
    }

    #[test]
    fn subgift_emits_subgifted_event_with_recipient() {
        // Canonical Twitch subgift example with the full required tag
        // set the lib's parser needs.
        let raw = "@badge-info=;badges=;color=;display-name=Gifter;emotes=;flags=;\
id=g1;login=gifter;mod=0;msg-id=subgift;msg-param-cumulative-months=1;\
msg-param-months=1;msg-param-origin-id=abc;msg-param-recipient-display-name=Recipient;\
msg-param-recipient-id=99;msg-param-recipient-user-name=recipient;\
msg-param-sender-count=1;msg-param-sub-plan-name=Channel;msg-param-sub-plan=1000;\
msg-param-gift-months=1;room-id=42;subscriber=0;\
system-msg=Gifter\\sgifted\\sa\\sTier\\s1\\ssub\\sto\\sRecipient!;\
tmi-sent-ts=1700000000000;turbo=0;user-id=1;user-type= :tmi.twitch.tv USERNOTICE #c";
        let msg = parse_user_notice(raw);
        let built = build_from_user_notice(&msg).expect("subgift maps to event");

        assert!(built.flags.contains(MessageFlags::SYSTEM));
        assert!(built.flags.contains(MessageFlags::SUBSCRIPTION));
        match built.event.expect("event populated") {
            ChatEvent::SubGifted(payload) => {
                assert_eq!(payload.gifter_login, "gifter");
                assert_eq!(payload.recipient_logins, vec!["recipient".to_string()]);
                assert_eq!(payload.tier, "1000");
                assert_eq!(payload.count, 1);
            }
            other => panic!("expected SubGifted, got {other:?}"),
        }
    }

    #[test]
    fn raid_emits_raid_event_with_viewer_count() {
        let raw = "@badge-info=;badges=;color=;display-name=Raider;emotes=;flags=;\
id=r1;login=raider;mod=0;msg-id=raid;msg-param-displayName=Raider;\
msg-param-login=raider;msg-param-viewerCount=42;\
msg-param-profileImageURL=https://example/p.png;room-id=42;subscriber=0;\
system-msg=Raid!;tmi-sent-ts=1700000000000;turbo=0;user-id=1;user-type= \
:tmi.twitch.tv USERNOTICE #c";
        let msg = parse_user_notice(raw);
        let built = build_from_user_notice(&msg).expect("raid maps to event");
        match built.event.expect("event populated") {
            ChatEvent::Raid(payload) => {
                assert_eq!(payload.raider_login, "raider");
                assert_eq!(payload.viewer_count, 42);
            }
            other => panic!("expected Raid, got {other:?}"),
        }
    }

    #[test]
    fn anon_subgift_uses_anonymousgifter_login() {
        let raw = "@badge-info=;badges=;color=;display-name=AnAnonymousGifter;emotes=;flags=;\
id=g1;login=ananonymousgifter;mod=0;msg-id=subgift;\
msg-param-cumulative-months=1;msg-param-months=1;msg-param-origin-id=abc;\
msg-param-recipient-display-name=Recipient;msg-param-recipient-id=99;\
msg-param-recipient-user-name=recipient;msg-param-sender-count=0;\
msg-param-sub-plan-name=Channel;msg-param-sub-plan=1000;\
msg-param-gift-months=1;msg-param-anon-gift=true;\
room-id=42;subscriber=0;system-msg=anon;tmi-sent-ts=1700000000000;\
turbo=0;user-id=274598607;user-type= :tmi.twitch.tv USERNOTICE #c";
        let msg = parse_user_notice(raw);
        let built = build_from_user_notice(&msg).expect("anon subgift maps");
        match built.event.expect("event") {
            ChatEvent::SubGifted(p) => {
                assert_eq!(p.gifter_login, ANON_GIFTER_LOGIN);
                assert_eq!(p.gifter_display_name, ANON_GIFTER_DISPLAY);
            }
            other => panic!("expected SubGifted (anon), got {other:?}"),
        }
    }

    #[test]
    fn resub_emits_member_milestone_with_months_count() {
        let raw = "@badge-info=subscriber/6;badges=subscriber/6;color=;display-name=Alice;\
emotes=;flags=;id=z;login=alice;mod=0;msg-id=resub;\
msg-param-cumulative-months=6;msg-param-months=0;msg-param-should-share-streak=0;\
msg-param-sub-plan-name=Channel;msg-param-sub-plan=Prime;\
room-id=42;subscriber=1;system-msg=resub;tmi-sent-ts=1700000000000;\
turbo=0;user-id=1;user-type= :tmi.twitch.tv USERNOTICE #c :six month message";
        let msg = parse_user_notice(raw);
        let built = build_from_user_notice(&msg).expect("resub maps");
        match built.event.expect("event") {
            ChatEvent::MemberMilestone(p) => {
                assert_eq!(p.months, 6);
                assert_eq!(p.display_name, "Alice");
                assert_eq!(p.message, "six month message");
            }
            other => panic!("expected MemberMilestone, got {other:?}"),
        }
        assert!(built.flags.contains(MessageFlags::SUBSCRIPTION));
    }

    #[test]
    fn ritual_event_returns_none_no_plan_variant() {
        let raw = "@badge-info=;badges=;color=;display-name=NewUser;emotes=;flags=;\
id=z;login=newuser;mod=0;msg-id=ritual;msg-param-ritual-name=new_chatter;\
room-id=42;subscriber=0;system-msg=hi;tmi-sent-ts=1700000000000;\
turbo=0;user-id=1;user-type= :tmi.twitch.tv USERNOTICE #c :hello";
        let msg = parse_user_notice(raw);
        assert!(
            build_from_user_notice(&msg).is_none(),
            "ritual has no plan ChatEvent variant — must drop"
        );
    }

    #[test]
    fn clearmsg_emits_message_deleted_event_with_prefixed_id() {
        let raw = "@login=alice;room-id=42;target-msg-id=ABC123;\
tmi-sent-ts=1700000000000 :tmi.twitch.tv CLEARMSG #c :deleted body";
        let msg = parse_clear_msg(raw);
        let built = build_from_clear_msg(&msg);
        match built.event.expect("event") {
            ChatEvent::MessageDeleted { id } => {
                assert_eq!(id, "twitch:ABC123");
            }
            other => panic!("expected MessageDeleted, got {other:?}"),
        }
        assert!(built.flags.contains(MessageFlags::SYSTEM));
    }

    #[test]
    fn clearchat_user_banned_emits_user_banned_event() {
        let raw = "@room-id=42;target-user-id=99;tmi-sent-ts=1700000000000 \
:tmi.twitch.tv CLEARCHAT #c :baduser";
        let msg = parse_clear_chat(raw);
        // Sanity: lib classifies as UserBanned (no ban-duration).
        assert!(matches!(msg.action, ClearChatAction::UserBanned { .. }));
        let built = build_from_clear_chat(&msg).expect("user-ban maps");
        match built.event.expect("event") {
            ChatEvent::UserBanned(p) => {
                assert_eq!(p.user_login, "baduser");
                assert_eq!(p.user_id, "99");
                assert_eq!(p.duration_secs, None);
            }
            other => panic!("expected UserBanned, got {other:?}"),
        }
        assert!(built.flags.contains(MessageFlags::TIMED_OUT_AUTHOR));
    }

    #[test]
    fn clearchat_timeout_emits_user_banned_with_duration() {
        let raw = "@ban-duration=300;room-id=42;target-user-id=99;\
tmi-sent-ts=1700000000000 :tmi.twitch.tv CLEARCHAT #c :baduser";
        let msg = parse_clear_chat(raw);
        assert!(matches!(
            msg.action,
            ClearChatAction::UserTimedOut {
                timeout_length: Duration { .. },
                ..
            }
        ));
        let built = build_from_clear_chat(&msg).expect("timeout maps");
        match built.event.expect("event") {
            ChatEvent::UserBanned(p) => {
                assert_eq!(p.duration_secs, Some(300));
            }
            other => panic!("expected UserBanned (timeout), got {other:?}"),
        }
    }

    #[test]
    fn clearchat_chat_cleared_returns_none() {
        let raw = "@room-id=42;tmi-sent-ts=1700000000000 \
:tmi.twitch.tv CLEARCHAT #c";
        let msg = parse_clear_chat(raw);
        assert!(matches!(msg.action, ClearChatAction::ChatCleared));
        assert!(
            build_from_clear_chat(&msg).is_none(),
            "chat-cleared has no plan ChatEvent variant"
        );
    }

    #[test]
    fn roomstate_slow_mode_emits_room_state_changed_with_delta() {
        let raw = "@room-id=42;slow=30 :tmi.twitch.tv ROOMSTATE #c";
        let msg = parse_room_state(raw);
        let built = build_from_room_state(&msg).expect("slow-mode delta surfaces");
        match built.event.expect("event") {
            ChatEvent::RoomStateChanged(p) => {
                assert_eq!(p.slow_mode_secs, Some(30));
                // Other fields unchanged.
                assert_eq!(p.emote_only, None);
                assert_eq!(p.subscribers_only, None);
                assert_eq!(p.r9k, None);
            }
            other => panic!("expected RoomStateChanged, got {other:?}"),
        }
        assert!(built.flags.contains(MessageFlags::SYSTEM));
    }

    #[test]
    fn roomstate_followers_only_disabled_distinct_from_unchanged() {
        let raw = "@room-id=42;followers-only=-1 :tmi.twitch.tv ROOMSTATE #c";
        let msg = parse_room_state(raw);
        let built = build_from_room_state(&msg).expect("followers-off delta surfaces");
        match built.event.expect("event") {
            ChatEvent::RoomStateChanged(p) => {
                assert_eq!(p.followers_only_disabled, Some(true));
                assert_eq!(p.followers_only_minutes, None);
            }
            other => panic!("expected RoomStateChanged, got {other:?}"),
        }
    }

    #[test]
    fn roomstate_followers_only_enabled_with_minimum_minutes() {
        let raw = "@room-id=42;followers-only=10 :tmi.twitch.tv ROOMSTATE #c";
        let msg = parse_room_state(raw);
        let built = build_from_room_state(&msg).expect("followers-on delta surfaces");
        match built.event.expect("event") {
            ChatEvent::RoomStateChanged(p) => {
                assert_eq!(p.followers_only_disabled, Some(false));
                assert_eq!(p.followers_only_minutes, Some(10));
            }
            other => panic!("expected RoomStateChanged, got {other:?}"),
        }
    }

    #[test]
    fn roomstate_full_initial_state_populates_every_field() {
        // Twitch's join-time ROOMSTATE has every field set. Verifies
        // each maps to a Some.
        let raw = "@emote-only=1;followers-only=5;r9k=0;rituals=0;\
room-id=42;slow=10;subs-only=1 :tmi.twitch.tv ROOMSTATE #c";
        let msg = parse_room_state(raw);
        let built = build_from_room_state(&msg).expect("full-state surfaces");
        match built.event.expect("event") {
            ChatEvent::RoomStateChanged(p) => {
                assert_eq!(p.emote_only, Some(true));
                assert_eq!(p.subscribers_only, Some(true));
                assert_eq!(p.r9k, Some(false));
                assert_eq!(p.slow_mode_secs, Some(10));
                assert_eq!(p.followers_only_minutes, Some(5));
                assert_eq!(p.followers_only_disabled, Some(false));
            }
            other => panic!("expected RoomStateChanged, got {other:?}"),
        }
    }
}
