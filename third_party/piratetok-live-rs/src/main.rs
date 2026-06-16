//! TikTokLive — connects to a live stream and prints all events.
//!
//! Setup:
//!   cp .env.example .env
//!   # edit .env with your username
//!   cargo run
//!
//! Or pass args directly:
//!   cargo run -- --username tiktok
//!
//! Set RUST_LOG=debug for verbose protocol logging.

use clap::Parser;
use piratetok_live_rs::http::api::{fetch_room_info, FetchParams};
use piratetok_live_rs::structs::TikTokLiveEvent;
use piratetok_live_rs::TikTokLive;
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(about = "Connect to a TikTok Live stream and print events")]
struct Args {
    /// TikTok username (with or without @)
    #[arg(short, long, env = "TIKTOK_USERNAME")]
    username: String,

    /// Also fetch room info (title, stream URLs). Optional.
    #[arg(long)]
    room_info: bool,

    /// Session cookies for 18+ room info (only needed with --room-info on 18+ rooms)
    #[arg(long, env = "TIKTOK_COOKIES")]
    cookies: Option<String>,
}

#[tokio::main]
async fn main() {
    let _ = dotenvy::dotenv();

    tracing_subscriber::fmt().with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"))).init();

    let args = Args::parse();

    let mut stream = match TikTokLive::builder(&args.username).connect().await {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to connect: {e}");
            return;
        }
    };

    while let Some(event) = stream.next_event().await {
        match event {
            TikTokLiveEvent::Connected { room_id } => {
                println!("[connected] room_id={room_id}");

                if args.room_info {
                    let cookies = args.cookies.as_deref();
                    match fetch_room_info(&room_id, FetchParams { cookies, ..Default::default() }).await {
                        Ok(info) => {
                            println!("[room] title=\"{}\" viewers={} likes={}", info.title, info.viewers, info.likes);
                            if let Some(urls) = &info.stream_url {
                                let url = urls.flv_sd.as_deref()
                                    .or(urls.flv_ld.as_deref())
                                    .or(urls.flv_origin.as_deref())
                                    .unwrap_or("n/a");
                                println!("[stream] {url}");
                            }
                        }
                        Err(e) => {
                            eprintln!("[room-info] failed: {e}");
                        }
                    }
                }
            }

            TikTokLiveEvent::Chat(msg) => {
                let nickname = match &msg.user {
                    Some(user) => user.nickname.as_str(),
                    None => "?",
                };
                println!("[chat] {nickname}: {}", msg.comment);
            }

            TikTokLiveEvent::Gift(msg) => {
                let nickname = match &msg.user {
                    Some(user) => user.nickname.as_str(),
                    None => "?",
                };
                let gift_name = match &msg.gift_details {
                    Some(details) => details.gift_name.as_str(),
                    None => "unknown",
                };
                let diamonds = msg.diamond_total();

                if msg.is_combo_gift() {
                    if msg.is_streak_over() {
                        println!("[gift] {nickname} sent {gift_name} x{} (streak ended, {diamonds} diamonds)", msg.repeat_count);
                    }
                } else {
                    println!("[gift] {nickname} sent {gift_name} ({diamonds} diamonds)");
                }
            }

            TikTokLiveEvent::Like(msg) => {
                let nickname = match &msg.user {
                    Some(user) => user.nickname.as_str(),
                    None => "?",
                };
                println!("[like] {nickname} ({} total)", msg.total_like_count);
            }

            TikTokLiveEvent::Follow(msg) => {
                let nickname = match &msg.user {
                    Some(user) => user.nickname.as_str(),
                    None => "?",
                };
                println!("[follow] {nickname}");
            }

            TikTokLiveEvent::Share(msg) => {
                let nickname = match &msg.user {
                    Some(user) => user.nickname.as_str(),
                    None => "?",
                };
                println!("[share] {nickname}");
            }

            TikTokLiveEvent::Join(msg) => {
                println!("[join] member_count={}", msg.member_count);
            }

            TikTokLiveEvent::RoomUserSeq(msg) => {
                println!("[viewers] {} watching, {} total", msg.viewer_count, msg.total_user);
            }

            TikTokLiveEvent::LiveEnded(_) => {
                println!("[control] stream ended");
                break;
            }

            // skip raw Social/Member/Control since we handle via convenience events above
            TikTokLiveEvent::Social(_) | TikTokLiveEvent::Member(_) | TikTokLiveEvent::Control(_) => {}

            TikTokLiveEvent::Envelope(msg) => {
                if let Some(info) = &msg.envelope_info {
                    println!("[envelope] from={} diamonds={}", info.send_user_name, info.diamond_count);
                }
            }

            TikTokLiveEvent::Disconnected => {
                println!("[disconnected]");
                break;
            }

            _ => {}
        }
    }
}
