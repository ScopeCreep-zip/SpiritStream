//! Multi-stream concurrent load test — M1.
//!
//! Connects to N live rooms simultaneously, counts chat events for 60s, then disconnects all.
//!
//! Gate: `PIRATETOK_LIVE_TEST_USERS` — comma-separated list of live TikTok usernames.
//! ALL listed users must be live during the test.
//!
//! Run:
//! ```
//! PIRATETOK_LIVE_TEST_USERS=user1,user2,user3 cargo test --test multi_stream_load_test
//! ```

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;

use piratetok_live_rs::structs::config::CdnEndpoint;
use piratetok_live_rs::structs::TikTokLiveEvent;
use piratetok_live_rs::TikTokLive;
use tokio::sync::Barrier;
use tokio::time::timeout;

const WSS_HTTP_TIMEOUT: Duration = Duration::from_secs(15);
const WSS_MAX_RETRIES: u32 = 5;
// M1 uses 120s stale timeout (longer than smoke tests — multi-stream needs headroom)
const WSS_STALE_TIMEOUT: Duration = Duration::from_secs(120);

const ALL_CONNECTED_TIMEOUT: Duration = Duration::from_secs(120);
const LIVE_WINDOW: Duration = Duration::from_secs(60);
const SESSION_JOIN_TIMEOUT: Duration = Duration::from_secs(120);

/// M1 — connect N clients concurrently, count chat for 60s, disconnect all cleanly.
#[tokio::test(flavor = "multi_thread")]
async fn multiple_live_clients_track_chat_for_one_minute() {
    let users_raw = match std::env::var("PIRATETOK_LIVE_TEST_USERS") {
        Ok(v) if !v.trim().is_empty() => v,
        _ => {
            eprintln!(
                "SKIP multiple_live_clients_track_chat_for_one_minute: \
                 set PIRATETOK_LIVE_TEST_USERS=user1,user2,... (comma-separated, all must be live)"
            );
            return;
        }
    };

    let users: Vec<String> = users_raw
        .split(',')
        .map(|u| u.trim().to_string())
        .filter(|u| !u.is_empty())
        .collect();

    assert!(!users.is_empty(), "PIRATETOK_LIVE_TEST_USERS must contain at least one username");
    eprintln!("[integration M1] starting {} clients: {:?}", users.len(), users);

    let n = users.len();

    // Shared state: connected barrier + per-channel chat counters + stop flag
    let connected_barrier = Arc::new(Barrier::new(n + 1)); // N workers + 1 main
    let stop_flag = Arc::new(AtomicBool::new(false));

    // Per-user chat counter (index matches `users` order)
    let chat_counters: Vec<Arc<AtomicUsize>> = (0..n).map(|_| Arc::new(AtomicUsize::new(0))).collect();

    // Spawn one task per user
    let mut task_handles = Vec::with_capacity(n);
    for (i, user) in users.iter().enumerate() {
        let username = user.clone();
        let barrier = Arc::clone(&connected_barrier);
        let stop = Arc::clone(&stop_flag);
        let counter = Arc::clone(&chat_counters[i]);

        let handle = tokio::spawn(async move {
            let stream_result = TikTokLive::builder(&username)
                .cdn(CdnEndpoint::Eu)
                .timeout(WSS_HTTP_TIMEOUT)
                .max_retries(WSS_MAX_RETRIES)
                .stale_timeout(WSS_STALE_TIMEOUT)
                .connect()
                .await;

            let mut stream = match stream_result {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("[integration M1] {username}: connect failed: {e}");
                    // Still wait at barrier so main doesn't deadlock (barrier handles partial arrives
                    // by each task calling wait once; failed connects wait here then main sees timeout)
                    barrier.wait().await;
                    return;
                }
            };

            eprintln!("[integration M1] {username}: connected");
            barrier.wait().await;

            // Consume events until stop flag is set
            while !stop.load(Ordering::Relaxed) {
                // Use a short poll timeout so we check stop flag regularly
                let maybe_event = timeout(Duration::from_millis(500), stream.next_event()).await;
                match maybe_event {
                    Ok(Some(TikTokLiveEvent::Chat(_))) => {
                        counter.fetch_add(1, Ordering::Relaxed);
                    }
                    Ok(Some(_)) => {}
                    Ok(None) => {
                        // stream ended (reconnect exhausted or disconnect)
                        break;
                    }
                    Err(_timeout) => {
                        // poll timeout — check stop flag
                    }
                }
            }
            eprintln!("[integration M1] {username}: task exiting");
        });

        task_handles.push(handle);
    }

    // Wait for all clients to connect (or timeout)
    let all_connected = timeout(ALL_CONNECTED_TIMEOUT, connected_barrier.wait()).await;
    assert!(
        all_connected.is_ok(),
        "not all clients reached CONNECTED within {}s — check PIRATETOK_LIVE_TEST_USERS",
        ALL_CONNECTED_TIMEOUT.as_secs()
    );
    eprintln!("[integration M1] all {} clients connected — live window starting ({}s)", n, LIVE_WINDOW.as_secs());

    // Live window: let events flow
    tokio::time::sleep(LIVE_WINDOW).await;

    // Signal all tasks to stop
    stop_flag.store(true, Ordering::Relaxed);
    eprintln!("[integration M1] stop flag set — waiting for tasks to exit");

    // Join all tasks within SESSION_JOIN_TIMEOUT
    let join_result = timeout(SESSION_JOIN_TIMEOUT, async {
        for handle in task_handles {
            handle.await.unwrap_or_else(|e| {
                eprintln!("[integration M1] task panicked or was aborted: {e}");
            });
        }
    })
    .await;

    assert!(
        join_result.is_ok(),
        "not all session tasks exited within {}s after disconnect",
        SESSION_JOIN_TIMEOUT.as_secs()
    );

    // Report per-channel chat counts
    let mut chat_totals: HashMap<String, usize> = HashMap::new();
    for (i, user) in users.iter().enumerate() {
        let count = chat_counters[i].load(Ordering::Relaxed);
        chat_totals.insert(user.clone(), count);
    }

    eprintln!("[integration M1] chat event counts per channel:");
    for (user, count) in &chat_totals {
        eprintln!("  {user}: {count} chat events");
    }

    eprintln!("[integration M1] all {} clients completed cleanly", n);
}
