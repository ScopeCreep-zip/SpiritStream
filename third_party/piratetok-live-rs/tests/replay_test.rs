//! Replay test — reads a capture file, processes it through the full decode
//! pipeline, and asserts every value matches the manifest JSON.
//!
//! Skips if testdata is not available. Set PIRATETOK_TESTDATA env var or
//! place captures in ../live-testdata/ or ./captures/.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use prost::Message;
use serde::Deserialize;

use piratetok_live_rs::decode::mapper;
use piratetok_live_rs::helpers::gift_streak::GiftStreakTracker;
use piratetok_live_rs::helpers::like_accumulator::LikeAccumulator;
use piratetok_live_rs::structs::proto::frames::WebcastPushFrame;
use piratetok_live_rs::structs::proto::messages::{
    WebcastGiftMessage, WebcastLikeMessage, WebcastResponse,
};
use piratetok_live_rs::structs::TikTokLiveEvent;
use piratetok_live_rs::websocket::frames::decompress_if_gzipped;

// --- manifest types (deserialization) ---

#[derive(Deserialize)]
struct Manifest {
    frame_count: u64,
    message_count: u64,
    event_count: u64,
    decode_failures: u64,
    decompress_failures: u64,
    payload_types: BTreeMap<String, u64>,
    message_types: BTreeMap<String, u64>,
    event_types: BTreeMap<String, u64>,
    sub_routed: SubRouted,
    unknown_types: BTreeMap<String, u64>,
    like_accumulator: LikeManifest,
    gift_streaks: GiftManifest,
}

#[derive(Deserialize)]
struct SubRouted {
    follow: u64,
    share: u64,
    join: u64,
    live_ended: u64,
}

#[derive(Deserialize)]
struct LikeManifest {
    event_count: u64,
    backwards_jumps: u64,
    final_max_total: i64,
    final_accumulated: i64,
    acc_total_monotonic: bool,
    accumulated_monotonic: bool,
    events: Vec<LikeEvent>,
}

#[derive(Deserialize)]
struct LikeEvent {
    wire_count: i32,
    wire_total: i64,
    acc_total: i64,
    accumulated: i64,
    went_backwards: bool,
}

#[derive(Deserialize)]
struct GiftManifest {
    event_count: u64,
    combo_count: u64,
    non_combo_count: u64,
    streak_finals: u64,
    negative_deltas: u64,
    groups: BTreeMap<String, Vec<GiftGroupEvent>>,
}

#[derive(Deserialize)]
struct GiftGroupEvent {
    gift_id: i32,
    repeat_count: i32,
    delta: i32,
    is_final: bool,
    diamond_total: i64,
}

// --- test data location ---

fn find_testdata() -> Option<PathBuf> {
    // 1. PIRATETOK_TESTDATA env var
    if let Ok(dir) = std::env::var("PIRATETOK_TESTDATA") {
        let p = PathBuf::from(dir);
        if p.exists() {
            return Some(p);
        }
    }
    // 2. testdata/ in repo root
    let local = PathBuf::from("testdata");
    if local.join("captures").exists() {
        return Some(local);
    }
    None
}

fn capture_path(testdata: &Path, name: &str) -> PathBuf {
    testdata.join("captures").join(format!("{name}.bin"))
}

fn manifest_path(testdata: &Path, name: &str) -> PathBuf {
    testdata.join("manifests").join(format!("{name}.json"))
}

// --- frame reader ---

fn read_capture(path: &Path) -> Vec<Vec<u8>> {
    let data = std::fs::read(path).unwrap_or_else(|e| {
        panic!("cannot read {}: {e}", path.display());
    });
    let mut frames = Vec::new();
    let mut pos = 0;
    while pos + 4 <= data.len() {
        let len = u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]) as usize;
        pos += 4;
        if pos + len > data.len() {
            panic!("truncated frame at offset {}", pos - 4);
        }
        frames.push(data[pos..pos + len].to_vec());
        pos += len;
    }
    frames
}

// --- replay engine ---

