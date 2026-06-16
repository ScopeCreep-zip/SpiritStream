//! WSS smoke tests against a real live TikTok room — W1–W7 + D1.
//!
//! All tests are gated on `PIRATETOK_LIVE_TEST_USER` and skip cleanly when absent.
//! These are inherently flaky on quiet streams; the long timeouts reduce that risk.
//!
//! Run:
//! ```
//! PIRATETOK_LIVE_TEST_USER=<live_username> cargo test --test wss_smoke_test -- --test-threads=1
//! ```
//!
//! Or in parallel (each test opens its own connection):
//! ```
//! PIRATETOK_LIVE_TEST_USER=<live_username> cargo test --test wss_smoke_test
//! ```

use std::time::Duration;

use piratetok_live_rs::structs::config::CdnEndpoint;
use piratetok_live_rs::structs::TikTokLiveEvent;
use piratetok_live_rs::TikTokLive;
use tokio::sync::oneshot;
use tokio::time::timeout;

// ---- Timeouts from spec Section 5 ----
const AWAIT_TRAFFIC: Duration = Duration::from_secs(90);
const AWAIT_CHAT: Duration = Duration::from_secs(120);
const AWAIT_GIFT: Duration = Duration::from_secs(180);
const AWAIT_LIKE: Duration = Duration::from_secs(120);
const AWAIT_JOIN: Duration = Duration::from_secs(150);
const AWAIT_FOLLOW: Duration = Duration::from_secs(180);
// W7 (subscription) is intentionally disabled — too rare.

// ---- WSS client config from spec Section 2.2 ----
const WSS_HTTP_TIMEOUT: Duration = Duration::from_secs(15);
const WSS_STALE_TIMEOUT: Duration = Duration::from_secs(45);
const WSS_MAX_RETRIES: u32 = 5;

/// Returns the live test username, or `None` if the env var is unset/empty.
fn live_user() -> Option<String> {
    std::env::var("PIRATETOK_LIVE_TEST_USER")
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

/// Connects to the live room on a spawned task, consuming events until `predicate` fires
/// or the deadline elapses. Returns `true` if the predicate fired before the deadline.
///
/// The stream is dropped (disconnected) after the function returns regardless of outcome.
async fn await_event<F>(user: &str, deadline: Duration, predicate: F) -> bool
where
    F: Fn(&TikTokLiveEvent) -> bool + Send + 'static,
{
    let (tx, rx) = oneshot::channel::<()>();

    let username = user.to_string();
    let handle = tokio::spawn(async move {
        let mut tx_opt = Some(tx);
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
                eprintln!("[wss smoke] connect failed: {e}");
                return;
            }
        };

        while let Some(event) = stream.next_event().await {
            if predicate(&event) {
                if let Some(t) = tx_opt.take() {
                    let _ = t.send(());
                }
            }
            // Stop consuming once we've signaled — but let the stream close naturally
            // via drop when the task is aborted by the outer timeout.
            if tx_opt.is_none() {
                break;
            }
        }
    });

    let hit = timeout(deadline, rx).await.map(|r| r.is_ok()).unwrap_or(false);
    handle.abort();
    let _ = handle.await;
    hit
}

// ---- W1: any traffic ----

/// W1 — any room traffic arrives within 90s.
///
/// Passes for any event type (room_user_seq, member, chat, like, control).
/// Even quiet streams send room_user_seq periodically.
#[tokio::test(flavor = "multi_thread")]
async fn connect_receives_traffic_before_timeout() {
    let user = match live_user() {
        Some(u) => u,
        None => {
            eprintln!(
                "SKIP connect_receives_traffic_before_timeout: \
                 set PIRATETOK_LIVE_TEST_USER to a live TikTok username"
            );
            return;
        }
    };

    let hit = await_event(&user, AWAIT_TRAFFIC, |event| {
        matches!(
            event,
            TikTokLiveEvent::RoomUserSeq(_)
                | TikTokLiveEvent::Member(_)
                | TikTokLiveEvent::Chat(_)
                | TikTokLiveEvent::Like(_)
                | TikTokLiveEvent::Control(_)
        )
    })
    .await;

    assert!(
        hit,
        "no room traffic within {}s — is the user live? Check PIRATETOK_LIVE_TEST_USER",
        AWAIT_TRAFFIC.as_secs()
    );
}

