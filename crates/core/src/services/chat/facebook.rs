//! Facebook Live comments connector — read + send.
//!
//! Uses Meta's Graph API Live Video Comments endpoint
//! (`graph.facebook.com/{version}/{live-video-id}/comments`) with
//! long-polling to surface new comments and a POST against the same
//! endpoint to send.
//!
//! **Identity risk.** Connecting Facebook Live binds chat to the
//! authenticated user's real-name Facebook account per Meta's Name
//! Policy. The streamer's handle, profile picture, and friends-list
//! metadata that Meta exposes by default are all visible to anyone
//! watching the stream. Anonymous mode pseudonymises *inbound*
//! messages only — the streamer's own identity on the platform is
//! unchanged. This is a deliberate trade-off the user has signed off
//! on; the UI's connect path adds a confirm-token gate (see
//! `transport-http/src/chat_lifecycle.rs` Facebook activation flow)
//! so muscle-memory click-through can't silently enable it.
//!
//! **Auth.** Requires a Page Access Token with `pages_read_engagement`
//! and `pages_manage_engagement` scopes. The token is supplied via
//! `ChatCredentials::Facebook { video_id, access_token }`; the OAuth
//! flow that produces it is wired elsewhere (or supplied by the
//! operator from their Meta App dashboard).

use async_trait::async_trait;
use log::{error, info, warn};
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, AtomicU8, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;
use tokio::sync::{mpsc, Mutex as TokioMutex};
use tokio::task::JoinHandle;

use crate::models::{
    ChatConnectionStatus, ChatCredentials, ChatMessage, ChatPlatform as ChatPlatformEnum,
    MessageFlags,
};

use super::endpoints::ChatEndpoints;
use super::platform::{ChatPlatform, PlatformError, PlatformResult};
use super::self_echo::{SelfClass, SelfEcho};

const STATUS_DISCONNECTED: u8 = 0;
const STATUS_CONNECTING: u8 = 1;
const STATUS_CONNECTED: u8 = 2;
const STATUS_ERROR: u8 = 3;

/// Comment-poll cadence. Facebook's documented rate limit is 200
/// calls/hour/user; 6s leaves plenty of headroom and stays under the
/// "perceptibly real-time" threshold for chat aggregation.
const POLL_INTERVAL: Duration = Duration::from_secs(6);

/// Graph API version — shared with the OAuth provider so the dialog,
/// token, and comments endpoints can never drift apart. Pinned so a
/// Meta-breaking change doesn't silently upgrade us mid-stream;
/// bumping `FACEBOOK_GRAPH_VERSION` is a maintainer task.
const GRAPH_API_VERSION: &str = crate::services::oauth::FACEBOOK_GRAPH_VERSION;

fn status_to_u8(status: ChatConnectionStatus) -> u8 {
    match status {
        ChatConnectionStatus::Disconnected => STATUS_DISCONNECTED,
        ChatConnectionStatus::Connecting => STATUS_CONNECTING,
        ChatConnectionStatus::Connected => STATUS_CONNECTED,
        ChatConnectionStatus::Error => STATUS_ERROR,
    }
}

fn status_from_u8(value: u8) -> ChatConnectionStatus {
    match value {
        STATUS_CONNECTING => ChatConnectionStatus::Connecting,
        STATUS_CONNECTED => ChatConnectionStatus::Connected,
        STATUS_ERROR => ChatConnectionStatus::Error,
        _ => ChatConnectionStatus::Disconnected,
    }
}

