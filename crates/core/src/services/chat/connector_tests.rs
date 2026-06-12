//! Network-free connector surface tests.
//!
//! Every connector's `connect()` validates credentials and required
//! fields synchronously BEFORE it touches the network. These tests pin
//! that pre-network surface — fresh-state getters, wrong-credential
//! rejection, empty-field rejection, read-only send behaviour, and the
//! not-connected guards on `disconnect`/`send` — without standing up a
//! mock server. The post-validation network bodies are exercised
//! separately.

use tokio::sync::mpsc;

use super::platform::{ChatPlatform, PlatformError};
use super::{
    FacebookConnector, KickConnector, TikTokConnector, TrovoConnector, TwitchConnector,
    YouTubeConnector,
};
use crate::models::{ChatConnectionStatus, ChatCredentials, ChatMessage};

fn assert_fresh(connector: &dyn ChatPlatform, name: &str) {
    assert_eq!(connector.status(), ChatConnectionStatus::Disconnected);
    assert!(!connector.is_connected());
    assert_eq!(connector.message_count(), 0);
    assert_eq!(connector.platform_name(), name);
    assert!(connector.last_error().is_none());
    assert!(!connector.can_send());
}

async fn connect_err(connector: &mut dyn ChatPlatform, cred: ChatCredentials) -> PlatformError {
    let (tx, _rx) = mpsc::channel::<ChatMessage>(8);
    connector
        .connect(cred, tx)
        .await
        .expect_err("connect should fail before any network call")
}

/// A credential of a platform the connector under test does not accept,
/// used to drive the "Expected X credentials" rejection branch.
fn foreign_cred() -> ChatCredentials {
    ChatCredentials::Trovo {
        channel_id: "someone-else".into(),
        oauth_token: None,
    }
}

#[test]
fn fresh_connectors_report_disconnected_zeroed_state() {
    assert_fresh(&TwitchConnector::new(), "twitch");
    assert_fresh(&YouTubeConnector::new(), "youtube");
    assert_fresh(&TrovoConnector::new(), "trovo");
    assert_fresh(&KickConnector::new(), "kick");
    assert_fresh(&FacebookConnector::new(), "facebook");
    assert_fresh(&TikTokConnector::new(), "tiktok");
}

#[test]
fn default_matches_new_for_every_connector() {
    assert_eq!(
        TwitchConnector::default().platform_name(),
        TwitchConnector::new().platform_name()
    );
    assert_eq!(
        YouTubeConnector::default().status(),
        ChatConnectionStatus::Disconnected
    );
    assert_eq!(
        TrovoConnector::default().status(),
        ChatConnectionStatus::Disconnected
    );
    assert_eq!(
        KickConnector::default().status(),
        ChatConnectionStatus::Disconnected
    );
    assert_eq!(
        FacebookConnector::default().status(),
        ChatConnectionStatus::Disconnected
    );
    assert_eq!(
        TikTokConnector::default().status(),
        ChatConnectionStatus::Disconnected
    );
}

#[tokio::test]
async fn wrong_credentials_are_rejected_before_network() {
    let cases: Vec<(Box<dyn ChatPlatform>, &str)> = vec![
        (
            Box::new(TwitchConnector::new()),
            "Expected Twitch credentials",
        ),
        (
            Box::new(YouTubeConnector::new()),
            "Expected YouTube credentials",
        ),
        (Box::new(KickConnector::new()), "Expected Kick credentials"),
        (
            Box::new(FacebookConnector::new()),
            "Expected Facebook credentials",
        ),
        (
            Box::new(TikTokConnector::new()),
            "Expected TikTok credentials",
        ),
    ];

    for (mut connector, expected) in cases {
        let err = connect_err(connector.as_mut(), foreign_cred()).await;
        match err {
            PlatformError::InvalidConfig(msg) => assert_eq!(msg, expected),
            other => panic!("{expected}: expected InvalidConfig, got {other:?}"),
        }
    }
}

#[tokio::test]
async fn trovo_rejects_foreign_credentials() {
    // Trovo's own credential is the foreign cred for the others, so it
    // gets a dedicated case driven by a Twitch credential instead.
    let err = connect_err(
        &mut TrovoConnector::new(),
        ChatCredentials::Twitch {
            channel: "x".into(),
            auth: None,
        },
    )
    .await;
    match err {
        PlatformError::InvalidConfig(msg) => assert_eq!(msg, "Expected Trovo credentials"),
        other => panic!("expected InvalidConfig, got {other:?}"),
    }
}