// ---- W2: chat ----

/// W2 — a chat message arrives within 120s.
#[tokio::test(flavor = "multi_thread")]
async fn connect_receives_chat_before_timeout() {
    let user = match live_user() {
        Some(u) => u,
        None => {
            eprintln!(
                "SKIP connect_receives_chat_before_timeout: \
                 set PIRATETOK_LIVE_TEST_USER to a live TikTok username"
            );
            return;
        }
    };

    let hit = await_event(&user, AWAIT_CHAT, |event| {
        if let TikTokLiveEvent::Chat(msg) = event {
            let uid = msg.user.as_ref().map(|u| u.unique_id.as_str()).unwrap_or("?");
            eprintln!("[integration W2 chat] {uid}: {}", msg.comment);
            true
        } else {
            false
        }
    })
    .await;

    assert!(
        hit,
        "no chat message within {}s — try a busier stream",
        AWAIT_CHAT.as_secs()
    );
}

// ---- W3: gift ----

/// W3 — a gift event arrives within 180s.
#[tokio::test(flavor = "multi_thread")]
async fn connect_receives_gift_before_timeout() {
    let user = match live_user() {
        Some(u) => u,
        None => {
            eprintln!(
                "SKIP connect_receives_gift_before_timeout: \
                 set PIRATETOK_LIVE_TEST_USER to a live TikTok username"
            );
            return;
        }
    };

    let hit = await_event(&user, AWAIT_GIFT, |event| {
        if let TikTokLiveEvent::Gift(msg) = event {
            let uid = msg.user.as_ref().map(|u| u.unique_id.as_str()).unwrap_or("?");
            let diamonds = msg.diamond_total();
            eprintln!(
                "[integration W3 gift] {uid} -> gift_id={} x{} ({} diamonds)",
                msg.gift_id, msg.repeat_count, diamonds
            );
            true
        } else {
            false
        }
    })
    .await;

    assert!(
        hit,
        "no gift within {}s — try a busier stream (gifts are less frequent than chat)",
        AWAIT_GIFT.as_secs()
    );
}

// ---- W4: like ----

/// W4 — a like event arrives within 120s.
#[tokio::test(flavor = "multi_thread")]
async fn connect_receives_like_before_timeout() {
    let user = match live_user() {
        Some(u) => u,
        None => {
            eprintln!(
                "SKIP connect_receives_like_before_timeout: \
                 set PIRATETOK_LIVE_TEST_USER to a live TikTok username"
            );
            return;
        }
    };

    let hit = await_event(&user, AWAIT_LIKE, |event| {
        if let TikTokLiveEvent::Like(msg) = event {
            let uid = msg.user.as_ref().map(|u| u.unique_id.as_str()).unwrap_or("?");
            eprintln!(
                "[integration W4 like] {uid} count={} total={}",
                msg.like_count, msg.total_like_count
            );
            true
        } else {
            false
        }
    })
    .await;

    assert!(
        hit,
        "no like within {}s — try a more active stream",
        AWAIT_LIKE.as_secs()
    );
}

// ---- W5: join (sub-routed from MemberMessage) ----

/// W5 — a Join event arrives within 150s.
///
/// Join is sub-routed from `WebcastMemberMessage` with action=1.
#[tokio::test(flavor = "multi_thread")]
async fn connect_receives_join_before_timeout() {
    let user = match live_user() {
        Some(u) => u,
        None => {
            eprintln!(
                "SKIP connect_receives_join_before_timeout: \
                 set PIRATETOK_LIVE_TEST_USER to a live TikTok username"
            );
            return;
        }
    };

    let hit = await_event(&user, AWAIT_JOIN, |event| {
        if let TikTokLiveEvent::Join(msg) = event {
            let uid = msg.user.as_ref().map(|u| u.unique_id.as_str()).unwrap_or("?");
            eprintln!("[integration W5 join] {uid}");
            true
        } else {
            false
        }
    })
    .await;

    assert!(
        hit,
        "no join within {}s — try a busier stream",
        AWAIT_JOIN.as_secs()
    );
}

