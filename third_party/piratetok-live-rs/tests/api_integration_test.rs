//! Integration tests for HTTP API calls — H1 through H4.
//!
//! All tests are gated behind env vars and skip cleanly (pass, not fail) when
//! the vars are absent. Set the appropriate vars before running:
//!
//! ```
//! PIRATETOK_LIVE_TEST_USER=<live_username>         cargo test --test api_integration_test
//! PIRATETOK_LIVE_TEST_OFFLINE_USER=<offline_user>  cargo test --test api_integration_test
//! PIRATETOK_LIVE_TEST_HTTP=1                       cargo test --test api_integration_test
//! PIRATETOK_LIVE_TEST_COOKIES=<cookie_header>      (optional, for 18+ room info)
//! ```

use std::time::Duration;

use piratetok_live_rs::errors::TikTokLiveError;
use piratetok_live_rs::http::api::{fetch_room_id, fetch_room_info, FetchParams};

/// Synthetic nonexistent username — deterministic, unlikely to be registered.
const SYNTHETIC_NONEXISTENT_USER: &str =
    "piratetok_rs_nf_7a3c9e2f1b8d4a6c0e5f3a2b1d9c8e7";

const HTTP_TIMEOUT: Duration = Duration::from_secs(25);

fn http_params() -> FetchParams<'static> {
    FetchParams { timeout: HTTP_TIMEOUT, ..Default::default() }
}

/// H1 — check_online with a live user returns a valid non-empty room ID.
///
/// Gate: `PIRATETOK_LIVE_TEST_USER`
#[tokio::test]
async fn check_online_live_user_returns_room_id() {
    let user = match std::env::var("PIRATETOK_LIVE_TEST_USER") {
        Ok(v) if !v.trim().is_empty() => v.trim().to_string(),
        _ => {
            eprintln!(
                "SKIP check_online_live_user_returns_room_id: \
                 set PIRATETOK_LIVE_TEST_USER to a live TikTok username"
            );
            return;
        }
    };

    let result = fetch_room_id(&user, http_params())
        .await
        .expect("fetch_room_id should succeed for a live user");

    assert!(!result.room_id.is_empty(), "room_id must not be empty");
    assert_ne!(result.room_id, "0", "room_id must not be \"0\"");
    eprintln!("[integration H1] user={user} room_id={}", result.room_id);
}

/// H2 — check_online with an offline user returns HostNotOnline, not Blocked or NotFound.
///
/// Gate: `PIRATETOK_LIVE_TEST_OFFLINE_USER`
#[tokio::test]
async fn check_online_offline_user_returns_host_not_online() {
    let user = match std::env::var("PIRATETOK_LIVE_TEST_OFFLINE_USER") {
        Ok(v) if !v.trim().is_empty() => v.trim().to_string(),
        _ => {
            eprintln!(
                "SKIP check_online_offline_user_returns_host_not_online: \
                 set PIRATETOK_LIVE_TEST_OFFLINE_USER to an offline TikTok username"
            );
            return;
        }
    };

    let result = fetch_room_id(&user, http_params()).await;
    let err = match result {
        Ok(_) => panic!("fetch_room_id should fail for offline user '{user}', but returned Ok"),
        Err(e) => e,
    };

    match &err {
        TikTokLiveError::HostNotOnline(msg) => {
            eprintln!("[integration H2] HostNotOnline: {msg}");
        }
        other => {
            panic!(
                "expected HostNotOnline for offline user '{user}', got: {other:?}\n\
                 Must NOT be confused with Blocked or UserNotFound."
            );
        }
    }
}

/// H3 — check_online with a nonexistent username returns UserNotFound.
///
/// Gate: `PIRATETOK_LIVE_TEST_HTTP=1` (safe to call anytime, but still a network call)
#[tokio::test]
async fn check_online_nonexistent_user_returns_user_not_found() {
    let enabled = std::env::var("PIRATETOK_LIVE_TEST_HTTP")
        .map(|v| matches!(v.trim(), "1" | "true" | "yes"))
        .unwrap_or(false);

    if !enabled {
        eprintln!(
            "SKIP check_online_nonexistent_user_returns_user_not_found: \
             set PIRATETOK_LIVE_TEST_HTTP=1 to call TikTok API for not-found probe"
        );
        return;
    }

    let result = fetch_room_id(SYNTHETIC_NONEXISTENT_USER, http_params()).await;
    let err = match result {
        Ok(_) => panic!(
            "fetch_room_id should fail for nonexistent user '{SYNTHETIC_NONEXISTENT_USER}', but returned Ok"
        ),
        Err(e) => e,
    };

    match &err {
        TikTokLiveError::UserNotFound(username) => {
            assert_eq!(
                username, SYNTHETIC_NONEXISTENT_USER,
                "UserNotFound must carry the username"
            );
            eprintln!("[integration H3] UserNotFound: {username}");
        }
        other => {
            panic!(
                "expected UserNotFound for synthetic user '{SYNTHETIC_NONEXISTENT_USER}', got: {other:?}"
            );
        }
    }
}

/// H4 — fetch_room_info for a live room returns room info with viewers >= 0.
///
/// Gate: `PIRATETOK_LIVE_TEST_USER` (uses optional `PIRATETOK_LIVE_TEST_COOKIES` for 18+ rooms)
#[tokio::test]
async fn fetch_room_info_live_room_returns_room_info() {
    let user = match std::env::var("PIRATETOK_LIVE_TEST_USER") {
        Ok(v) if !v.trim().is_empty() => v.trim().to_string(),
        _ => {
            eprintln!(
                "SKIP fetch_room_info_live_room_returns_room_info: \
                 set PIRATETOK_LIVE_TEST_USER to a live TikTok username"
            );
            return;
        }
    };

    let room = fetch_room_id(&user, http_params())
        .await
        .expect("fetch_room_id should succeed for a live user");

    let cookies_env = std::env::var("PIRATETOK_LIVE_TEST_COOKIES").unwrap_or_default();
    let cookies_str = cookies_env.trim();

    let info_result = if cookies_str.is_empty() {
        fetch_room_info(&room.room_id, http_params()).await
    } else {
        fetch_room_info(
            &room.room_id,
            FetchParams { timeout: HTTP_TIMEOUT, cookies: Some(cookies_str), ..Default::default() },
        )
        .await
    };

    match info_result {
        Ok(info) => {
            assert!(
                info.viewers >= 0,
                "viewer count must be >= 0, got {}",
                info.viewers
            );
            eprintln!(
                "[integration H4] room_id={} title={:?} viewers={}",
                room.room_id, info.title, info.viewers
            );
        }
        Err(TikTokLiveError::AgeRestricted(msg)) => {
            // 18+ room without cookies — acceptable when cookies not provided
            eprintln!(
                "[integration H4] AgeRestricted (18+ room, no cookies): {msg}\n\
                 Set PIRATETOK_LIVE_TEST_COOKIES to test 18+ room info fetch."
            );
        }
        Err(other) => {
            panic!("[integration H4] fetch_room_info failed: {other:?}");
        }
    }
}
