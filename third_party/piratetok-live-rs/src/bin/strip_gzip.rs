//! One-shot tool: convert gzipped captures to raw (uncompressed) captures.
//!
//! Reads a .bin capture, decompresses any gzipped PushFrame payloads,
//! re-encodes the frames, and writes a _raw.bin alongside the original.
//! The manifest is identical for both — the decoded output is the same.
//!
//! Usage:
//!   cargo run --bin strip_gzip -- <capture.bin> [<capture2.bin> ...]

use std::io::Read;

use flate2::read::GzDecoder;
use prost::Message;

use piratetok_live_rs::structs::proto::frames::WebcastPushFrame;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("usage: strip_gzip <capture.bin> [<capture2.bin> ...]");
        std::process::exit(1);
    }

    for path in &args {
        process_capture(path);
    }
}

fn process_capture(path: &str) {
    let data = std::fs::read(path).unwrap_or_else(|e| {
        eprintln!("cannot read {path}: {e}");
        std::process::exit(1);
    });

    let mut frames_in = Vec::new();
    let mut pos = 0;
    while pos + 4 <= data.len() {
        let len = u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]) as usize;
        pos += 4;
        if pos + len > data.len() {
            break;
        }
        frames_in.push(data[pos..pos + len].to_vec());
        pos += len;
    }

    let mut out = Vec::new();
    let mut stripped = 0u64;

    for raw in &frames_in {
        let mut frame = match WebcastPushFrame::decode(raw.as_slice()) {
            Ok(f) => f,
            Err(_) => {
                // pass through un-decodable frames as-is
                let len = raw.len() as u32;
                out.extend_from_slice(&len.to_le_bytes());
                out.extend_from_slice(raw);
                continue;
            }
        };

        if frame.payload.len() >= 2 && frame.payload[0] == 0x1f && frame.payload[1] == 0x8b {
            let mut decoder = GzDecoder::new(frame.payload.as_slice());
            let mut decompressed = Vec::new();
            if decoder.read_to_end(&mut decompressed).is_ok() {
                frame.payload = decompressed;
                stripped += 1;
            }
        }

        let encoded = frame.encode_to_vec();
        let len = encoded.len() as u32;
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(&encoded);
    }

    let out_path = path.replace(".bin", "_raw.bin");
    std::fs::write(&out_path, &out).unwrap_or_else(|e| {
        eprintln!("cannot write {out_path}: {e}");
        std::process::exit(1);
    });

    eprintln!("{path} -> {out_path}: {}/{} frames had gzip stripped", stripped, frames_in.len());
}
