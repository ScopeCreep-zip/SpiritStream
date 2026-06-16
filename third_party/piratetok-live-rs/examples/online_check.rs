//! Check if a TikTok user is currently live.
//!
//! Usage:
//!   cargo run --example online_check -- <username> [username2] ...
//!
//! Example:
//!   cargo run --example online_check -- tiktok fakeuser999xyznotreal

use piratetok_live_rs::http::api::{fetch_room_id, FetchParams};
use piratetok_live_rs::errors::TikTokLiveError;

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        println!("Usage: {} <username> [username2] ...", args[0]);
        return;
    }

    let timeout = std::time::Duration::from_secs(10);

    for username in &args[1..] {
        match fetch_room_id(username, FetchParams { timeout, ..Default::default() }).await {
            Ok(resp) => {
                println!("  LIVE  @{username} — room {}", resp.room_id);
            }
            Err(e) => {
                let label = match &e {
                    TikTokLiveError::UserNotFound { .. } => "404",
                    TikTokLiveError::HostNotOnline { .. } => "OFF",
                    _ => "ERR",
                };
                println!("  {label:5} @{username} — {e}");
            }
        }
    }
}