struct ReplayResult {
    frame_count: u64,
    message_count: u64,
    event_count: u64,
    decode_failures: u64,
    decompress_failures: u64,
    payload_types: BTreeMap<String, u64>,
    message_types: BTreeMap<String, u64>,
    event_types: BTreeMap<String, u64>,
    follow_count: u64,
    share_count: u64,
    join_count: u64,
    live_ended_count: u64,
    unknown_types: BTreeMap<String, u64>,
    like_events: Vec<(i32, i64, i64, i64, bool)>, // (wire_count, wire_total, acc_total, accumulated, went_backwards)
    gift_groups: BTreeMap<String, Vec<(i32, i32, i32, bool, i64)>>, // (gift_id, repeat_count, delta, is_final, diamond_total)
    combo_count: u64,
    non_combo_count: u64,
    streak_finals: u64,
    negative_deltas: u64,
}

fn replay(frames: &[Vec<u8>]) -> ReplayResult {
    let mut r = ReplayResult {
        frame_count: frames.len() as u64,
        message_count: 0,
        event_count: 0,
        decode_failures: 0,
        decompress_failures: 0,
        payload_types: BTreeMap::new(),
        message_types: BTreeMap::new(),
        event_types: BTreeMap::new(),
        follow_count: 0,
        share_count: 0,
        join_count: 0,
        live_ended_count: 0,
        unknown_types: BTreeMap::new(),
        like_events: Vec::new(),
        gift_groups: BTreeMap::new(),
        combo_count: 0,
        non_combo_count: 0,
        streak_finals: 0,
        negative_deltas: 0,
    };

    let mut like_acc = LikeAccumulator::new();
    let mut gift_tracker = GiftStreakTracker::new();

    for raw in frames {
        let frame = match WebcastPushFrame::decode(raw.as_slice()) {
            Ok(f) => f,
            Err(_) => { r.decode_failures += 1; continue; }
        };

        *r.payload_types.entry(frame.payload_type.clone()).or_default() += 1;

        if frame.payload_type != "msg" { continue; }

        let decompressed = match decompress_if_gzipped(&frame.payload) {
            Ok(d) => d,
            Err(_) => { r.decompress_failures += 1; continue; }
        };

        let response = match WebcastResponse::decode(decompressed.as_slice()) {
            Ok(resp) => resp,
            Err(_) => { r.decode_failures += 1; continue; }
        };

        for msg in &response.messages {
            r.message_count += 1;
            *r.message_types.entry(msg.r#type.clone()).or_default() += 1;

            let events = mapper::decode_message(&msg.r#type, &msg.payload);
            for event in &events {
                r.event_count += 1;
                let etype = event_type_name(event);
                *r.event_types.entry(etype.to_string()).or_default() += 1;

                match event {
                    TikTokLiveEvent::Follow(_) => r.follow_count += 1,
                    TikTokLiveEvent::Share(_) => r.share_count += 1,
                    TikTokLiveEvent::Join(_) => r.join_count += 1,
                    TikTokLiveEvent::LiveEnded(_) => r.live_ended_count += 1,
                    TikTokLiveEvent::Unknown { method, .. } => {
                        *r.unknown_types.entry(method.clone()).or_default() += 1;
                    }
                    _ => {}
                }
            }

            if msg.r#type == "WebcastLikeMessage" {
                if let Ok(like_msg) = WebcastLikeMessage::decode(msg.payload.as_slice()) {
                    let stats = like_acc.process(&like_msg);
                    r.like_events.push((
                        like_msg.like_count,
                        like_msg.total_like_count,
                        stats.total_like_count,
                        stats.accumulated_count,
                        stats.went_backwards,
                    ));
                }
            }

            if msg.r#type == "WebcastGiftMessage" {
                if let Ok(gift_msg) = WebcastGiftMessage::decode(msg.payload.as_slice()) {
                    if gift_msg.is_combo_gift() { r.combo_count += 1; } else { r.non_combo_count += 1; }
                    let streak = gift_tracker.process(&gift_msg);
                    if streak.is_final { r.streak_finals += 1; }
                    if streak.event_gift_count < 0 { r.negative_deltas += 1; }

                    let key = gift_msg.group_id.to_string();
                    r.gift_groups.entry(key).or_default().push((
                        gift_msg.gift_id,
                        gift_msg.repeat_count,
                        streak.event_gift_count,
                        streak.is_final,
                        streak.total_diamond_count,
                    ));
                }
            }
        }
    }

    r
}

