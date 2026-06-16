//! Raw WSS frame recorder — connects to a live room and dumps every binary
//! frame to disk with [u32_le length][raw bytes] framing.
//!
//! Does NOT decode anything. Just raw bytes off the wire.
//!
//! Usage:
//!   cargo run --bin record_capture -- <username> [output.bin]
//!
//! Ctrl+C to stop. Prints stats on exit.

use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use base64::Engine;
use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;

use piratetok_live_rs::http::api::{fetch_room_id, FetchParams};
use piratetok_live_rs::http::ttwid::fetch_ttwid;
use piratetok_live_rs::http::ua::{random_ua, system_timezone};
use piratetok_live_rs::websocket::frames::{build_enter_room, build_heartbeat};

type WsMessage = tokio_tungstenite::tungstenite::Message;

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("usage: record_capture <username> [output.bin]");
        std::process::exit(1);
    }

    let username = &args[1];
    let output_path = if args.len() > 2 {
        PathBuf::from(&args[2])
    } else {
        PathBuf::from(format!("capture_{}.bin", username.trim_start_matches('@')))
    };

    let running = Arc::new(AtomicBool::new(true));
    let frame_count = Arc::new(AtomicU64::new(0));
    let byte_count = Arc::new(AtomicU64::new(0));

    // ctrl+c handler
    let r = running.clone();
    let fc = frame_count.clone();
    let bc = byte_count.clone();
    let out_clone = output_path.clone();
    ctrlc_handler(r, fc, bc, out_clone);

    eprintln!("[record] resolving @{username}...");
    let ua = random_ua().to_string();
    let room_id = match fetch_room_id(username, FetchParams {
        timeout: Duration::from_secs(10),
        user_agent: Some(&ua),
        ..Default::default()
    }).await {
        Ok(r) => r.room_id,
        Err(e) => {
            eprintln!("[record] FATAL: {e}");
            std::process::exit(1);
        }
    };
    eprintln!("[record] room_id={room_id}");

    eprintln!("[record] fetching ttwid...");
    let ttwid = match fetch_ttwid(Duration::from_secs(10), Some(&ua), None).await {
        Ok(t) => t,
        Err(e) => {
            eprintln!("[record] FATAL: ttwid fetch failed: {e}");
            std::process::exit(1);
        }
    };

    let tz = system_timezone();
    let ws_url = build_ws_url(&room_id, &tz);
    let cookie = format!("ttwid={ttwid}");

    eprintln!("[record] connecting WSS...");
    let ws_key: String = {
        let bytes: [u8; 16] = rand::random();
        base64::engine::general_purpose::STANDARD.encode(bytes)
    };

    let host = ws_url.strip_prefix("wss://").unwrap_or(&ws_url)
        .split('/').next().unwrap_or("webcast-ws.tiktok.com");

    let request = http::Request::builder()
        .method("GET")
        .uri(&ws_url)
        .header("Host", host)
        .header("Upgrade", "websocket")
        .header("Connection", "Upgrade")
        .header("Sec-WebSocket-Key", &ws_key)
        .header("Sec-WebSocket-Version", "13")
        .header("User-Agent", &ua)
        .header("Referer", "https://www.tiktok.com/")
        .header("Origin", "https://www.tiktok.com")
        .header("Cookie", &cookie)
        .body(())
        .expect("request build");

    let (ws_stream, _) = match tokio_tungstenite::connect_async(request).await {
        Ok(pair) => pair,
        Err(e) => {
            eprintln!("[record] FATAL: WSS connect failed: {e}");
            std::process::exit(1);
        }
    };
    let (mut write, mut read) = ws_stream.split();

    // send heartbeat + enter room
    let hb = build_heartbeat(&room_id).expect("heartbeat");
    write.send(WsMessage::Binary(hb.into())).await.expect("send hb");
    let enter = build_enter_room(&room_id).expect("enter room");
    write.send(WsMessage::Binary(enter.into())).await.expect("send enter");

    eprintln!("[record] connected! writing to {}", output_path.display());
    eprintln!("[record] Ctrl+C to stop\n");

    let mut file = std::fs::File::create(&output_path).expect("create output file");
    let start = Instant::now();

    // heartbeat task
    let (hb_tx, mut hb_rx) = mpsc::channel::<Vec<u8>>(4);
    let room_id_clone = room_id.clone();
    let r2 = running.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(10));
        interval.tick().await; // skip immediate
        while r2.load(Ordering::Relaxed) {
            interval.tick().await;
            if let Ok(hb) = build_heartbeat(&room_id_clone) {
                if hb_tx.send(hb).await.is_err() { break; }
            }
        }
    });

    while running.load(Ordering::Relaxed) {
        tokio::select! {
            hb = hb_rx.recv() => {
                if let Some(hb) = hb {
                    let _ = write.send(WsMessage::Binary(hb.into())).await;
                }
            }
            msg = read.next() => {
                match msg {
                    Some(Ok(WsMessage::Binary(data))) => {
                        let len = data.len() as u32;
                        file.write_all(&len.to_le_bytes()).expect("write len");
                        file.write_all(&data).expect("write data");

                        let n = frame_count.fetch_add(1, Ordering::Relaxed) + 1;
                        byte_count.fetch_add(data.len() as u64, Ordering::Relaxed);

                        if n % 50 == 0 {
                            let elapsed = start.elapsed().as_secs();
                            let bytes = byte_count.load(Ordering::Relaxed);
                            eprintln!("[record] {n} frames, {bytes} bytes, {elapsed}s");
                        }
                    }
                    Some(Ok(WsMessage::Ping(data))) => {
                        let _ = write.send(WsMessage::Pong(data)).await;
                    }
                    Some(Ok(WsMessage::Close(_))) | None => {
                        eprintln!("[record] server closed connection");
                        break;
                    }
                    Some(Err(e)) => {
                        eprintln!("[record] WSS error: {e}");
                        break;
                    }
                    _ => {}
                }
            }
        }
    }

    file.flush().expect("flush");
    print_stats(&output_path, &frame_count, &byte_count, start);
}

