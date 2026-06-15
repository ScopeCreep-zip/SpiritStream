use log::{debug, error, info, warn};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

use crate::models::{ChatConnectionStatus, ChatMessage, MessageFlags};

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
    /// Spawn THE single poll loop for this connection and return its handle.
    /// The connector stores the handle: `is_active()` consults it (so a 2nd
    /// connect is a no-op while this loop lives), and `disconnect()`/`Drop`
    /// abort it. There is exactly one poll loop per connection, and its
    /// `page_token` is preserved across its whole life — so the backlog is
    /// never re-fetched (the duplicate-flood fix).
    pub(super) fn spawn(self, mut disconnect_rx: mpsc::Receiver<()>) -> tokio::task::JoinHandle<()> {
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
            // Consecutive transient failures → exponential backoff (single
            // task self-healing, quota-friendly). Reset on every good poll.
            let mut consecutive_errors: u32 = 0;

            loop {
                // Exit on EITHER an explicit disconnect signal OR the channel
                // closing. A reconnect overwrites the connector's
                // `disconnect_tx`, which DROPS this poller's sender — that
                // closes the channel (`Disconnected`) without sending a value.
                // The old check (`try_recv().is_ok()`) ignored a closed
                // channel, so every reconnect leaked a still-running poller;
                // the survivors kept polling the same chat and flooding the
                // feed with duplicates. Treat closed exactly like a signal.
                match disconnect_rx.try_recv() {
                    Ok(()) | Err(mpsc::error::TryRecvError::Disconnected) => {
                        info!("YouTube chat poller stopping (disconnect signal or channel closed)");
                        disconnecting.store(true, Ordering::Relaxed);
                        break;
                    }
                    Err(mpsc::error::TryRecvError::Empty) => {}
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
                            // 403 = quota exhausted: a hard daily limit that
                            // won't recover until reset, so STOP polling instead
                            // of hammering it every 30s. Report a calm
                            // Disconnected (the reconnect loop fires on Error,
                            // so it won't churn). `disconnecting = true`
                            // suppresses the post-loop Error store. The user
                            // reconnects once quota resets.
                            if http_status.as_u16() == 403 {
                                warn!(
                                    "YouTube API quota exhausted (403); stopping the chat poll until reconnect"
                                );
                                if let Ok(mut guard) = last_error.lock() {
                                    *guard = Some(
                                        "YouTube API quota exhausted (resets daily ~midnight US Pacific)"
                                            .to_string(),
                                    );
                                }
                                disconnecting.store(true, Ordering::Relaxed);
                                status.store(
                                    status_to_u8(ChatConnectionStatus::Disconnected),
                                    Ordering::Relaxed,
                                );
                                break;
                            }
                            consecutive_errors = consecutive_errors.saturating_add(1);
                            status.store(
                                status_to_u8(ChatConnectionStatus::Error),
                                Ordering::Relaxed,
                            );
                            // 401 means the token expired — keep the connection
                            // and back off until the refresh task swaps it in.
                            if http_status.as_u16() == 401 {
                                error!("YouTube API auth expired, waiting for token refresh");
                                poll_interval_ms = poll_interval_ms.max(30000);
                                if let Ok(mut guard) = last_error.lock() {
                                    *guard = Some("YouTube auth expired".to_string());
                                }
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
                                    consecutive_errors = 0;
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
                                            let Some(mut chat_msg) = parse_youtube_chat_item(item)
                                            else {
                                                continue;
                                            };

                                            // Is this from our own channel? If it matches a
                                            // recent app-sent message it's the echo of an
                                            // outbound we already show — drop it. If it
                                            // survives, it was typed in YouTube's native chat,
                                            // so mark it SELF_AUTHOR to render as "you".
                                            let mut is_self = false;
                                            if let Some(self_id) = &self_channel_id {
                                                if item["authorDetails"]["channelId"].as_str()
                                                    == Some(self_id.as_str())
                                                {
                                                    is_self = true;
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
                                            if is_self {
                                                chat_msg.flags |= MessageFlags::SELF_AUTHOR;
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
                                    consecutive_errors = consecutive_errors.saturating_add(1);
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
                        consecutive_errors = consecutive_errors.saturating_add(1);
                        status.store(status_to_u8(ChatConnectionStatus::Error), Ordering::Relaxed);
                        if let Ok(mut guard) = last_error.lock() {
                            *guard = Some("YouTube chat request failed".to_string());
                        }
                    }
                }

                // Next-poll delay: the API-recommended interval on success, or
                // an exponential backoff (2s,4s,8s… cap 30s) while erroring, so
                // a single self-healing loop doesn't hammer the API (or quota)
                // through an outage. ±10% jitter avoids a thundering herd of
                // streamers recovering on the same tick.
                let base_ms = if consecutive_errors > 0 {
                    (2000u64.saturating_mul(1 << consecutive_errors.min(4).saturating_sub(1)))
                        .min(30000)
                } else {
                    poll_interval_ms
                };
                let jitter_range = (base_ms / 10).max(100);
                let jitter = rand::random::<u64>() % jitter_range;
                let sleep_ms = base_ms.saturating_sub(jitter_range / 2) + jitter;
                tokio::time::sleep(Duration::from_millis(sleep_ms)).await;
            }

            if !disconnecting.load(Ordering::Relaxed) {
                status.store(status_to_u8(ChatConnectionStatus::Error), Ordering::Relaxed);
                if let Ok(mut guard) = last_error.lock() {
                    *guard = Some("YouTube chat polling stopped".to_string());
                }
            }
            info!("YouTube chat polling stopped");
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    /// A reconnect overwrites the connector's `disconnect_tx`, dropping THIS
    /// poller's sender — the channel closes WITHOUT a value being sent. The
    /// poller must stop anyway; otherwise every reconnect leaks a poller that
    /// keeps hitting the API and floods the feed with duplicates.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn poller_stops_when_disconnect_channel_is_dropped() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/liveChat/messages"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "pollingIntervalMillis": 80,
                "nextPageToken": "p",
                "items": []
            })))
            .mount(&server)
            .await;

        let (msg_tx, _msg_rx) = mpsc::channel::<ChatMessage>(16);
        let task = PollTask {
            status: Arc::new(AtomicU8::new(status_to_u8(ChatConnectionStatus::Connected))),
            last_error: Arc::new(StdMutex::new(None)),
            message_count: Arc::new(AtomicU64::new(0)),
            disconnecting: Arc::new(AtomicBool::new(false)),
            recent_outbound: Arc::new(StdMutex::new(VecDeque::new())),
            self_channel_id: None,
            api_base: server.uri(),
            auth_mode: AuthMode::ApiKey { key: "k".into() },
            live_chat_id: "lc".into(),
            http_client: reqwest::Client::new(),
            message_tx: msg_tx,
        };
        let (disconnect_tx, disconnect_rx) = mpsc::channel::<()>(1);
        let _poll = task.spawn(disconnect_rx);

        // Let it poll a few times.
        tokio::time::sleep(Duration::from_millis(300)).await;
        let before = server.received_requests().await.unwrap().len();
        assert!(before >= 2, "poller should be actively polling (got {before})");

        // Drop the sender → channel closes (the reconnect-replacement case).
        drop(disconnect_tx);

        // It must stop within one poll cycle (allow one in-flight request),
        // and stay stopped.
        tokio::time::sleep(Duration::from_millis(300)).await;
        let after = server.received_requests().await.unwrap().len();
        tokio::time::sleep(Duration::from_millis(300)).await;
        let later = server.received_requests().await.unwrap().len();

        assert!(
            after - before <= 1,
            "at most one in-flight poll after the channel closed (before={before}, after={after})"
        );
        assert_eq!(
            after, later,
            "poller must be fully stopped after the channel closed (after={after}, later={later})"
        );
    }
}
