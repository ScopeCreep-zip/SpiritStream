//! Like event debugger — tracks per-user like streaks to determine if
//! `like_count` is a delta or a running total (like gift repeat_count).
//!
//! If user X sends likes and we get events with count 5, 10, 15 — that's
//! a running total (gift-style). If we get 5, 5, 5 — that's true deltas.
//!
//! Usage:
//!   cargo run --example like_debug -- <tiktok_username>

use piratetok_live_rs::structs::TikTokLiveEvent;
use piratetok_live_rs::TikTokLive;
use std::collections::HashMap;
use std::time::Instant;

struct UserStreak {
    events: Vec<(f32, i32)>, // (timestamp, count)
    last_seen: f32,
}

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
        Err(e) => {
            eprintln!("Error: {e}");
            return;
        }
    };

    let mut users: HashMap<String, UserStreak> = HashMap::new();
    let mut event_count: u64 = 0;
    let start = Instant::now();

    println!("=== LIKE STREAK DEBUG — @{username} ===");
    println!("Tracking per-user like events to detect streak patterns.\n");
    println!("{:<6} {:<8} {:<20} {:<8} {:<8} {:<10} {}",
        "#", "dt(s)", "user", "count", "prev", "gap(s)", "pattern");
    println!("{}", "-".repeat(90));

    while let Some(event) = stream.next_event().await {
        match event {
            TikTokLiveEvent::Connected { room_id } => {
                println!("--- connected room={room_id} ---");
            }
            TikTokLiveEvent::Like(msg) => {
                event_count += 1;
                let elapsed = start.elapsed().as_secs_f32();
                let count = msg.like_count;

                let uid = match &msg.user {
                    Some(u) => {
                        if u.nickname.is_empty() {
                            format!("uid:{}", u.user_id)
                        } else {
                            u.nickname.clone()
                        }
                    }
                    None => "?".into(),
                };

                let entry = users.entry(uid.clone()).or_insert(UserStreak {
                    events: Vec::new(),
                    last_seen: 0.0,
                });

                let gap = elapsed - entry.last_seen;
                let prev_count = entry.events.last().map(|e| e.1).unwrap_or(0);
                let event_num = entry.events.len() + 1;

                // detect pattern
                let pattern = if entry.events.is_empty() {
                    "first".to_string()
                } else if count == prev_count {
                    "SAME (suspicious)".to_string()
                } else if count > prev_count && count == prev_count * 2 {
                    format!("DOUBLING {}->{}  !!!", prev_count, count)
                } else if count > prev_count {
                    format!("INCREASING {}->{}  (running total?)", prev_count, count)
                } else if count < prev_count {
                    format!("DECREASED {}->{}  (new streak?)", prev_count, count)
                } else {
                    "?".to_string()
                };

                entry.events.push((elapsed, count));
                entry.last_seen = elapsed;

                // truncate user name for display
                let display_uid: String = uid.chars().take(18).collect();

                println!("{:<6} {:<8.1} {:<20} {:<8} {:<8} {:<10.1} [#{event_num}] {}",
                    event_count, elapsed, display_uid, count, prev_count, gap, pattern);
            }
            TikTokLiveEvent::Disconnected => {
                break;
            }
            _ => {}
        }
    }

    // summary: show users with 3+ events so we can analyze their streak pattern
    println!("\n=== USERS WITH 3+ EVENTS (streak analysis) ===");
    let mut multi: Vec<_> = users.iter()
        .filter(|(_, s)| s.events.len() >= 3)
        .collect();
    multi.sort_by(|a, b| b.1.events.len().cmp(&a.1.events.len()));

    for (uid, streak) in &multi {
        let counts: Vec<i32> = streak.events.iter().map(|e| e.1).collect();
        let sum: i64 = counts.iter().map(|c| *c as i64).sum();
        let display_uid: String = uid.chars().take(25).collect();
        println!("\n  {} ({} events, naive_sum={})", display_uid, counts.len(), sum);
        print!("    counts: ");
        for (i, (t, c)) in streak.events.iter().enumerate() {
            if i > 0 { print!(", "); }
            print!("{c}@{t:.1}s");
        }
        println!();

        // check if counts are monotonically increasing (running total pattern)
        let monotonic = counts.windows(2).all(|w| w[1] >= w[0]);
        // check if all counts are the same (constant delta pattern)
        let constant = counts.iter().all(|c| *c == counts[0]);
        // check if counts are always 15 (max batch)
        let all_max = counts.iter().all(|c| *c == 15);

        if monotonic && !constant {
            println!("    >>> MONOTONIC INCREASING — likely running total (gift-style bug)");
        } else if all_max {
            println!("    >>> ALL MAX BATCH (15) — consistent deltas, user is spam-tapping");
        } else if constant {
            println!("    >>> CONSTANT — consistent deltas");
        } else {
            println!("    >>> MIXED — likely true deltas with variable tap speed");
        }
    }

    println!("\n=== TOTALS ===");
    println!("events: {event_count}");
    println!("unique users: {}", users.len());
    println!("users with 3+ events: {}", multi.len());
}
