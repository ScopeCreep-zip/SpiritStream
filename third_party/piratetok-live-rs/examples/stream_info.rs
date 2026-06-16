//! Stream info — fetches room info and stream URLs, then exits.
//!
//! Useful for checking if someone is live and grabbing their stream URLs
//! (e.g. to pipe into ffmpeg/mpv for recording or watching).
//!
//! Usage:
//!   cargo run --example stream_info -- <tiktok_username> [cookies]
//!
//! Example:
//!   cargo run --example stream_info -- tiktok
//!
//! For 18+ rooms (needs session cookies):
//!   cargo run --example stream_info -- username "sessionid=abc; sid_tt=abc"
//!
//! Pipe to mpv:
//!   cargo run --example stream_info -- tiktok 2>/dev/null | grep "^SD:" | cut -d' ' -f2 | xargs mpv

use piratetok_live_rs::http::api::{fetch_room_id, fetch_room_info, FetchParams};

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        println!("Usage: {} <tiktok_username> [cookies]", args[0]);
        return;
    }

    let username = &args[1];
    let cookies = args.get(2).map(|s| s.as_str());
    let timeout = std::time::Duration::from_secs(10);

    let room_id_resp = match fetch_room_id(username, FetchParams { timeout, ..Default::default() }).await {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Error: {e}");
            return;
        }
    };

    let room_id = &room_id_resp.room_id;

    match fetch_room_info(room_id, FetchParams { timeout, cookies, ..Default::default() }).await {
        Ok(room_info) => {
            println!("=== Room Info ===");
            println!("Username: @{username}");
            println!("Room ID:  {room_id}");
            println!("Title:    {}", room_info.title);
            println!("Viewers:  {}", room_info.viewers);
            println!("Likes:    {}", room_info.likes);
            println!("Total:    {} unique viewers", room_info.total_viewers);

            match &room_info.stream_url {
                Some(urls) => {
                    println!();
                    println!("=== Stream URLs (FLV) ===");

                    if let Some(url) = &urls.flv_origin {
                        println!("Origin: {url}");
                    }
                    if let Some(url) = &urls.flv_hd {
                        println!("HD:     {url}");
                    }
                    if let Some(url) = &urls.flv_sd {
                        println!("SD:     {url}");
                    }
                    if let Some(url) = &urls.flv_ld {
                        println!("LD:     {url}");
                    }
                    if let Some(url) = &urls.flv_ao {
                        println!("Audio:  {url}");
                    }
                }
                None => {
                    println!();
                    println!("No stream URLs available.");
                }
            }
        }
        Err(e) => {
            eprintln!("Room info failed: {e}");
            if cookies.is_none() {
                eprintln!("Hint: if this is an 18+ room, pass session cookies as the second argument");
            }
        }
    }
}