/// Parses one Facebook Graph API comment into a [`ChatMessage`] plus the
/// comment's epoch-seconds cursor (present only when `created_time` parses).
/// Returns `None` for comments with blank message text. Pure — the poll loop
/// owns delivery, counting, and advancing the `since` cursor past the cursor
/// value returned here.
pub(super) fn parse_facebook_comment(
    comment: &serde_json::Value,
) -> Option<(ChatMessage, Option<i64>)> {
    let message_text = comment["message"].as_str().unwrap_or("").trim().to_string();
    if message_text.is_empty() {
        return None;
    }
    let username = comment["from"]["name"]
        .as_str()
        .unwrap_or("Anonymous")
        .to_string();

    let mut chat_msg = ChatMessage::new(ChatPlatformEnum::Facebook, username, message_text);
    if let Some(comment_id) = comment["id"].as_str() {
        chat_msg = chat_msg.with_source_id(comment_id.to_string());
    }

    // Facebook returns ISO-8601 (`created_time`). Parse + use as both the
    // message timestamp and the next-poll cursor. The Graph API renders the
    // offset *without* a colon (`2017-12-17T16:01:42+0000`), which
    // `parse_from_rfc3339` rejects; the explicit `%z` parse accepts it.
    // Without this the cursor never advances past 0 and every poll
    // re-fetches all comments, flooding chat with duplicates.
    let mut epoch_secs = None;
    if let Some(created_time) = comment["created_time"].as_str() {
        if let Ok(parsed) = chrono::DateTime::parse_from_rfc3339(created_time)
            .or_else(|_| chrono::DateTime::parse_from_str(created_time, "%Y-%m-%dT%H:%M:%S%z"))
        {
            chat_msg.timestamp = parsed.timestamp_millis();
            epoch_secs = Some(parsed.timestamp());
        }
    }

    Some((chat_msg, epoch_secs))
}

pub struct FacebookConnector {
    status: Arc<AtomicU8>,
    last_error: Arc<StdMutex<Option<String>>>,
    message_count: Arc<AtomicU64>,
    disconnecting: Arc<AtomicBool>,
    disconnect_tx: Option<mpsc::Sender<()>>,
    /// Send credentials captured at connect time. Cleared on disconnect.
    send_state: Arc<StdMutex<Option<SendState>>>,
    /// Self-message detector + outbound echo dedup, built at `connect()` from
    /// the user's own Facebook actor id. Shared between the poll task and `send`.
    self_echo: Arc<StdMutex<Option<Arc<SelfEcho>>>>,
    /// Q2: handle to the background Graph API poll task. Pre-this fix
    /// the spawn handle was dropped; a connector dropped without
    /// `disconnect()` (panic-disconnect, test teardown) leaked the
    /// task. `Drop` aborts so the runtime reclaims it.
    task_handle: Arc<TokioMutex<Option<JoinHandle<()>>>>,
    /// Graph API origin; the connector appends `/{version}/{id}/comments`.
    graph_base: String,
}

impl Drop for FacebookConnector {
    fn drop(&mut self) {
        if let Ok(mut guard) = self.task_handle.try_lock() {
            if let Some(handle) = guard.take() {
                handle.abort();
            }
        }
    }
}

#[derive(Clone)]
struct SendState {
    video_id: String,
    access_token: String,
}

impl FacebookConnector {
    pub fn new() -> Self {
        Self::with_endpoints(&ChatEndpoints::default())
    }

    pub fn with_endpoints(endpoints: &ChatEndpoints) -> Self {
        Self {
            status: Arc::new(AtomicU8::new(STATUS_DISCONNECTED)),
            last_error: Arc::new(StdMutex::new(None)),
            message_count: Arc::new(AtomicU64::new(0)),
            disconnecting: Arc::new(AtomicBool::new(false)),
            disconnect_tx: None,
            send_state: Arc::new(StdMutex::new(None)),
            self_echo: Arc::new(StdMutex::new(None)),
            task_handle: Arc::new(TokioMutex::new(None)),
            graph_base: endpoints.facebook_graph_base.clone(),
        }
    }

    fn set_error(&self, message: impl Into<String>) {
        self.status
            .store(status_to_u8(ChatConnectionStatus::Error), Ordering::Relaxed);
        if let Ok(mut guard) = self.last_error.lock() {
            *guard = Some(message.into());
        }
    }
}