// --- assertion helpers ---

fn assert_replay(name: &str, r: &ReplayResult, m: &Manifest) {
    assert_eq!(r.frame_count, m.frame_count, "{name}: frame_count");
    assert_eq!(r.message_count, m.message_count, "{name}: message_count");
    assert_eq!(r.event_count, m.event_count, "{name}: event_count");
    assert_eq!(r.decode_failures, m.decode_failures, "{name}: decode_failures");
    assert_eq!(r.decompress_failures, m.decompress_failures, "{name}: decompress_failures");

    assert_eq!(r.payload_types, m.payload_types, "{name}: payload_types");
    assert_eq!(r.message_types, m.message_types, "{name}: message_types");
    assert_eq!(r.event_types, m.event_types, "{name}: event_types");

    assert_eq!(r.follow_count, m.sub_routed.follow, "{name}: sub_routed.follow");
    assert_eq!(r.share_count, m.sub_routed.share, "{name}: sub_routed.share");
    assert_eq!(r.join_count, m.sub_routed.join, "{name}: sub_routed.join");
    assert_eq!(r.live_ended_count, m.sub_routed.live_ended, "{name}: sub_routed.live_ended");

    assert_eq!(r.unknown_types, m.unknown_types, "{name}: unknown_types");

    // like accumulator
    let ml = &m.like_accumulator;
    assert_eq!(r.like_events.len() as u64, ml.event_count, "{name}: like event_count");

    let backwards = r.like_events.iter().filter(|e| e.4).count() as u64;
    assert_eq!(backwards, ml.backwards_jumps, "{name}: like backwards_jumps");

    if let Some(last) = r.like_events.last() {
        assert_eq!(last.2, ml.final_max_total, "{name}: like final_max_total");
        assert_eq!(last.3, ml.final_accumulated, "{name}: like final_accumulated");
    }

    let acc_mono = r.like_events.windows(2).all(|w| w[1].2 >= w[0].2);
    let accum_mono = r.like_events.windows(2).all(|w| w[1].3 >= w[0].3);
    assert_eq!(acc_mono, ml.acc_total_monotonic, "{name}: like acc_total_monotonic");
    assert_eq!(accum_mono, ml.accumulated_monotonic, "{name}: like accumulated_monotonic");

    // like event-by-event
    assert_eq!(r.like_events.len(), ml.events.len(), "{name}: like events length");
    for (i, (got, expected)) in r.like_events.iter().zip(ml.events.iter()).enumerate() {
        assert_eq!(got.0, expected.wire_count, "{name}: like[{i}].wire_count");
        assert_eq!(got.1, expected.wire_total, "{name}: like[{i}].wire_total");
        assert_eq!(got.2, expected.acc_total, "{name}: like[{i}].acc_total");
        assert_eq!(got.3, expected.accumulated, "{name}: like[{i}].accumulated");
        assert_eq!(got.4, expected.went_backwards, "{name}: like[{i}].went_backwards");
    }

    // gift streaks
    let mg = &m.gift_streaks;
    assert_eq!(r.combo_count + r.non_combo_count, mg.event_count, "{name}: gift event_count");
    assert_eq!(r.combo_count, mg.combo_count, "{name}: gift combo_count");
    assert_eq!(r.non_combo_count, mg.non_combo_count, "{name}: gift non_combo_count");
    assert_eq!(r.streak_finals, mg.streak_finals, "{name}: gift streak_finals");
    assert_eq!(r.negative_deltas, mg.negative_deltas, "{name}: gift negative_deltas");

    // gift group-by-group
    assert_eq!(r.gift_groups.len(), mg.groups.len(), "{name}: gift groups count");
    for (gid, got_evts) in &r.gift_groups {
        let expected_evts = mg.groups.get(gid).unwrap_or_else(|| panic!("{name}: missing gift group {gid}"));
        assert_eq!(got_evts.len(), expected_evts.len(), "{name}: gift group {gid} length");
        for (i, (got, expected)) in got_evts.iter().zip(expected_evts.iter()).enumerate() {
            assert_eq!(got.0, expected.gift_id, "{name}: gift[{gid}][{i}].gift_id");
            assert_eq!(got.1, expected.repeat_count, "{name}: gift[{gid}][{i}].repeat_count");
            assert_eq!(got.2, expected.delta, "{name}: gift[{gid}][{i}].delta");
            assert_eq!(got.3, expected.is_final, "{name}: gift[{gid}][{i}].is_final");
            assert_eq!(got.4, expected.diamond_total, "{name}: gift[{gid}][{i}].diamond_total");
        }
    }
}

