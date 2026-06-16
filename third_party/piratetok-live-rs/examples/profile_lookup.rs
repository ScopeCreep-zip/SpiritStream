//! Look up TikTok profile data including HD avatars.
//!
//! Usage:
//!   cargo run --example profile_lookup -- <username> [username2] ...
//!
//! Example:
//!   cargo run --example profile_lookup -- tiktok fakeuser999xyznotreal

use piratetok_live_rs::errors::TikTokLiveError;
use piratetok_live_rs::helpers::profile_cache::ProfileCache;

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        println!("Usage: {} <username> [username2] ...", args[0]);
        return;
    }

    let cache = ProfileCache::new();

    for username in &args[1..] {
        println!("Fetching profile for @{username}...");

        match cache.fetch(username).await {
            Ok(profile) => {
                let room = if profile.room_id.is_empty() {
                    "(offline)".to_string()
                } else {
                    profile.room_id.clone()
                };
                let link = profile
                    .bio_link
                    .as_deref()
                    .unwrap_or("(none)");

                println!("  User ID:    {}", profile.user_id);
                println!("  Nickname:   {}", profile.nickname);
                println!("  Verified:   {}", profile.verified);
                println!("  Followers:  {}", profile.follower_count);
                println!("  Videos:     {}", profile.video_count);
                println!("  Avatar (thumb):  {}", profile.avatar_thumb);
                println!("  Avatar (720):    {}", profile.avatar_medium);
                println!("  Avatar (1080):   {}", profile.avatar_large);
                println!("  Bio link:   {link}");
                println!("  Room ID:    {room}");
            }
            Err(e) => {
                let label = match &e {
                    TikTokLiveError::ProfilePrivate(_) => "PRIVATE",
                    TikTokLiveError::ProfileNotFound(_) => "NOT FOUND",
                    _ => "ERROR",
                };
                println!("  [{label}] {e}");
            }
        }
        println!();
    }

    // Demonstrate cache hit
    let first = &args[1];
    println!("Fetching @{first} again (should be cached)...");
    match cache.fetch(first).await {
        Ok(p) => println!("  [cached] {} — {} followers", p.nickname, p.follower_count),
        Err(e) => println!("  [cached error] {e}"),
    }
}