#[async_trait]
impl ChatPlatform for FacebookConnector {
    async fn connect(
        &mut self,
        credentials: ChatCredentials,
        message_tx: mpsc::Sender<ChatMessage>,
    ) -> PlatformResult<()> {
        if self.is_connected() {
            return Err(PlatformError::AlreadyConnected);
        }

        self.status.store(
            status_to_u8(ChatConnectionStatus::Connecting),
            Ordering::Relaxed,
        );
        self.disconnecting.store(false, Ordering::Relaxed);
        self.message_count.store(0, Ordering::Relaxed);
        if let Ok(mut guard) = self.last_error.lock() {
            *guard = None;
        }

        let (video_id, access_token, self_identity) = match credentials {
            ChatCredentials::Facebook {
                video_id,
                access_token,
                self_identity,
            } => (video_id, access_token, self_identity),
            _ => {
                self.set_error("Expected Facebook credentials");
                return Err(PlatformError::InvalidConfig(
                    "Expected Facebook credentials".to_string(),
                ));
            }
        };

        let video_id = video_id.trim().to_string();
        let access_token = access_token.trim().to_string();
        if video_id.is_empty() {
            self.set_error("Facebook live video id is required");
            return Err(PlatformError::InvalidConfig(
                "Facebook live video id is required".to_string(),
            ));
        }
        if access_token.is_empty() {
            self.set_error("Facebook Page Access Token is required");
            return Err(PlatformError::Authentication(
                "Facebook Page Access Token is required".to_string(),
            ));
        }

        // Smoke-test the credentials with a single comments fetch before
        // declaring connected — Meta returns 400/401 on bad token or
        // mistyped video id, and failing fast saves the user a confusing
        // "no chat appearing" debug session.
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(15))
            .build()
            .map_err(|e| {
                self.set_error(format!("Failed to build Facebook HTTP client: {e}"));
                PlatformError::Network(format!("Failed to build HTTP client: {e}"))
            })?;

        let probe_url = format!(
            "{}/{GRAPH_API_VERSION}/{}/comments",
            self.graph_base,
            urlencoding::encode(&video_id)
        );
        let probe = client
            .get(&probe_url)
            .query(&[
                ("access_token", access_token.as_str()),
                ("limit", "1"),
                ("order", "reverse_chronological"),
                ("fields", "id,from{id,name},message,created_time"),
            ])
            .send()
            .await
            .map_err(|e| {
                self.set_error(format!("Facebook probe request failed: {e}"));
                PlatformError::Network(format!("Facebook probe failed: {e}"))
            })?;

        if probe.status() == reqwest::StatusCode::UNAUTHORIZED
            || probe.status() == reqwest::StatusCode::FORBIDDEN
        {
            self.set_error("Facebook Page Access Token rejected");
            return Err(PlatformError::Authentication(
                "Facebook Page Access Token rejected — \
                 check pages_read_engagement scope + token validity"
                    .to_string(),
            ));
        }
        if !probe.status().is_success() {
            let status = probe.status();
            let detail = match probe.text().await {
                Ok(b) => b,
                Err(e) => format!("<body read failed: {e}>"),
            };
            let msg = format!(
                "Facebook probe failed ({status}): {}",
                detail.chars().take(200).collect::<String>()
            );
            self.set_error(msg.clone());
            return Err(PlatformError::Platform(msg));
        }

        // Capture send credentials.
        if let Ok(mut guard) = self.send_state.lock() {
            *guard = Some(SendState {
                video_id: video_id.clone(),
                access_token: access_token.clone(),
            });
        }

        // Facebook actor ids are exact (numeric strings) — case-sensitive match.
        // Shared with the poll task (mark native self-comments) and `send_message`.
        let self_echo = Arc::new(SelfEcho::new(self_identity, false));
        if let Ok(mut guard) = self.self_echo.lock() {
            *guard = Some(self_echo.clone());
        }

        let (disconnect_tx, mut disconnect_rx) = mpsc::channel::<()>(1);
        self.disconnect_tx = Some(disconnect_tx);
        self.status.store(
            status_to_u8(ChatConnectionStatus::Connected),
            Ordering::Relaxed,
        );

        let status = self.status.clone();
        let last_error = self.last_error.clone();
        let message_count = self.message_count.clone();
        let disconnecting = self.disconnecting.clone();
        // Track the latest `created_time` we've seen so the next poll
        // uses the `since={epoch}` filter and never re-emits the same
        // comment twice.
        let since_epoch = Arc::new(AtomicI64::new(0));
        let video_id_polling = video_id.clone();
        let access_token_polling = access_token.clone();
        let graph_base = self.graph_base.clone();