fn event_type_name(event: &TikTokLiveEvent) -> &'static str {
    match event {
        TikTokLiveEvent::Connected { .. } => "Connected",
        TikTokLiveEvent::Reconnecting { .. } => "Reconnecting",
        TikTokLiveEvent::Disconnected => "Disconnected",
        TikTokLiveEvent::Chat(_) => "Chat",
        TikTokLiveEvent::Gift(_) => "Gift",
        TikTokLiveEvent::Like(_) => "Like",
        TikTokLiveEvent::Member(_) => "Member",
        TikTokLiveEvent::Social(_) => "Social",
        TikTokLiveEvent::Follow(_) => "Follow",
        TikTokLiveEvent::Share(_) => "Share",
        TikTokLiveEvent::Join(_) => "Join",
        TikTokLiveEvent::RoomUserSeq(_) => "RoomUserSeq",
        TikTokLiveEvent::Control(_) => "Control",
        TikTokLiveEvent::LiveEnded(_) => "LiveEnded",
        TikTokLiveEvent::LiveIntro(_) => "LiveIntro",
        TikTokLiveEvent::RoomMessage(_) => "RoomMessage",
        TikTokLiveEvent::Caption(_) => "Caption",
        TikTokLiveEvent::GoalUpdate(_) => "GoalUpdate",
        TikTokLiveEvent::ImDelete(_) => "ImDelete",
        TikTokLiveEvent::RankUpdate(_) => "RankUpdate",
        TikTokLiveEvent::Poll(_) => "Poll",
        TikTokLiveEvent::Envelope(_) => "Envelope",
        TikTokLiveEvent::RoomPin(_) => "RoomPin",
        TikTokLiveEvent::UnauthorizedMember(_) => "UnauthorizedMember",
        TikTokLiveEvent::LinkMicMethod(_) => "LinkMicMethod",
        TikTokLiveEvent::LinkMicBattle(_) => "LinkMicBattle",
        TikTokLiveEvent::LinkMicArmies(_) => "LinkMicArmies",
        TikTokLiveEvent::LinkMessage(_) => "LinkMessage",
        TikTokLiveEvent::LinkLayer(_) => "LinkLayer",
        TikTokLiveEvent::LinkMicLayoutState(_) => "LinkMicLayoutState",
        TikTokLiveEvent::GiftPanelUpdate(_) => "GiftPanelUpdate",
        TikTokLiveEvent::InRoomBanner(_) => "InRoomBanner",
        TikTokLiveEvent::Guide(_) => "Guide",
        TikTokLiveEvent::EmoteChat(_) => "EmoteChat",
        TikTokLiveEvent::QuestionNew(_) => "QuestionNew",
        TikTokLiveEvent::SubNotify(_) => "SubNotify",
        TikTokLiveEvent::Barrage(_) => "Barrage",
        TikTokLiveEvent::HourlyRank(_) => "HourlyRank",
        TikTokLiveEvent::MsgDetect(_) => "MsgDetect",
        TikTokLiveEvent::LinkMicFanTicket(_) => "LinkMicFanTicket",
        TikTokLiveEvent::RoomVerify(_) => "RoomVerify",
        TikTokLiveEvent::OecLiveShopping(_) => "OecLiveShopping",
        TikTokLiveEvent::GiftBroadcast(_) => "GiftBroadcast",
        TikTokLiveEvent::RankText(_) => "RankText",
        TikTokLiveEvent::GiftDynamicRestriction(_) => "GiftDynamicRestriction",
        TikTokLiveEvent::ViewerPicksUpdate(_) => "ViewerPicksUpdate",
        TikTokLiveEvent::SystemMessage(_) => "SystemMessage",
        TikTokLiveEvent::LiveGameIntro(_) => "LiveGameIntro",
        TikTokLiveEvent::AccessControl(_) => "AccessControl",
        TikTokLiveEvent::AccessRecall(_) => "AccessRecall",
        TikTokLiveEvent::AlertBoxAuditResult(_) => "AlertBoxAuditResult",
        TikTokLiveEvent::BindingGift(_) => "BindingGift",
        TikTokLiveEvent::BoostCard(_) => "BoostCard",
        TikTokLiveEvent::BottomMessage(_) => "BottomMessage",
        TikTokLiveEvent::GameRankNotify(_) => "GameRankNotify",
        TikTokLiveEvent::GiftPrompt(_) => "GiftPrompt",
        TikTokLiveEvent::LinkState(_) => "LinkState",
        TikTokLiveEvent::LinkMicBattlePunishFinish(_) => "LinkMicBattlePunishFinish",
        TikTokLiveEvent::LinkmicBattleTask(_) => "LinkmicBattleTask",
        TikTokLiveEvent::MarqueeAnnouncement(_) => "MarqueeAnnouncement",
        TikTokLiveEvent::Notice(_) => "Notice",
        TikTokLiveEvent::Notify(_) => "Notify",
        TikTokLiveEvent::PartnershipDropsUpdate(_) => "PartnershipDropsUpdate",
        TikTokLiveEvent::PartnershipGameOffline(_) => "PartnershipGameOffline",
        TikTokLiveEvent::PartnershipPunish(_) => "PartnershipPunish",
        TikTokLiveEvent::Perception(_) => "Perception",
        TikTokLiveEvent::Speaker(_) => "Speaker",
        TikTokLiveEvent::SubCapsule(_) => "SubCapsule",
        TikTokLiveEvent::SubPinEvent(_) => "SubPinEvent",
        TikTokLiveEvent::SubscriptionNotify(_) => "SubscriptionNotify",
        TikTokLiveEvent::Toast(_) => "Toast",
        TikTokLiveEvent::Unknown { .. } => "Unknown",
    }
}

