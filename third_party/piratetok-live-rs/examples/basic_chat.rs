//! Minimal chat reader — connects to a TikTok Live stream and prints chat messages.
//!
//! This is the simplest possible example. Copy-paste this to get started.
//!
//! Usage:
//!   cargo run --example basic_chat -- <tiktok_username>
//!
//! Example:
//!   cargo run --example basic_chat -- tiktok

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

    println!("Connecting to @{username}...");

    let mut stream = match TikTokLive::builder(username).connect().await {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error: {e}");
            return;
        }
    };

    println!("Connected! Waiting for chat messages...\n");

    while let Some(event) = stream.next_event().await {
        match event {
            TikTokLiveEvent::Connected { room_id } => {
                println!("Connected to room {room_id}! Waiting for chat messages...\n");
            }
            TikTokLiveEvent::Chat(msg) => {
                let nickname = match &msg.user {
                    Some(user) => user.nickname.as_str(),
                    None => "?",
                };
                println!("{nickname}: {}", msg.comment);
            }
            TikTokLiveEvent::Disconnected => {
                println!("Stream ended.");
                break;
            }
            _ => {}
        }
    }
}