        let task_handle = tokio::spawn(async move {
            let mut tick = tokio::time::interval(POLL_INTERVAL);
            // First tick fires immediately — fine for "catch up since
            // connect" semantics.

            loop {
                tokio::select! {
                    _ = tick.tick() => {
                        let url = format!(
                            "{}/{GRAPH_API_VERSION}/{}/comments",
                            graph_base,
                            urlencoding::encode(&video_id_polling)
                        );
                        let mut query: Vec<(&str, String)> = vec![
                            ("access_token", access_token_polling.clone()),
                            ("limit", "100".to_string()),
                            ("order", "chronological".to_string()),
                            ("fields", "id,from{id,name},message,created_time".to_string()),
                        ];
                        let since = since_epoch.load(Ordering::Relaxed);
                        if since > 0 {
                            query.push(("since", since.to_string()));
                        }

                        let response = match client.get(&url).query(&query).send().await {
                            Ok(r) => r,
                            Err(err) => {
                                warn!("Facebook comments poll failed: {err}");
                                if let Ok(mut guard) = last_error.lock() {
                                    *guard = Some(format!("Facebook poll failed: {err}"));
                                }
                                // Transient network errors don't tear down the connection;
                                // wait for the next tick and try again.
                                continue;
                            }
                        };

                        if response.status() == reqwest::StatusCode::UNAUTHORIZED
                            || response.status() == reqwest::StatusCode::FORBIDDEN
                        {
                            error!("Facebook comments fetch unauthorized (token expired/revoked)");
                            if let Ok(mut guard) = last_error.lock() {
                                *guard = Some("Facebook token expired or revoked".to_string());
                            }
                            status.store(status_to_u8(ChatConnectionStatus::Error), Ordering::Relaxed);
                            break;
                        }
                        if !response.status().is_success() {
                            let resp_status = response.status();
                            let detail = match response.text().await {
                                Ok(b) => b,
                                Err(e) => format!("<body read failed: {e}>"),
                            };
                            warn!(
                                "Facebook comments fetch failed ({resp_status}): {}",
                                detail.chars().take(200).collect::<String>()
                            );
                            if let Ok(mut guard) = last_error.lock() {
                                *guard = Some(format!(
                                    "Facebook poll {resp_status}: {}",
                                    detail.chars().take(120).collect::<String>()
                                ));
                            }
                            continue;
                        }

                        let body: serde_json::Value = match response.json().await {
                            Ok(v) => v,
                            Err(err) => {
                                warn!("Failed to parse Facebook response: {err}");
                                continue;
                            }
                        };

                        let comments = match body["data"].as_array() {
                            Some(c) => c,
                            None => continue,
                        };

                        let mut emitted = 0_u64;
                        let mut latest_epoch = since;
                        for comment in comments {
                            let Some((mut chat_msg, epoch_secs)) = parse_facebook_comment(comment)
                            else {
                                continue;
                            };
                            if let Some(epoch) = epoch_secs {
                                if epoch > latest_epoch {
                                    latest_epoch = epoch;
                                }
                            }

                            // Match on the stable `from.id` (the parser keeps
                            // `from.name` as the username but drops the id).
                            // Drop our own app-sent echo; mark a natively typed
                            // self-comment as "you".
                            let author_id = comment["from"]["id"].as_str().unwrap_or("");
                            let class = self_echo.classify(author_id, &chat_msg.message);
                            if class == SelfClass::Echo {
                                continue;
                            }
                            if class == SelfClass::Native {
                                chat_msg.flags |= MessageFlags::SELF_AUTHOR;
                            }

                            if message_tx.send(chat_msg).await.is_err() {
                                warn!("Failed to deliver Facebook comment: receiver dropped");
                                break;
                            }
                            emitted += 1;
                        }
                        if emitted > 0 {
                            message_count.fetch_add(emitted, Ordering::Relaxed);
                        }
                        // Advance the cursor past the newest comment SEEN (even
                        // ones skipped as our own echo), or an echo-only poll
                        // would never advance and refetch forever.
                        if latest_epoch > since {
                            since_epoch.store(latest_epoch + 1, Ordering::Relaxed);
                        }
                    }
                    _ = disconnect_rx.recv() => {
                        info!("Facebook chat disconnect requested");
                        break;
                    }
                }
            }

            if !disconnecting.load(Ordering::Relaxed) {
                status.store(status_to_u8(ChatConnectionStatus::Error), Ordering::Relaxed);
                if let Ok(mut guard) = last_error.lock() {
                    if guard.is_none() {
                        *guard = Some("Facebook connection lost".to_string());
                    }
                }
            }
            info!("Facebook chat task stopped");
        });