// --- test runner ---

fn run_capture_test(name: &str) {
    run_capture_test_variant(name, name);
}

fn run_capture_test_raw(name: &str) {
    let raw_name = format!("{name}_raw");
    run_capture_test_variant(&raw_name, name);
}

fn run_capture_test_variant(capture_name: &str, manifest_name: &str) {
    let testdata = match find_testdata() {
        Some(d) => d,
        None => {
            eprintln!("SKIP {capture_name}: no testdata (set PIRATETOK_TESTDATA or clone live-testdata)");
            return;
        }
    };

    let cap = capture_path(&testdata, capture_name);
    let man = manifest_path(&testdata, manifest_name);

    if !cap.exists() {
        eprintln!("SKIP {capture_name}: capture not found at {}", cap.display());
        return;
    }
    if !man.exists() {
        eprintln!("SKIP {capture_name}: manifest not found at {}", man.display());
        return;
    }

    let manifest_json = std::fs::read_to_string(&man).expect("read manifest");
    let manifest: Manifest = serde_json::from_str(&manifest_json).expect("parse manifest");

    let frames = read_capture(&cap);
    let result = replay(&frames);

    assert_replay(capture_name, &result, &manifest);
}

#[test]
fn replay_calvinterest6() {
    run_capture_test("calvinterest6");
}

#[test]
fn replay_happyhappygaltv() {
    run_capture_test("happyhappygaltv");
}

#[test]
fn replay_fox4newsdallasfortworth() {
    run_capture_test("fox4newsdallasfortworth");
}

// Raw (uncompressed) capture variants — same manifests, gzip stripped from payloads

#[test]
fn replay_calvinterest6_raw() {
    run_capture_test_raw("calvinterest6");
}

#[test]
fn replay_happyhappygaltv_raw() {
    run_capture_test_raw("happyhappygaltv");
}

#[test]
fn replay_fox4newsdallasfortworth_raw() {
    run_capture_test_raw("fox4newsdallasfortworth");
}
