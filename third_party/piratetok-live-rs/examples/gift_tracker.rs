//! Gift tracker — shows all gifts with proper streak/combo handling.
//!
//! TikTok has two types of gifts:
//!   1. Single-fire gifts — one event, done. Always count these.
//!   2. Combo gifts — fire MULTIPLE events during a streak.
//!      Only the final event (streak over) carries the real total.
//!      Intermediate events are just UI updates.
//!
//! This example handles both correctly and tracks total diamonds.
//!
//! Usage:
//!   cargo run --example gift_tracker -- <tiktok_username>
//!
//! Example:
//!   cargo run --example gift_tracker -- tiktok

use piratetok_live_rs::structs::TikTokLiveEvent;
use piratetok_live_rs::TikTokLive;

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        println!("Usage: {} <tiktok_username>", args[0]);
        println!();
        println!("Example:");
        println!("  {} tiktok", args[0]);
        return;
    }

    let username = &args[1];

    let mut stream = match TikTokLive::builder(username).connect().await {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error: {e}");
            return;
        }
    };

    let mut total_diamonds: i64 = 0;

    while let Some(event) = stream.next_event().await {
        match event {
            TikTokLiveEvent::Connected { room_id } => {
                println!("Connected to @{username} (room {room_id})! Tracking gifts...\n");
            }
            TikTokLiveEvent::Gift(gift) => {
                let nickname = match &gift.user {
                    Some(user) => user.nickname.clone(),
                    None => String::from("unknown"),
                };

                let gift_name = match &gift.gift_details {
                    Some(details) => details.gift_name.clone(),
                    None => String::from("unknown gift"),
                };

                let diamonds = gift.diamond_total();

                if gift.is_combo_gift() {
                    if gift.is_streak_over() {
                        total_diamonds += diamonds;
                        println!("[GIFT] {nickname} sent {gift_name} x{} — {diamonds} diamonds (streak ended)", gift.repeat_count);
                        println!("       Running total: {total_diamonds} diamonds\n");
                    }
                } else {
                    total_diamonds += diamonds;
                    println!("[GIFT] {nickname} sent {gift_name} — {diamonds} diamonds");
                    println!("       Running total: {total_diamonds} diamonds\n");
                }
            }

            TikTokLiveEvent::Disconnected => {
                println!("Stream ended.");
                break;
            }

            _ => {}
        }
    }

    println!("\nFinal total: {total_diamonds} diamonds");
}