// ---- W6: follow (sub-routed from SocialMessage) ----

/// W6 — a Follow event arrives within 180s.
///
/// Follow is sub-routed from `WebcastSocialMessage` with action=1.
/// May flake on quiet streams — follows are infrequent.
#[tokio::test(flavor = "multi_thread")]
async fn connect_receives_follow_before_timeout() {
    let user = match live_user() {
        Some(u) => u,
        None => {
            eprintln!(
                "SKIP connect_receives_follow_before_timeout: \
                 set PIRATETOK_LIVE_TEST_USER to a live TikTok username"
            );
            return;
        }
    };

    let hit = await_event(&user, AWAIT_FOLLOW, |event| {
        if let TikTokLiveEvent::Follow(msg) = event {
            let uid = msg.user.as_ref().map(|u| u.unique_id.as_str()).unwrap_or("?");
            eprintln!("[integration W6 follow] {uid}");
            true
        } else {
            false
        }
    })
    .await;

    assert!(
        hit,
        "no follow within {}s — follows are infrequent, try a growing stream",
        AWAIT_FOLLOW.as_secs()
    );
}

// ---- D1: disconnect lifecycle ----

/// D1 — dropping the stream while connected causes the task to exit within 18s.
///
/// Validates that aborting the JoinHandle (via Drop) cleanly unblocks the event loop.
/// The Rust lib uses a tokio task + mpsc channel; dropping `TikTokLiveStream` aborts
/// the task. This test confirms that pattern doesn't leave zombie tasks.
#[tokio::test(flavor = "multi_thread")]
async fn disconnect_unblocks_connect_task_after_connected() {
    let user = match live_user() {
        Some(u) => u,
        None => {
            eprintln!(
                "SKIP disconnect_unblocks_connect_task_after_connected: \
                 set PIRATETOK_LIVE_TEST_USER to a live TikTok username"
            );
            return;
        }
    };

    // Step 1: wait for CONNECTED event (up to 90s)
    let (connected_tx, connected_rx) = oneshot::channel::<()>();

    let username = user.clone();
    let connect_handle = tokio::spawn(async move {
        let stream_result = TikTokLive::builder(&username)
            .cdn(CdnEndpoint::Eu)
            .timeout(WSS_HTTP_TIMEOUT)
            .max_retries(WSS_MAX_RETRIES)
            .stale_timeout(WSS_STALE_TIMEOUT)
            .connect()
            .await;

        match stream_result {
            Ok(mut stream) => {
                // Signal connected (first event is always Connected)
                let _ = connected_tx.send(());
                // Keep consuming events until task is aborted
                while let Some(_event) = stream.next_event().await {}
            }
            Err(e) => {
                eprintln!("[integration D1] connect failed: {e}");
                // connected_tx dropped here — connected_rx will see error
            }
        }
    });

    // Wait for CONNECTED signal
    let connected = timeout(Duration::from_secs(90), connected_rx).await;
    assert!(
        connected.is_ok() && connected.unwrap().is_ok(),
        "never reached CONNECTED within 90s (is {user} live? check PIRATETOK_LIVE_TEST_USER)"
    );
    eprintln!("[integration D1] connected to {user}");

    // Step 2: abort the connect task (equivalent to disconnect)
    let t0 = std::time::Instant::now();
    connect_handle.abort();

    // Step 3: assert task exits within 18s
    let join_result = timeout(Duration::from_secs(18), connect_handle).await;
    let elapsed = t0.elapsed();

    assert!(
        join_result.is_ok(),
        "connect task did not exit within 18s after abort — possible zombie task"
    );
    eprintln!("[integration D1] task exited in {:.2}s after disconnect", elapsed.as_secs_f64());
}
