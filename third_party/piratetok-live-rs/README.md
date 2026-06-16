<p align="center">
  <img src="https://raw.githubusercontent.com/PirateTok/.github/main/profile/assets/og-banner-v2.png" alt="PirateTok" width="640" />
</p>

# piratetok-live-rs

Connect to any TikTok Live stream and receive real-time events — chat, gifts, likes, joins, viewer counts, and 64 decoded event types. No signing server, no API keys, no authentication required.

```rust
use piratetok_live_rs::TikTokLive;
use piratetok_live_rs::structs::TikTokLiveEvent;

#[tokio::main]
async fn main() {
    // Connect to a live stream — handles auth, room resolution, and WSS automatically
    let mut stream = TikTokLive::builder("username_here")
        .connect()
        .await
        .unwrap();

    // Each event is a fully decoded protobuf message
    while let Some(event) = stream.next_event().await {
        match event {
            TikTokLiveEvent::Chat(msg) => {
                let nick = msg.user.as_ref().map_or("?", |u| u.nickname.as_str());
                println!("{nick}: {}", msg.comment);
            }
            TikTokLiveEvent::Gift(msg) => {
                let nick = msg.user.as_ref().map_or("?", |u| u.nickname.as_str());
                let gift = msg.gift_details.as_ref().map_or("gift", |g| g.gift_name.as_str());
                let diamonds = msg.gift_details.as_ref().map_or(0, |g| g.diamond_count);
                println!("{nick} sent {gift} ({diamonds} diamonds)");
            }
            TikTokLiveEvent::Like(msg) => {
                println!("{} total likes", msg.total_like_count);
            }
            TikTokLiveEvent::Disconnected => break,
            _ => {} // 64 decoded event types available
        }
    }
}
```

## Install

```toml
[dependencies]
piratetok-live-rs = "0.1"
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

## Other languages

| Language | Install | Repo |
|:---------|:--------|:-----|
| **Go** | `go get github.com/PirateTok/live-go` | [live-go](https://github.com/PirateTok/live-go) |
| **Python** | `pip install piratetok-live-py` | [live-py](https://github.com/PirateTok/live-py) |
| **JavaScript** | `npm install piratetok-live-js` | [live-js](https://github.com/PirateTok/live-js) |
| **C#** | `dotnet add package PirateTok.Live` | [live-cs](https://github.com/PirateTok/live-cs) |
| **Java** | `com.piratetok:live` | [live-java](https://github.com/PirateTok/live-java) |
| **Lua** | `luarocks install piratetok-live-lua` | [live-lua](https://github.com/PirateTok/live-lua) |
| **Elixir** | `{:piratetok_live, "~> 0.1"}` | [live-ex](https://github.com/PirateTok/live-ex) |
| **Dart** | `dart pub add piratetok_live` | [live-dart](https://github.com/PirateTok/live-dart) |
| **C** | `#include "piratetok.h"` | [live-c](https://github.com/PirateTok/live-c) |
| **PowerShell** | `Install-Module PirateTok.Live` | [live-ps1](https://github.com/PirateTok/live-ps1) |
| **Shell** | `bpkg install PirateTok/live-sh` | [live-sh](https://github.com/PirateTok/live-sh) |

## Features

- **Zero signing dependency** — no API keys, no signing server, no external auth
- **64 decoded event types** — chat, gifts, likes, joins, follows, shares, battles, polls, envelopes, and more
- **Auto-reconnection** — stale detection, exponential backoff, self-healing auth
- **Enriched User data** — badges, gifter level, moderator status, follow info, fan club, subscriber status
- **Sub-routed convenience events** — `Follow`, `Share`, `Join`, `LiveEnded` fire alongside raw events

## Configuration

```rust
TikTokLive::builder("username_here")
    .cdn(CdnEndpoint::Eu)        // EU / US / Global (default)
    .timeout(Duration::from_secs(15))
    .heartbeat_interval(Duration::from_secs(10))  // default 10s
    .max_retries(10)             // default 5, 0 to disable
    .stale_timeout(Duration::from_secs(90))  // default 60s
    .proxy("socks5://127.0.0.1:1080")        // HTTP/HTTPS/SOCKS5 proxy for all connections
    .user_agent("Mozilla/5.0 ...")            // override random UA pool with a fixed UA
    .cookies("sessionid=abc; sid_tt=abc")     // session cookies appended alongside ttwid
    .language("de")              // override auto-detected language (default from system locale)
    .region("DE")                // override auto-detected region (default from system locale)
    .compress(false)             // disable gzip for WSS payloads (default true)
    .connect()
    .await
```

## Room info (optional, separate call)

```rust
use piratetok_live_rs::http::api::{fetch_room_info, FetchParams};

// Normal rooms — no cookies needed
let info = fetch_room_info("ROOM_ID", FetchParams::default()).await?;

// 18+ rooms — pass session cookies from browser DevTools
let info = fetch_room_info("ROOM_ID", FetchParams {
    cookies: Some("sessionid=abc; sid_tt=abc"), ..Default::default()
}).await?;
```

## Gift streaks

```rust
TikTokLiveEvent::Gift(gift) => {
    if gift.is_combo_gift() {
        if gift.is_streak_over() {
            println!("x{} = {} diamonds", gift.repeat_count, gift.diamond_total());
        }
    } else {
        println!("{} diamonds", gift.diamond_total());
    }
}
```

## How it works

1. Resolves username to room ID via TikTok JSON API
2. Authenticates and opens a direct WSS connection
3. Sends protobuf heartbeats every 10s to keep alive
4. Decodes protobuf event stream into typed Rust structs
5. Auto-reconnects on stale/dropped connections with fresh credentials

All protobuf structs are hand-written with `prost` derive macros — no `.proto` files, no codegen, no build-time protoc dependency.

## Examples

```bash
cargo run --example basic_chat -- <username>        # connect + print chat events
cargo run --example online_check -- <username>      # check if user is live
cargo run --example stream_info -- <username>       # fetch room metadata + stream URLs
cargo run --example gift_tracker -- <username>      # track gifts with diamond totals
cargo run --example gift_streak -- <username>       # per-event gift deltas via GiftStreakTracker
cargo run --example profile_lookup -- <username>    # look up profile data + HD avatars
cargo run --example like_debug -- <username>        # debug like event patterns
```

## Replay testing

Deterministic cross-lib validation against binary WSS captures. Requires testdata from a separate repo:

```bash
git clone https://github.com/PirateTok/live-testdata testdata
cargo test
```

Tests skip gracefully if testdata is not found. You can also set `PIRATETOK_TESTDATA` to point to a custom location.

## License

0BSD
