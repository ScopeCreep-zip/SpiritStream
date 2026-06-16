//! Decode every msg type in a capture using prost and print non-default field
//! counts per type. Lets us eyeball that we're not dropping fields vs. soy libs.
//!
//! Usage: dump_decoded <capture.bin>

use std::collections::BTreeMap;

use prost::Message;

use piratetok_live_rs::structs::proto::frames::WebcastPushFrame;
use piratetok_live_rs::structs::proto::messages::{
    WebcastCaptionMessage, WebcastChatMessage, WebcastControlMessage, WebcastGiftMessage,
    WebcastImDeleteMessage, WebcastLikeMessage, WebcastLiveIntroMessage, WebcastMemberMessage,
    WebcastResponse, WebcastRoomMessage, WebcastRoomUserSeqMessage, WebcastSocialMessage,
};
use piratetok_live_rs::websocket::frames::decompress_if_gzipped;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let path = match args.get(1) {
        Some(p) => p,
        None => {
            eprintln!("usage: dump_decoded <capture.bin>");
            std::process::exit(1);
        }
    };

    let frames = read_capture(path);
    let mut totals: BTreeMap<String, (usize, usize)> = BTreeMap::new();

    for raw in &frames {
        let frame = match WebcastPushFrame::decode(raw.as_slice()) {
            Ok(f) => f,
            Err(_) => continue,
        };
        if frame.payload_type != "msg" {
            continue;
        }
        let decompressed = match decompress_if_gzipped(&frame.payload) {
            Ok(d) => d,
            Err(_) => continue,
        };
        let response = match WebcastResponse::decode(decompressed.as_slice()) {
            Ok(r) => r,
            Err(_) => continue,
        };
        for msg in &response.messages {
            let entry = totals.entry(msg.r#type.clone()).or_insert((0, 0));
            entry.0 += 1;
            let ok = match msg.r#type.as_str() {
                "WebcastChatMessage" => WebcastChatMessage::decode(msg.payload.as_slice()).is_ok(),
                "WebcastGiftMessage" => WebcastGiftMessage::decode(msg.payload.as_slice()).is_ok(),
                "WebcastLikeMessage" => WebcastLikeMessage::decode(msg.payload.as_slice()).is_ok(),
                "WebcastMemberMessage" => {
                    WebcastMemberMessage::decode(msg.payload.as_slice()).is_ok()
                }
                "WebcastSocialMessage" => {
                    WebcastSocialMessage::decode(msg.payload.as_slice()).is_ok()
                }
                "WebcastRoomUserSeqMessage" => {
                    WebcastRoomUserSeqMessage::decode(msg.payload.as_slice()).is_ok()
                }
                "WebcastControlMessage" => {
                    WebcastControlMessage::decode(msg.payload.as_slice()).is_ok()
                }
                "WebcastCaptionMessage" => {
                    WebcastCaptionMessage::decode(msg.payload.as_slice()).is_ok()
                }
                "WebcastLiveIntroMessage" => {
                    WebcastLiveIntroMessage::decode(msg.payload.as_slice()).is_ok()
                }
                "WebcastRoomMessage" => WebcastRoomMessage::decode(msg.payload.as_slice()).is_ok(),
                "WebcastImDeleteMessage" => {
                    WebcastImDeleteMessage::decode(msg.payload.as_slice()).is_ok()
                }
                _ => true, // skip non-core types
            };
            if ok {
                entry.1 += 1;
            }
        }
    }

    println!("type,total,ok");
    for (k, (t, ok)) in &totals {
        println!("{k},{t},{ok}");
    }
}

fn read_capture(path: &str) -> Vec<Vec<u8>> {
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("read {path}: {e}");
            std::process::exit(1);
        }
    };
    let mut out = Vec::new();
    let mut i = 0;
    while i + 4 <= bytes.len() {
        let len = u32::from_le_bytes([bytes[i], bytes[i + 1], bytes[i + 2], bytes[i + 3]]) as usize;
        i += 4;
        if i + len > bytes.len() {
            break;
        }
        out.push(bytes[i..i + len].to_vec());
        i += len;
    }
    out
}