        // Q2: capture the poll task handle so `Drop` aborts it.
        {
            let mut guard = self.task_handle.lock().await;
            if let Some(prev) = guard.take() {
                prev.abort();
            }
            *guard = Some(task_handle);
        }

        info!(
            "Connected to Facebook Live comments for video {video_id} (real-name identity exposed)"
        );
        Ok(())
    }

    async fn disconnect(&mut self) -> PlatformResult<()> {
        if !self.is_connected() {
            return Err(PlatformError::NotConnected);
        }

        self.disconnecting.store(true, Ordering::Relaxed);
        if let Some(tx) = self.disconnect_tx.take() {
            let _ = tx.send(()).await;
        }

        self.status.store(
            status_to_u8(ChatConnectionStatus::Disconnected),
            Ordering::Relaxed,
        );
        if let Ok(mut guard) = self.send_state.lock() {
            *guard = None;
        }
        if let Ok(mut guard) = self.last_error.lock() {
            *guard = None;
        }

        Ok(())
    }

    fn status(&self) -> ChatConnectionStatus {
        status_from_u8(self.status.load(Ordering::Relaxed))
    }

    fn message_count(&self) -> u64 {
        self.message_count.load(Ordering::Relaxed)
    }

    fn platform_name(&self) -> &'static str {
        "facebook"
    }

    fn can_send(&self) -> bool {
        if !self.is_connected() {
            return false;
        }
        self.send_state.lock().map(|g| g.is_some()).unwrap_or(false)
    }

    async fn send_message(&mut self, message: String) -> PlatformResult<()> {
        if !self.is_connected() {
            return Err(PlatformError::NotConnected);
        }
        let state = match self.send_state.lock() {
            Ok(g) => g.clone(),
            Err(_) => None,
        };
        let state = state.ok_or_else(|| {
            PlatformError::Platform(
                "Facebook send disabled — no access token captured at connect".to_string(),
            )
        })?;

        // Record for echo-dedup: this comment comes back on the next poll.
        if let Some(echo) = self.self_echo.lock().ok().and_then(|g| g.clone()) {
            echo.record_outbound(&message);
        }

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(15))
            .build()
            .map_err(|e| {
                PlatformError::Network(format!("Failed to build Facebook HTTP client: {e}"))
            })?;

        let url = format!(
            "{}/{GRAPH_API_VERSION}/{}/comments",
            self.graph_base,
            urlencoding::encode(&state.video_id)
        );

        let response = client
            .post(&url)
            .form(&[
                ("access_token", state.access_token.as_str()),
                ("message", message.as_str()),
            ])
            .send()
            .await
            .map_err(|e| PlatformError::Network(format!("Facebook send request failed: {e}")))?;

        let status = response.status();
        if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
            return Err(PlatformError::Authentication(
                "Facebook access token rejected — re-auth required".to_string(),
            ));
        }
        if !status.is_success() {
            let detail = match response.text().await {
                Ok(b) => b,
                Err(e) => format!("<body read failed: {e}>"),
            };
            return Err(PlatformError::Platform(format!(
                "Facebook send failed ({status}): {}",
                detail.chars().take(200).collect::<String>()
            )));
        }
        Ok(())
    }

    fn last_error(&self) -> Option<String> {
        self.last_error.lock().ok().and_then(|e| e.clone())
    }
}

impl Default for FacebookConnector {
    fn default() -> Self {
        Self::new()
    }
}
