use log::{debug, error, info, warn};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

use crate::models::{ChatConnectionStatus, ChatMessage};

use super::auth::AuthMode;
use super::parse::{parse_youtube_chat_item, OutboundMessage};
use super::status::status_to_u8;

const OUTBOUND_DEDUP_WINDOW_SECS: u64 = 10;

/// Owns every handle the background poll task needs. Bundled into a struct so
/// the spawn entrypoint takes a single argument instead of a dozen positional
/// ones (which would trip `clippy::too_many_arguments`).
pub(super) struct PollTask {
    pub(super) status: Arc<AtomicU8>,
    pub(super) last_error: Arc<StdMutex<Option<String>>>,
    pub(super) message_count: Arc<AtomicU64>,
    pub(super) disconnecting: Arc<AtomicBool>,
    pub(super) recent_outbound: Arc<StdMutex<VecDeque<OutboundMessage>>>,
    pub(super) self_channel_id: Option<String>,
    pub(super) api_base: String,
    pub(super) auth_mode: AuthMode,
    pub(super) live_chat_id: String,
    pub(super) http_client: reqwest::Client,
    pub(super) message_tx: mpsc::Sender<ChatMessage>,
}

impl PollTask {
    pub(super) fn spawn(self, mut disconnect_rx: mpsc::Receiver<()>) {
        let PollTask {
            status,
            last_error,
            message_count,
            disconnecting,
            recent_outbound,
            self_channel_id,
            api_base,
            auth_mode,
            live_chat_id,
            http_client,
            message_tx,
        } = self;

        tokio::spawn(async move {
            let mut page_token: Option<String> = None;
            // Start with a reasonable default; updated from API response
            let mut poll_interval_ms: u64 = 6000;

            loop {
                // Check for disconnect signal
                if disconnect_rx.try_recv().is_ok() {
                    info!("YouTube chat disconnect signal received");
                    disconnecting.store(true, Ordering::Relaxed);
                    break;
                }

                // Build the request
                let mut url = format!(
                    "{}/liveChat/messages?liveChatId={}&part=snippet,authorDetails&maxResults=200",
                    api_base, live_chat_id
                );
                if let Some(ref token) = page_token {
                    url.push_str(&format!("&pageToken={}", token));
                }

                let request = auth_mode.apply(http_client.get(&url));
                match request.send().await {
                    Ok(resp) => {
                        if !resp.status().is_success() {
                            let http_status = resp.status();
                            // H6: surface the body-read failure instead
                            // of swallowing it with `unwrap_or_default()`.
                            // A failed `text()` previously hid disconnects
                            // mid-error-read as a generic empty body.
                            let body = match resp.text().await {
                                Ok(b) => b,
                                Err(e) => {
                                    warn!(
                                        "YouTube chat API error {}: body read failed: {}",
                                        http_status, e
                                    );
                                    format!("<body read failed: {e}>")
                                }
                            };
                            status.store(
                                status_to_u8(ChatConnectionStatus::Error),
                                Ordering::Relaxed,
                            );
                            // 403 often means quota exhausted; 401 means token expired
                            if http_status.as_u16() == 401 {
                                error!("YouTube API auth expired, waiting for token refresh");
                                poll_interval_ms = poll_interval_ms.max(30000);
                                if let Ok(mut guard) = last_error.lock() {
                                    *guard = Some("YouTube auth expired".to_string());
                                }
                            }
                            if http_status.as_u16() == 403 {
                                warn!("YouTube API quota may be exhausted (403). Backing off.");
                                poll_interval_ms = poll_interval_ms.max(30000);
                            } else {
                                warn!("YouTube chat API error ({}): {}", http_status, body);
                                if let Ok(mut guard) = last_error.lock() {
                                    *guard = Some(format!("YouTube API error {}", http_status));
                                }
                            }
                        } else {
                            match resp.json::<serde_json::Value>().await {
                                Ok(data) => {
                                    if let Ok(mut guard) = last_error.lock() {
                                        *guard = None;
                                    }
                                    status.store(
                                        status_to_u8(ChatConnectionStatus::Connected),
                                        Ordering::Relaxed,
                                    );

                                    // I4: reset the poll interval to base on every
                                    // successful poll BEFORE applying any API
                                    // recommendation. Pre-I4 the 401/403 paths
                                    // bumped `poll_interval_ms` to ≥30 s and the
                                    // reset only happened when the API explicitly
                                    // returned `pollingIntervalMillis`. If quota
                                    // recovered but the API omitted that field
                                    // (it's optional in the live-chat response),
                                    // the connector stayed stuck at 30 s polling
                                    // for the rest of the stream.
                                    poll_interval_ms = 6000;

                                    // Update polling interval from API recommendation
                                    if let Some(interval) = data["pollingIntervalMillis"].as_u64() {
                                        poll_interval_ms = interval;
                                    }

                                    // Update page token for next request
                                    page_token =
                                        data["nextPageToken"].as_str().map(|s| s.to_string());

                                    // Process messages
                                    if let Some(items) = data["items"].as_array() {
                                        for item in items {
                                            let Some(chat_msg) = parse_youtube_chat_item(item)
                                            else {
                                                continue;
                                            };

                                            // Skip our own outbound echoes inside the
                                            // dedup window. Needs the live
                                            // `recent_outbound` ring + own channel id,
                                            // so it stays in the loop.
                                            if let Some(self_id) = &self_channel_id {
                                                if item["authorDetails"]["channelId"].as_str()
                                                    == Some(self_id.as_str())
                                                {
                                                    let mut recent = recent_outbound
                                                        .lock()
                                                        .unwrap_or_else(|e| e.into_inner());
                                                    let now = Instant::now();
                                                    while let Some(front) = recent.front() {
                                                        if now
                                                            .duration_since(front.timestamp)
                                                            .as_secs()
                                                            > OUTBOUND_DEDUP_WINDOW_SECS
                                                        {
                                                            recent.pop_front();
                                                        } else {
                                                            break;
                                                        }
                                                    }
                                                    if recent
                                                        .iter()
                                                        .any(|entry| entry.text == chat_msg.message)
                                                    {
                                                        continue;
                                                    }
                                                }
                                            }

                                            if message_tx.send(chat_msg).await.is_err() {
                                                warn!("Failed to send YouTube message: receiver dropped");
                                                return;
                                            }
                                        }

                                        if !items.is_empty() {
                                            debug!(
                                                "Processed {} YouTube chat messages",
                                                items.len()
                                            );
                                            message_count
                                                .fetch_add(items.len() as u64, Ordering::Relaxed);
                                        }
                                    }
                                }
                                Err(e) => {
                                    warn!("Failed to parse YouTube chat response: {}", e);
                                    status.store(
                                        status_to_u8(ChatConnectionStatus::Error),
                                        Ordering::Relaxed,
                                    );
                                    if let Ok(mut guard) = last_error.lock() {
                                        *guard = Some(
                                            "Failed to parse YouTube chat response".to_string(),
                                        );
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        warn!("YouTube chat request failed: {}", e);
                        status.store(status_to_u8(ChatConnectionStatus::Error), Ordering::Relaxed);
                        if let Ok(mut guard) = last_error.lock() {
                            *guard = Some("YouTube chat request failed".to_string());
                        }
                    }
                }

                // Wait before next poll with ±10% jitter so N streamers
                // recovering from the same network blip don't all poll
                // YouTube on the same tick (thundering herd). Floor of
                // 100ms keeps the jitter range meaningful at small
                // intervals.
                let jitter_range = (poll_interval_ms / 10).max(100);
                let jitter = rand::random::<u64>() % jitter_range;
                let sleep_ms = poll_interval_ms.saturating_sub(jitter_range / 2) + jitter;
                tokio::time::sleep(Duration::from_millis(sleep_ms)).await;
            }

            if !disconnecting.load(Ordering::Relaxed) {
                status.store(status_to_u8(ChatConnectionStatus::Error), Ordering::Relaxed);
                if let Ok(mut guard) = last_error.lock() {
                    *guard = Some("YouTube chat polling stopped".to_string());
                }
            }
            info!("YouTube chat polling stopped");
        });
    }
}
