//! Replay a recorded capture through the full decode pipeline.
//!
//! Reads a [u32_le len][bytes] framed .bin file, processes every frame
//! through the same PushFrame → decompress → Response → message decode
//! pipeline that the live WSS connection uses.
//!
//! Dumps everything: per-frame breakdown, per-message-type counts, helper
//! state (gift streaks, like accumulator), decode failures, unknown types.
//!
//! Usage:
//!   cargo run --bin replay_capture -- <capture.bin> [--quiet] [--dump-payloads <dir>]
//!
//! --dump-payloads writes each message payload as a separate .bin file:
//!   <dir>/msg_0042_WebcastLikeMessage.bin
//! Verify independently with: protoc --decode_raw < msg_0042_WebcastLikeMessage.bin

use std::collections::{BTreeMap, HashMap};
use std::time::Instant;

use prost::Message;

use piratetok_live_rs::decode::mapper;
use piratetok_live_rs::helpers::gift_streak::GiftStreakTracker;
use piratetok_live_rs::helpers::like_accumulator::LikeAccumulator;
use piratetok_live_rs::structs::proto::frames::WebcastPushFrame;
use piratetok_live_rs::structs::proto::messages::{
    WebcastGiftMessage, WebcastLikeMessage, WebcastResponse,
};
use piratetok_live_rs::structs::TikTokLiveEvent;
use piratetok_live_rs::websocket::frames::decompress_if_gzipped;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("usage: replay_capture <capture.bin> [--quiet]");
        std::process::exit(1);
    }

    let path = &args[1];
    let quiet = args.iter().any(|a| a == "--quiet");
    let dump_dir = args.windows(2)
        .find(|w| w[0] == "--dump-payloads")
        .map(|w| w[1].clone());

    if let Some(ref dir) = dump_dir {
        std::fs::create_dir_all(dir).unwrap_or_else(|e| {
            eprintln!("cannot create dump dir {dir}: {e}");
            std::process::exit(1);
        });
        eprintln!("[replay] dumping payloads to {dir}/");
    }

    let frames = read_capture(path);
    eprintln!("[replay] loaded {} raw WSS frames from {}", frames.len(), path);

    let start = Instant::now();

    // counters
    let mut payload_type_counts: BTreeMap<String, u64> = BTreeMap::new();
    let mut msg_type_counts: BTreeMap<String, u64> = BTreeMap::new();
    let mut event_type_counts: BTreeMap<String, u64> = BTreeMap::new();
    let mut decode_failures: Vec<(usize, String)> = Vec::new();
    let mut decompress_failures: Vec<(usize, String)> = Vec::new();
    let mut total_messages: u64 = 0;
    let mut total_events: u64 = 0;

    // sub-route tracking
    let mut follow_count: u64 = 0;
    let mut share_count: u64 = 0;
    let mut join_count: u64 = 0;
    let mut live_ended_count: u64 = 0;

    // helpers
    let mut like_acc = LikeAccumulator::new();
    let mut gift_tracker = GiftStreakTracker::new();
    let mut like_events: Vec<LikeRow> = Vec::new();
    let mut gift_events: Vec<GiftRow> = Vec::new();

    // unknown collector
    let mut unknown_types: BTreeMap<String, u64> = BTreeMap::new();

    for (i, raw) in frames.iter().enumerate() {
        let frame = match WebcastPushFrame::decode(raw.as_slice()) {
            Ok(f) => f,
            Err(e) => {
                decode_failures.push((i, format!("PushFrame decode: {e}")));
                if !quiet {
                    eprintln!("[frame {i}] FAIL PushFrame decode: {e}");
                }
                continue;
            }
        };

        *payload_type_counts.entry(frame.payload_type.clone()).or_default() += 1;

        if !quiet {
            eprintln!(
                "[frame {i}] type={:10} payload={} bytes  seq={} log={}",
                frame.payload_type,
                frame.payload.len(),
                frame.seq_id,
                frame.log_id
            );
        }

        if frame.payload_type != "msg" {
            continue;
        }

        let decompressed = match decompress_if_gzipped(&frame.payload) {
            Ok(d) => d,
            Err(e) => {
                decompress_failures.push((i, format!("{e}")));
                if !quiet {
                    eprintln!("  FAIL decompress: {e}");
                }
                continue;
            }
        };

        let response = match WebcastResponse::decode(decompressed.as_slice()) {
            Ok(r) => r,
            Err(e) => {
                decode_failures.push((i, format!("Response decode: {e}")));
                if !quiet {
                    eprintln!("  FAIL Response decode: {e}");
                }
                continue;
            }
        };

        if !quiet {
            eprintln!(
                "  Response: {} messages, needs_ack={}, cursor={}",
                response.messages.len(),
                response.needs_ack,
                if response.cursor.is_empty() { "(empty)" } else { &response.cursor }
            );
        }

        for msg in &response.messages {
            total_messages += 1;
            *msg_type_counts.entry(msg.r#type.clone()).or_default() += 1;

            if let Some(ref dir) = dump_dir {
                let fname = format!("{}/msg_{:04}_{}.bin", dir, total_messages, msg.r#type);
                if let Err(e) = std::fs::write(&fname, &msg.payload) {
                    eprintln!("  WARN: dump write failed {fname}: {e}");
                }
            }

            let events = mapper::decode_message(&msg.r#type, &msg.payload);

            if !quiet {
                eprintln!(
                    "    {} ({} bytes) -> {} event(s)",
                    msg.r#type,
                    msg.payload.len(),
                    events.len()
                );
            }

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

            // run helpers on raw payloads
            if msg.r#type == "WebcastLikeMessage" {
                if let Ok(like_msg) = WebcastLikeMessage::decode(msg.payload.as_slice()) {
                    let stats = like_acc.process(&like_msg);
                    like_events.push(LikeRow {
                        frame_idx: i,
                        wire_count: like_msg.like_count,
                        wire_total: like_msg.total_like_count,
                        acc_total: stats.total_like_count,
                        acc_accumulated: stats.accumulated_count,
                        went_backwards: stats.went_backwards,
                    });
                    if !quiet {
                        eprintln!(
                            "      [like] delta={} wire_total={} acc_total={} acc={} {}",
                            like_msg.like_count,
                            like_msg.total_like_count,
                            stats.total_like_count,
                            stats.accumulated_count,
                            if stats.went_backwards { "BACKWARDS" } else { "" }
                        );
                    }
                }
            }

            if msg.r#type == "WebcastGiftMessage" {
                if let Ok(gift_msg) = WebcastGiftMessage::decode(msg.payload.as_slice()) {
                    let streak = gift_tracker.process(&gift_msg);
                    gift_events.push(GiftRow {
                        frame_idx: i,
                        group_id: gift_msg.group_id,
                        gift_id: gift_msg.gift_id,
                        repeat_count: gift_msg.repeat_count,
                        is_combo: gift_msg.is_combo_gift(),
                        streak_active: streak.is_active,
                        streak_final: streak.is_final,
                        streak_delta: streak.event_gift_count,
                        streak_total: streak.total_gift_count,
                        diamond_delta: streak.event_diamond_count,
                        diamond_total: streak.total_diamond_count,
                    });
                    if !quiet {
                        eprintln!(
                            "      [gift] gid={} group={} repeat={} combo={} delta={} streak_total={} diamonds={}",
                            gift_msg.gift_id,
                            gift_msg.group_id,
                            gift_msg.repeat_count,
                            gift_msg.is_combo_gift(),
                            streak.event_gift_count,
                            streak.total_gift_count,
                            streak.total_diamond_count,
                        );
                    }
                }
            }
        }
    }

    let elapsed = start.elapsed();

    // ===== SUMMARY =====
    println!("\n{}", "=".repeat(80));
    println!("=== REPLAY SUMMARY: {} ===", path);
    println!("{}\n", "=".repeat(80));

    println!("--- BASICS ---");
    println!("raw frames:        {}", frames.len());
    println!("total messages:    {total_messages}");
    println!("total events:      {total_events}");
    println!("decode failures:   {}", decode_failures.len());
    println!("decomp failures:   {}", decompress_failures.len());
    println!("replay time:       {:.3}s\n", elapsed.as_secs_f64());

    println!("--- PAYLOAD TYPES (PushFrame.payload_type) ---");
    for (k, v) in &payload_type_counts {
        println!("  {k:30} {v}");
    }

    println!("\n--- MESSAGE TYPES (WebcastResponse.messages[].type) ---");
    let mut sorted_msgs: Vec<_> = msg_type_counts.iter().collect();
    sorted_msgs.sort_by(|a, b| b.1.cmp(a.1));
    for (k, v) in &sorted_msgs {
        println!("  {k:50} {v}");
    }

    println!("\n--- EVENT TYPES (after decode + sub-routing) ---");
    let mut sorted_events: Vec<_> = event_type_counts.iter().collect();
    sorted_events.sort_by(|a, b| b.1.cmp(a.1));
    for (k, v) in &sorted_events {
        println!("  {k:30} {v}");
    }

    println!("\n--- SUB-ROUTED EVENTS ---");
    println!("  Follow:     {follow_count}");
    println!("  Share:      {share_count}");
    println!("  Join:       {join_count}");
    println!("  LiveEnded:  {live_ended_count}");

    if !unknown_types.is_empty() {
        println!("\n--- UNKNOWN MESSAGE TYPES (decode fell through) ---");
        let mut sorted_unk: Vec<_> = unknown_types.iter().collect();
        sorted_unk.sort_by(|a, b| b.1.cmp(a.1));
        for (k, v) in &sorted_unk {
            println!("  {k:50} {v}");
        }
    }

    // LIKE ACCUMULATOR REPORT
    if !like_events.is_empty() {
        println!("\n--- LIKE ACCUMULATOR ---");
        let backwards_count = like_events.iter().filter(|r| r.went_backwards).count();
        let last = like_events.last().unwrap();
        println!("  like events:       {}", like_events.len());
        println!("  backwards jumps:   {backwards_count} ({:.1}%)",
            backwards_count as f64 / like_events.len() as f64 * 100.0);
        println!("  final wire max:    {}", last.acc_total);
        println!("  final accumulated: {}", last.acc_accumulated);
        println!("  drift:             {}", last.acc_total as i64 - last.acc_accumulated);

        // dump first 20 + any backwards for detailed inspection
        println!("\n  --- LIKE DETAIL (first 20 + all backwards) ---");
        println!("  {:>6} {:>6} {:>10} {:>10} {:>12} {:>12} {}",
            "idx", "frame", "delta", "wire_total", "acc_total", "accumulated", "");
        let mut printed = 0;
        for (li, row) in like_events.iter().enumerate() {
            if li < 20 || row.went_backwards {
                println!("  {:>6} {:>6} {:>10} {:>10} {:>12} {:>12} {}",
                    li, row.frame_idx, row.wire_count, row.wire_total,
                    row.acc_total, row.acc_accumulated,
                    if row.went_backwards { "<<<" } else { "" });
                printed += 1;
            }
        }
        if printed < like_events.len() {
            println!("  ... ({} more rows, {} total)", like_events.len() - printed, like_events.len());
        }

        // monotonicity check
        let monotonic = like_events.windows(2).all(|w| w[1].acc_total >= w[0].acc_total);
        let acc_monotonic = like_events.windows(2).all(|w| w[1].acc_accumulated >= w[0].acc_accumulated);
        println!("\n  acc_total monotonic:       {}", if monotonic { "PASS" } else { "FAIL !!!" });
        println!("  accumulated monotonic:     {}", if acc_monotonic { "PASS" } else { "FAIL !!!" });
    }

    // GIFT STREAK REPORT
    if !gift_events.is_empty() {
        println!("\n--- GIFT STREAK TRACKER ---");
        let combo_count = gift_events.iter().filter(|r| r.is_combo).count();
        let non_combo = gift_events.len() - combo_count;
        let finals = gift_events.iter().filter(|r| r.streak_final).count();
        println!("  gift events:    {}", gift_events.len());
        println!("  combo gifts:    {combo_count}");
        println!("  non-combo:      {non_combo}");
        println!("  streak finals:  {finals}");
        println!("  active streaks: {}", gift_tracker.active_streaks());

        // per-group summary
        let mut groups: HashMap<u64, Vec<&GiftRow>> = HashMap::new();
        for row in &gift_events {
            groups.entry(row.group_id).or_default().push(row);
        }
        let mut groups_sorted: Vec<_> = groups.iter().collect();
        groups_sorted.sort_by(|a, b| b.1.len().cmp(&a.1.len()));

        println!("\n  --- TOP GIFT STREAKS (by event count) ---");
        println!("  {:>16} {:>6} {:>8} {:>10} {:>12} {:>8}",
            "group_id", "events", "gift_id", "max_repeat", "diamonds", "final?");
        for (gid, rows) in groups_sorted.iter().take(30) {
            let max_repeat = rows.iter().map(|r| r.repeat_count).max().unwrap_or(0);
            let total_diamonds = rows.last().map(|r| r.diamond_total).unwrap_or(0);
            let gift_id = rows[0].gift_id;
            let has_final = rows.iter().any(|r| r.streak_final);
            println!("  {:>16} {:>6} {:>8} {:>10} {:>12} {:>8}",
                gid, rows.len(), gift_id, max_repeat, total_diamonds,
                if has_final { "yes" } else { "NO" });
        }

        // delta sanity: no negative deltas
        let neg_deltas: Vec<_> = gift_events.iter()
            .filter(|r| r.streak_delta < 0)
            .collect();
        println!("\n  negative deltas: {}", neg_deltas.len());
        if !neg_deltas.is_empty() {
            println!("  FAIL: negative deltas detected!");
            for r in &neg_deltas {
                println!("    frame={} group={} delta={}", r.frame_idx, r.group_id, r.streak_delta);
            }
        }
    }

    // DECODE FAILURES
    if !decode_failures.is_empty() {
        println!("\n--- DECODE FAILURES ---");
        for (i, reason) in &decode_failures {
            println!("  frame {i}: {reason}");
        }
    }
    if !decompress_failures.is_empty() {
        println!("\n--- DECOMPRESS FAILURES ---");
        for (i, reason) in &decompress_failures {
            println!("  frame {i}: {reason}");
        }
    }

    println!();
}

#[derive(Debug)]
struct LikeRow {
    frame_idx: usize,
    wire_count: i32,
    wire_total: i64,
    acc_total: i64,
    acc_accumulated: i64,
    went_backwards: bool,
}

#[derive(Debug)]
struct GiftRow {
    frame_idx: usize,
    group_id: u64,
    gift_id: i32,
    repeat_count: i32,
    is_combo: bool,
    #[allow(dead_code)]
    streak_active: bool,
    streak_final: bool,
    streak_delta: i32,
    #[allow(dead_code)]
    streak_total: i32,
    #[allow(dead_code)]
    diamond_delta: i64,
    diamond_total: i64,
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
