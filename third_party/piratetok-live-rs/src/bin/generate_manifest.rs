//! Manifest generator — reads a capture and outputs a JSON manifest
//! that serves as ground truth for cross-lib replay validation.
//!
//! The manifest contains counts, sequences, and assertions that every
//! lib's replay harness must reproduce exactly from the same capture.
//!
//! Usage:
//!   cargo run --bin generate_manifest -- <capture.bin> [output.json]
//!
//! If output.json is omitted, writes to stdout.

use std::collections::BTreeMap;
use std::io::Write;

use prost::Message;
use serde::Serialize;

use piratetok_live_rs::decode::mapper;
use piratetok_live_rs::helpers::gift_streak::GiftStreakTracker;
use piratetok_live_rs::helpers::like_accumulator::LikeAccumulator;
use piratetok_live_rs::structs::proto::frames::WebcastPushFrame;
use piratetok_live_rs::structs::proto::messages::{
    WebcastGiftMessage, WebcastLikeMessage, WebcastResponse,
};
use piratetok_live_rs::structs::TikTokLiveEvent;
use piratetok_live_rs::websocket::frames::decompress_if_gzipped;

#[derive(Serialize)]
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

#[derive(Serialize)]
struct SubRouted {
    follow: u64,
    share: u64,
    join: u64,
    live_ended: u64,
}

#[derive(Serialize)]
struct LikeManifest {
    event_count: u64,
    backwards_jumps: u64,
    final_max_total: i64,
    final_accumulated: i64,
    acc_total_monotonic: bool,
    accumulated_monotonic: bool,
    events: Vec<LikeEvent>,
}

#[derive(Serialize)]
struct LikeEvent {
    wire_count: i32,
    wire_total: i64,
    acc_total: i64,
    accumulated: i64,
    went_backwards: bool,
}

#[derive(Serialize)]
struct GiftManifest {
    event_count: u64,
    combo_count: u64,
    non_combo_count: u64,
    streak_finals: u64,
    negative_deltas: u64,
    groups: BTreeMap<String, Vec<GiftGroupEvent>>,
}

