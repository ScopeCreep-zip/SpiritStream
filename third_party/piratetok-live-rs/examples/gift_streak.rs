//! Gift streak tracker — shows per-event deltas for combo gifts.
//!
//! Usage:
//!   cargo run --example gift_streak -- <tiktok_username>

use piratetok_live_rs::helpers::gift_streak::GiftStreakTracker;
use piratetok_live_rs::structs::TikTokLiveEvent;
use piratetok_live_rs::TikTokLive;

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        println!("Usage: {} <tiktok_username>", args[0]);
        return;
    }

    let username = &args[1];
    let mut stream = match TikTokLive::builder(username).connect().await {
        Ok(s) => s,
        Err(e) => { eprintln!("Error: {e}"); return; }
    };

    let mut tracker = GiftStreakTracker::new();
    let mut total_diamonds: i64 = 0;

    while let Some(event) = stream.next_event().await {
        match event {
            TikTokLiveEvent::Connected { room_id } => {
                println!("Connected to @{username} (room {room_id})\n");
            }
            TikTokLiveEvent::Gift(ref gift) => {
                let e = tracker.process(gift);

                let nick = gift.user.as_ref()
                    .map(|u| u.nickname.as_str()).unwrap_or("?");
                let name = gift.gift_details.as_ref()
                    .map(|g| g.gift_name.as_str()).unwrap_or("?");

                if e.is_final {
                    total_diamonds += e.total_diamond_count;
                    println!("[FINAL] streak={} {nick} -> {name} x{} — {} diamonds",
                        e.streak_id, e.total_gift_count, e.total_diamond_count);
                    println!("        running total: {total_diamonds} diamonds\n");
                } else if e.event_gift_count > 0 {
                    println!("[ongoing] streak={} {nick} -> {name} +{} (+{} dmnd)",
                        e.streak_id, e.event_gift_count, e.event_diamond_count);
                }
            }
            TikTokLiveEvent::Disconnected => break,
            _ => {}
        }
    }

    println!("\nFinal total: {total_diamonds} diamonds");
    println!("Active streaks at disconnect: {}", tracker.active_streaks());
}