#[tokio::test]
async fn kick_requires_a_channel_name() {
    let err = connect_err(
        &mut KickConnector::new(),
        ChatCredentials::Kick {
            channel: "   ".into(),
            oauth_token: None,
            broadcaster_user_id: None,
        },
    )
    .await;
    match err {
        PlatformError::InvalidConfig(msg) => assert_eq!(msg, "Kick channel name is required"),
        other => panic!("expected InvalidConfig, got {other:?}"),
    }
}

#[tokio::test]
async fn tiktok_requires_a_username() {
    let err = connect_err(
        &mut TikTokConnector::new(),
        ChatCredentials::TikTok {
            username: "  @  ".into(),
            session_token: None,
        },
    )
    .await;
    match err {
        PlatformError::InvalidConfig(msg) => assert_eq!(msg, "TikTok username is required"),
        other => panic!("expected InvalidConfig, got {other:?}"),
    }
}

#[tokio::test]
async fn facebook_requires_video_id_then_access_token() {
    let missing_video = connect_err(
        &mut FacebookConnector::new(),
        ChatCredentials::Facebook {
            video_id: "  ".into(),
            access_token: "tok".into(),
        },
    )
    .await;
    match missing_video {
        PlatformError::InvalidConfig(msg) => {
            assert_eq!(msg, "Facebook live video id is required")
        }
        other => panic!("expected InvalidConfig, got {other:?}"),
    }

    let missing_token = connect_err(
        &mut FacebookConnector::new(),
        ChatCredentials::Facebook {
            video_id: "123".into(),
            access_token: "   ".into(),
        },
    )
    .await;
    match missing_token {
        PlatformError::Authentication(msg) => {
            assert_eq!(msg, "Facebook Page Access Token is required")
        }
        other => panic!("expected Authentication, got {other:?}"),
    }
}

#[tokio::test]
async fn failed_connect_records_last_error() {
    let mut kick = KickConnector::new();
    let _ = connect_err(
        &mut kick,
        ChatCredentials::Kick {
            channel: "".into(),
            oauth_token: None,
            broadcaster_user_id: None,
        },
    )
    .await;
    assert_eq!(
        kick.last_error().as_deref(),
        Some("Kick channel name is required")
    );

    let mut tiktok = TikTokConnector::new();
    let _ = connect_err(&mut tiktok, foreign_cred()).await;
    assert_eq!(
        tiktok.last_error().as_deref(),
        Some("Expected TikTok credentials")
    );

    let mut facebook = FacebookConnector::new();
    let _ = connect_err(&mut facebook, foreign_cred()).await;
    assert_eq!(
        facebook.last_error().as_deref(),
        Some("Expected Facebook credentials")
    );
}

#[tokio::test]
async fn disconnect_while_idle_is_not_connected() {
    let mut connectors: Vec<Box<dyn ChatPlatform>> = vec![
        Box::new(TwitchConnector::new()),
        Box::new(YouTubeConnector::new()),
        Box::new(TrovoConnector::new()),
        Box::new(KickConnector::new()),
        Box::new(FacebookConnector::new()),
        Box::new(TikTokConnector::new()),
    ];
    for connector in connectors.iter_mut() {
        let err = connector
            .disconnect()
            .await
            .expect_err("idle disconnect should error");
        assert!(matches!(err, PlatformError::NotConnected));
    }
}

