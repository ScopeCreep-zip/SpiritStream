//! Enriched chat — online check, room info, WSS connect, profile enrichment.
//!
//! Every user who sends an event gets their profile fetched in the background
//! via ProfileCache. Subsequent events show enriched info: [nickname][N followers]
//!
//! Usage:
//!   cargo run --example enriched_chat -- <username> [cookies]

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use piratetok_live_rs::errors::TikTokLiveError;
use piratetok_live_rs::http::api::{fetch_room_id, fetch_room_info, FetchParams};
use piratetok_live_rs::helpers::profile_cache::ProfileCache;
use piratetok_live_rs::http::sigi::SigiProfile;
use piratetok_live_rs::structs::proto::user::UserIdentity;
use piratetok_live_rs::structs::TikTokLiveEvent;
use piratetok_live_rs::TikTokLive;
use tokio::sync::Mutex;

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        println!("Usage: {} <username> [cookies]", args[0]);
        return;
    }

    let username = &args[1];
    let cookies = args.get(2).map(|s| s.as_str());
    let timeout = Duration::from_secs(10);

    // --- 1. Online check ---
    println!("Checking if @{username} is live...");
    let room_id_resp = match fetch_room_id(username, FetchParams { timeout, ..Default::default() }).await {
        Ok(r) => r,
        Err(e) => {
            eprintln!("  {e}");
            return;
        }
    };
    let room_id = &room_id_resp.room_id;
    println!("  LIVE — room {room_id}");

    // --- 2. Room info ---
    println!("\nFetching room info...");
    match fetch_room_info(room_id, FetchParams { timeout, cookies, ..Default::default() }).await {
        Ok(info) => {
            println!("  Title:   {}", info.title);
            println!("  Viewers: {}", info.viewers);
            println!("  Likes:   {}", info.likes);
            if let Some(urls) = &info.stream_url {
                if let Some(url) = &urls.flv_sd {
                    println!("  Stream:  {url}");
                }
            }
        }
        Err(e) => {
            eprintln!("  Room info failed: {e}");
            if cookies.is_none() {
                eprintln!("  (might need cookies for 18+ rooms)");
            }
        }
    }

    // --- 3. Connect WSS ---
    println!("\nConnecting to WSS...");
    let mut stream = match TikTokLive::builder(username).connect().await {
        Ok(s) => s,
        Err(e) => {
            eprintln!("  {e}");
            return;
        }
    };
    println!("Connected! Enriching user profiles in background...\n");

    let cache = match cookies {
        Some(c) => ProfileCache::new().cookies(c),
        None => ProfileCache::new(),
    };
    let pending: Arc<Mutex<HashSet<String>>> = Arc::new(Mutex::new(HashSet::new()));

    while let Some(event) = stream.next_event().await {
        match &event {
            TikTokLiveEvent::Connected { room_id } => {
                println!("[connected] room {room_id}");
            }
            TikTokLiveEvent::Chat(msg) => {
                let tag = user_tag(&cache, msg.user.as_ref());
                println!("{tag}: {}", msg.comment);
                enrich_bg(&cache, &pending, msg.user.as_ref());
            }
            TikTokLiveEvent::Gift(msg) => {
                let tag = user_tag(&cache, msg.user.as_ref());
                let name = msg.gift_details.as_ref().map(|g| g.gift_name.as_str()).unwrap_or("?");
                let diamonds = msg.diamond_total();
                if msg.is_streak_over() {
                    println!("{tag} sent {name} x{} ({diamonds} diamonds)", msg.repeat_count);
                }
                enrich_bg(&cache, &pending, msg.user.as_ref());
            }
            TikTokLiveEvent::Like(msg) => {
                let tag = user_tag(&cache, msg.user.as_ref());
                println!("{tag} liked (total: {})", msg.total_like_count);
                enrich_bg(&cache, &pending, msg.user.as_ref());
            }
            TikTokLiveEvent::Follow(msg) => {
                let tag = user_tag(&cache, msg.user.as_ref());
                println!("{tag} followed!");
                enrich_bg(&cache, &pending, msg.user.as_ref());
            }
            TikTokLiveEvent::Share(msg) => {
                let tag = user_tag(&cache, msg.user.as_ref());
                println!("{tag} shared the stream");
                enrich_bg(&cache, &pending, msg.user.as_ref());
            }
            TikTokLiveEvent::Join(msg) => {
                let tag = user_tag(&cache, msg.user.as_ref());
                println!("{tag} joined");
                enrich_bg(&cache, &pending, msg.user.as_ref());
            }
            TikTokLiveEvent::RoomUserSeq(msg) => {
                println!("[viewers] {}", msg.total_user);
            }
            TikTokLiveEvent::Disconnected => {
                println!("[disconnected]");
                break;
            }
            _ => {}
        }
    }
}

fn user_tag(cache: &ProfileCache, user: Option<&UserIdentity>) -> String {
    let Some(user) = user else {
        return "[?]".into();
    };

    let uid = &user.unique_id;
    if uid.is_empty() {
        return format!("[{}]", user.nickname);
    }

    match cache.cached(uid) {
        Some(p) => format_enriched(&p),
        None => format!("[{}]", user.nickname),
    }
}

fn format_enriched(p: &SigiProfile) -> String {
    let verified = if p.verified { " ✓" } else { "" };
    format!("[{}{}][{} followers]", p.nickname, verified, p.follower_count)
}

fn enrich_bg(cache: &ProfileCache, pending: &Arc<Mutex<HashSet<String>>>, user: Option<&UserIdentity>) {
    let Some(user) = user else { return };
    let uid = user.unique_id.clone();
    if uid.is_empty() {
        return;
    }

    if cache.cached(&uid).is_some() {
        return;
    }

    let cache = cache.clone();
    let pending = pending.clone();

    tokio::spawn(async move {
        {
            let mut set = pending.lock().await;
            if set.contains(&uid) {
                return;
            }
            set.insert(uid.clone());
        }

        match cache.fetch(&uid).await {
            Ok(p) => {
                eprintln!("  [enriched] @{} — {} followers", p.unique_id, p.follower_count);
            }
            Err(TikTokLiveError::ProfilePrivate(_)) => {}
            Err(TikTokLiveError::ProfileNotFound(_)) => {}
            Err(e) => {
                eprintln!("  [enrich failed] @{uid}: {e}");
            }
        }
    });
}