#[derive(Serialize)]
struct GiftGroupEvent {
    gift_id: i32,
    repeat_count: i32,
    delta: i32,
    is_final: bool,
    diamond_total: i64,
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("usage: generate_manifest <capture.bin> [output.json]");
        std::process::exit(1);
    }

    let path = &args[1];
    let output_path = args.get(2);

    let frames = read_capture(path);
    eprintln!("[manifest] loaded {} raw WSS frames from {}", frames.len(), path);

    let mut payload_type_counts: BTreeMap<String, u64> = BTreeMap::new();
    let mut msg_type_counts: BTreeMap<String, u64> = BTreeMap::new();
    let mut event_type_counts: BTreeMap<String, u64> = BTreeMap::new();
    let mut decode_failures: u64 = 0;
    let mut decompress_failures: u64 = 0;
    let mut total_messages: u64 = 0;
    let mut total_events: u64 = 0;

    let mut follow_count: u64 = 0;
    let mut share_count: u64 = 0;
    let mut join_count: u64 = 0;
    let mut live_ended_count: u64 = 0;

    let mut like_acc = LikeAccumulator::new();
    let mut gift_tracker = GiftStreakTracker::new();
    let mut like_events: Vec<LikeEvent> = Vec::new();
    let mut gift_groups: BTreeMap<String, Vec<GiftGroupEvent>> = BTreeMap::new();
    let mut combo_count: u64 = 0;
    let mut non_combo_count: u64 = 0;
    let mut streak_finals: u64 = 0;
    let mut negative_deltas: u64 = 0;

    let mut unknown_types: BTreeMap<String, u64> = BTreeMap::new();

    for raw in &frames {
        let frame = match WebcastPushFrame::decode(raw.as_slice()) {
            Ok(f) => f,
            Err(_) => {
                decode_failures += 1;
                continue;
            }
        };

        *payload_type_counts.entry(frame.payload_type.clone()).or_default() += 1;

        if frame.payload_type != "msg" {
            continue;
        }

        let decompressed = match decompress_if_gzipped(&frame.payload) {
            Ok(d) => d,
            Err(_) => {
                decompress_failures += 1;
                continue;
            }
        };

        let response = match WebcastResponse::decode(decompressed.as_slice()) {
            Ok(r) => r,
            Err(_) => {
                decode_failures += 1;
                continue;
            }
        };

        for msg in &response.messages {
            total_messages += 1;
            *msg_type_counts.entry(msg.r#type.clone()).or_default() += 1;

            let events = mapper::decode_message(&msg.r#type, &msg.payload);

            for event in &events {
                total_events += 1;
                let etype = event_type_name(event);
                *event_type_counts.entry(etype.to_string()).or_default() += 1;

                match event {
                    TikTokLiveEvent::Follow(_) => follow_count += 1,
                    TikTokLiveEvent::Share(_) => share_count += 1,
                    TikTokLiveEvent::Join(_) => join_count += 1,
                    TikTokLiveEvent::LiveEnded(_) => live_ended_count += 1,
                    TikTokLiveEvent::Unknown { method, .. } => {
                        *unknown_types.entry(method.clone()).or_default() += 1;
                    }
                    _ => {}
                }
            }

            if msg.r#type == "WebcastLikeMessage" {
                if let Ok(like_msg) = WebcastLikeMessage::decode(msg.payload.as_slice()) {
                    let stats = like_acc.process(&like_msg);
                    like_events.push(LikeEvent {
                        wire_count: like_msg.like_count,
                        wire_total: like_msg.total_like_count,
                        acc_total: stats.total_like_count,
                        accumulated: stats.accumulated_count,
                        went_backwards: stats.went_backwards,
                    });
                }
            }

            if msg.r#type == "WebcastGiftMessage" {
                if let Ok(gift_msg) = WebcastGiftMessage::decode(msg.payload.as_slice()) {
                    let is_combo = gift_msg.is_combo_gift();
                    if is_combo { combo_count += 1; } else { non_combo_count += 1; }

                    let streak = gift_tracker.process(&gift_msg);
                    if streak.is_final { streak_finals += 1; }
                    if streak.event_gift_count < 0 { negative_deltas += 1; }

                    let key = gift_msg.group_id.to_string();
                    gift_groups.entry(key).or_default().push(GiftGroupEvent {
                        gift_id: gift_msg.gift_id,
                        repeat_count: gift_msg.repeat_count,
                        delta: streak.event_gift_count,
                        is_final: streak.is_final,
                        diamond_total: streak.total_diamond_count,
                    });
                }
            }
        }
    }

    let backwards_jumps = like_events.iter().filter(|e| e.went_backwards).count() as u64;
    let acc_total_monotonic = like_events.windows(2).all(|w| w[1].acc_total >= w[0].acc_total);
    let accumulated_monotonic = like_events.windows(2).all(|w| w[1].accumulated >= w[0].accumulated);
    let (final_max, final_acc) = like_events.last()
        .map(|e| (e.acc_total, e.accumulated))
        .unwrap_or((0, 0));

    let manifest = Manifest {
        frame_count: frames.len() as u64,
        message_count: total_messages,
        event_count: total_events,
        decode_failures,
        decompress_failures,
        payload_types: payload_type_counts,
        message_types: msg_type_counts,
        event_types: event_type_counts,
        sub_routed: SubRouted {
            follow: follow_count,
            share: share_count,
            join: join_count,
            live_ended: live_ended_count,
        },
        unknown_types,
        like_accumulator: LikeManifest {
            event_count: like_events.len() as u64,
            backwards_jumps,
            final_max_total: final_max,
            final_accumulated: final_acc,
            acc_total_monotonic,
            accumulated_monotonic,
            events: like_events,
        },
        gift_streaks: GiftManifest {
            event_count: combo_count + non_combo_count,
            combo_count,
            non_combo_count,
            streak_finals,
            negative_deltas,
            groups: gift_groups,
        },
    };

    let json = serde_json::to_string_pretty(&manifest).expect("json serialize");

    match output_path {
        Some(p) => {
            let mut f = std::fs::File::create(p).expect("create output file");
            f.write_all(json.as_bytes()).expect("write json");
            f.write_all(b"\n").expect("write newline");
            eprintln!("[manifest] wrote {}", p);
        }
        None => {
            println!("{json}");
        }
    }
}

fn read_capture(path: &str) -> Vec<Vec<u8>> {
    let data = std::fs::read(path).unwrap_or_else(|e| {
        eprintln!("cannot read {path}: {e}");
        std::process::exit(1);
    });

    let mut frames = Vec::new();
    let mut pos = 0;

    while pos + 4 <= data.len() {
        let len = u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]) as usize;
        pos += 4;
        if pos + len > data.len() {
            eprintln!("[warn] truncated frame at offset {}: need {len} bytes, have {}", pos - 4, data.len() - pos);
            break;
        }
        frames.push(data[pos..pos + len].to_vec());
        pos += len;
    }

    frames
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