#[tokio::test]
async fn send_while_idle_is_rejected() {
    // An unauthenticated Twitch connector fails the auth gate before it
    // ever looks at the connection state.
    let mut twitch = TwitchConnector::new();
    assert!(matches!(
        twitch.send_message("hi".into()).await,
        Err(PlatformError::Authentication(_))
    ));

    // Kick gates send on an active connection — idle is NotConnected.
    let mut kick = KickConnector::new();
    assert!(matches!(
        kick.send_message("hi".into()).await,
        Err(PlatformError::NotConnected)
    ));

    // TikTok is read-only and rejects regardless of connection state.
    let mut tiktok = TikTokConnector::new();
    match tiktok.send_message("hi".into()).await {
        Err(PlatformError::Platform(msg)) => assert!(msg.contains("read-only")),
        other => panic!("expected read-only Platform error, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Frame-parse unit tests
//
// Each connector's spawned websocket/poll loop is network-bound and can't run
// under tarpaulin, but the JSON-frame → `ChatMessage` mapping is pure. These
// tests pin that mapping directly against the `pub(super)` parse functions the
// loops delegate to.
// ---------------------------------------------------------------------------

use serde_json::json;

use super::facebook::parse_facebook_comment;
use super::kick::parse_kick_chat_event;
use super::tiktok::parse_tiktok_chat;
use super::trovo::parse_trovo_chats;
use super::youtube::parse_youtube_chat_item;

#[test]
fn trovo_non_chat_frame_yields_nothing() {
    let frame = json!({ "type": "PING", "data": { "chats": [] } });
    assert!(parse_trovo_chats(&frame).is_empty());

    let no_chats = json!({ "type": "CHAT", "data": {} });
    assert!(parse_trovo_chats(&no_chats).is_empty());
}

#[test]
fn trovo_chat_frame_maps_fields_and_skips_blank_content() {
    let frame = json!({
        "type": "CHAT",
        "data": { "chats": [
            {
                "content": "hello world",
                "nick_name": "Streamer",
                "message_id": "m-1",
                "send_time": 1_700_000_000_i64,
                "roles": ["mod", "sub"],
            },
            { "content": "   ", "nick_name": "Blank" },
        ] }
    });

    let msgs = parse_trovo_chats(&frame);
    assert_eq!(msgs.len(), 1, "blank-content entry is skipped");
    let m = &msgs[0];
    assert_eq!(m.message, "hello world");
    assert_eq!(m.username, "Streamer");
    assert_eq!(m.id, "trovo:m-1");
    assert_eq!(m.source_id.as_deref(), Some("m-1"));
    assert_eq!(
        m.badges.as_deref(),
        Some(&["mod".to_string(), "sub".to_string()][..])
    );
    // Second-precision send_time is normalised to milliseconds.
    assert_eq!(m.timestamp, 1_700_000_000_000);
}

#[test]
fn trovo_username_falls_back_through_user_name_then_unknown() {
    let only_user_name = json!({
        "type": "CHAT",
        "data": { "chats": [ { "content": "hi", "user_name": "fallback" } ] }
    });
    assert_eq!(parse_trovo_chats(&only_user_name)[0].username, "fallback");

    let neither = json!({
        "type": "CHAT",
        "data": { "chats": [ { "content": "hi" } ] }
    });
    assert_eq!(parse_trovo_chats(&neither)[0].username, "Unknown");
}

#[test]
fn trovo_millisecond_send_time_is_left_unscaled() {
    let frame = json!({
        "type": "CHAT",
        "data": { "chats": [ { "content": "hi", "send_time": 1_700_000_000_000_i64 } ] }
    });
    assert_eq!(parse_trovo_chats(&frame)[0].timestamp, 1_700_000_000_000);
}

#[test]
fn kick_ignores_non_chat_events_and_unparseable_bodies() {
    let other_event = json!({ "event": "App\\Events\\FollowerEvent", "data": "{}" });
    assert!(parse_kick_chat_event(&other_event).is_none());

    let empty_data = json!({ "event": "App\\Events\\ChatMessageEvent", "data": "" });
    assert!(parse_kick_chat_event(&empty_data).is_none());

    let bad_inner = json!({ "event": "App\\Events\\ChatMessageEvent", "data": "not json" });
    assert!(parse_kick_chat_event(&bad_inner).is_none());
}

#[test]
fn kick_chat_event_maps_fields() {
    let data = json!({
        "id": "k-9",
        "content": "kick message",
        "created_at": "2024-01-02T03:04:05+00:00",
        "sender": {
            "username": "kicker",
            "identity": {
                "color": "#ff8800",
                "badges": [ { "type": "moderator" }, { "type": "subscriber" } ],
            }
        }
    })
    .to_string();
    let frame = json!({ "event": "App\\Events\\ChatMessageEvent", "data": data });

    let m = parse_kick_chat_event(&frame).expect("valid chat event parses");
    assert_eq!(m.message, "kick message");
    assert_eq!(m.username, "kicker");
    assert_eq!(m.id, "kick:k-9");
    assert_eq!(m.color.as_deref(), Some("#ff8800"));
    assert_eq!(
        m.badges.as_deref(),
        Some(&["moderator".to_string(), "subscriber".to_string()][..])
    );
    // 2024-01-02T03:04:05Z == 1704164645 s == 1704164645000 ms.
    assert_eq!(m.timestamp, 1_704_164_645_000);
}

#[test]
fn kick_empty_content_is_dropped() {
    // Kick checks for an empty string only (it does not trim), so a truly
    // empty `content` is dropped.
    let data = json!({ "content": "", "sender": { "username": "x" } }).to_string();
    let frame = json!({ "event": "App\\Events\\ChatMessageEvent", "data": data });
    assert!(parse_kick_chat_event(&frame).is_none());
}

#[test]
fn youtube_only_text_message_events_map() {
    let non_text = json!({
        "id": "y-1",
        "snippet": { "type": "superChatEvent" },
        "authorDetails": { "displayName": "x" }
    });
    assert!(parse_youtube_chat_item(&non_text).is_none());

    let empty_text = json!({
        "id": "y-2",
        "snippet": { "type": "textMessageEvent", "textMessageDetails": { "messageText": "" } },
        "authorDetails": { "displayName": "x" }
    });
    assert!(parse_youtube_chat_item(&empty_text).is_none());
}

#[test]
fn youtube_text_message_maps_fields_and_badges() {
    let item = json!({
        "id": "y-3",
        "snippet": {
            "type": "textMessageEvent",
            "textMessageDetails": { "messageText": "hi chat" }
        },
        "authorDetails": {
            "displayName": "Alice",
            "isChatOwner": true,
            "isChatModerator": false,
            "isChatSponsor": true,
            "channelId": "UC123"
        }
    });

    let m = parse_youtube_chat_item(&item).expect("text event parses");
    assert_eq!(m.message, "hi chat");
    assert_eq!(m.username, "Alice");
    assert_eq!(m.id, "youtube:y-3");
    assert_eq!(
        m.badges.as_deref(),
        Some(&["owner".to_string(), "member".to_string()][..])
    );
}

#[test]
fn facebook_blank_message_is_dropped() {
    let comment = json!({ "id": "c1", "from": { "name": "x" }, "message": "   " });
    assert!(parse_facebook_comment(&comment).is_none());
}

#[test]
fn facebook_comment_maps_fields_and_cursor() {
    let comment = json!({
        "id": "c-7",
        "from": { "name": "Real Name" },
        "message": "  trimmed  ",
        "created_time": "2024-01-02T03:04:05Z"
    });

    let (m, epoch) = parse_facebook_comment(&comment).expect("valid comment parses");
    assert_eq!(m.message, "trimmed");
    assert_eq!(m.username, "Real Name");
    assert_eq!(m.id, "facebook:c-7");
    assert_eq!(epoch, Some(1_704_164_645));
    assert_eq!(m.timestamp, 1_704_164_645_000);
}

#[test]
fn facebook_comment_parses_graph_api_no_colon_offset() {
    // The real Graph API renders `created_time` with a colon-less offset
    // (`+0000`), which `parse_from_rfc3339` rejects. Regression for the
    // duplicate-flooding bug: a failed parse left the `since` cursor at 0,
    // so every poll re-fetched the whole comment history.
    let comment = json!({
        "id": "c-9",
        "from": { "name": "Real Name" },
        "message": "hi",
        "created_time": "2017-12-17T16:01:42+0000"
    });
    let (m, epoch) = parse_facebook_comment(&comment).expect("no-colon offset parses");
    assert_eq!(epoch, Some(1_513_526_502));
    assert_eq!(m.timestamp, 1_513_526_502_000);
}

#[test]
fn facebook_username_defaults_to_anonymous_and_cursor_optional() {
    let comment = json!({ "id": "c-8", "message": "no author, no time" });
    let (m, epoch) = parse_facebook_comment(&comment).expect("parses");
    assert_eq!(m.username, "Anonymous");
    assert_eq!(epoch, None);
}

#[test]
fn tiktok_blank_comment_is_dropped() {
    use piratetok_live_rs::structs::proto::WebcastChatMessage;
    let msg = WebcastChatMessage {
        comment: "   ".into(),
        ..Default::default()
    };
    assert!(parse_tiktok_chat(&msg).is_none());
}

#[test]
fn tiktok_chat_maps_nickname_and_msg_id() {
    use piratetok_live_rs::structs::proto::{CommonMessageData, UserIdentity, WebcastChatMessage};
    let msg = WebcastChatMessage {
        comment: " hello tiktok ".into(),
        user: Some(UserIdentity {
            nickname: "Dancer".into(),
            ..Default::default()
        }),
        common: Some(CommonMessageData {
            msg_id: 42,
            ..Default::default()
        }),
        ..Default::default()
    };

    let m = parse_tiktok_chat(&msg).expect("valid chat parses");
    assert_eq!(m.message, "hello tiktok");
    assert_eq!(m.username, "Dancer");
    assert_eq!(m.id, "tiktok:42");
}

#[test]
fn tiktok_missing_user_and_zero_msg_id_use_fallbacks() {
    use piratetok_live_rs::structs::proto::WebcastChatMessage;
    let msg = WebcastChatMessage {
        comment: "anon".into(),
        ..Default::default()
    };
    let m = parse_tiktok_chat(&msg).expect("parses");
    assert_eq!(m.username, "Unknown");
    // msg_id == 0 → no source id, so the id keeps the generated UUID (not
    // the `tiktok:` source-id form).
    assert!(!m.id.starts_with("tiktok:"));
    assert!(m.source_id.is_none());
}