fn ctrlc_handler(
    running: Arc<AtomicBool>,
    frame_count: Arc<AtomicU64>,
    byte_count: Arc<AtomicU64>,
    output_path: PathBuf,
) {
    let start = Instant::now();
    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.ok();
        running.store(false, Ordering::Relaxed);
        eprintln!();
        print_stats(&output_path, &frame_count, &byte_count, start);
        std::process::exit(0);
    });
}

fn print_stats(path: &PathBuf, frames: &AtomicU64, bytes: &AtomicU64, start: Instant) {
    let f = frames.load(Ordering::Relaxed);
    let b = bytes.load(Ordering::Relaxed);
    let elapsed = start.elapsed().as_secs_f32();
    let file_size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    eprintln!("\n=== CAPTURE STATS ===");
    eprintln!("file:       {}", path.display());
    eprintln!("frames:     {f}");
    eprintln!("payload:    {b} bytes");
    eprintln!("file size:  {file_size} bytes (payload + {f}x4 framing)");
    eprintln!("duration:   {elapsed:.1}s");
    if elapsed > 0.0 {
        eprintln!("rate:       {:.1} frames/s, {:.0} bytes/s", f as f32 / elapsed, b as f32 / elapsed);
    }
}

fn build_ws_url(room_id: &str, tz: &str) -> String {
    let last_rtt = format!("{:.3}", 100.0 + rand::random::<f64>() * 100.0);
    let params: &[(&str, &str)] = &[
        ("version_code", "180800"),
        ("device_platform", "web"),
        ("cookie_enabled", "true"),
        ("screen_width", "1920"),
        ("screen_height", "1080"),
        ("browser_language", "en-US"),
        ("browser_platform", "Linux x86_64"),
        ("browser_name", "Mozilla"),
        ("browser_version", "5.0 (X11)"),
        ("browser_online", "true"),
        ("tz_name", tz),
        ("app_name", "tiktok_web"),
        ("sup_ws_ds_opt", "1"),
        ("update_version_code", "2.0.0"),
        ("compress", "gzip"),
        ("webcast_language", "en"),
        ("ws_direct", "1"),
        ("aid", "1988"),
        ("live_id", "12"),
        ("app_language", "en"),
        ("client_enter", "1"),
        ("room_id", room_id),
        ("identity", "audience"),
        ("history_comment_count", "6"),
        ("last_rtt", &last_rtt),
        ("heartbeat_duration", "10000"),
        ("resp_content_type", "protobuf"),
        ("did_rule", "3"),
    ];
    let query: String = params.iter()
        .map(|(k, v)| format!("{}={}", urlencoding::encode(k), urlencoding::encode(v)))
        .collect::<Vec<_>>()
        .join("&");
    format!("wss://webcast-ws.tiktok.com/webcast/im/ws_proxy/ws_reuse_supplement/?{query}")
}
