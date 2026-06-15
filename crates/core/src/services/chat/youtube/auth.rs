use tokio::sync::watch;

use super::super::platform::PlatformError;

/// Auth info extracted from credentials for API calls
#[derive(Clone)]
pub(super) enum AuthMode {
    /// OAuth Bearer token shared via watch channel (supports live refresh)
    OAuth {
        access_token_rx: watch::Receiver<String>,
    },
    /// API key query parameter
    ApiKey { key: String },
}

impl AuthMode {
    /// Apply auth to a reqwest RequestBuilder
    pub(super) fn apply(&self, builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        match self {
            AuthMode::OAuth { access_token_rx } => {
                let token = access_token_rx.borrow().clone();
                builder.header("Authorization", format!("Bearer {}", token))
            }
            AuthMode::ApiKey { key } => builder.query(&[("key", key.as_str())]),
        }
    }
}

/// Map a non-2xx YouTube API response to the right error by reading the actual
/// Google error `reason` — NEVER by guessing from the status code (a 403 is
/// quota for `quotaExceeded` but an auth failure for `authError`; conflating
/// them sent users to chase a daily-quota reset for a dead token). Quota →
/// `QuotaExceeded` (calm, wait for reset). Auth → `Authentication` (re-sign-in).
/// Everything else preserves the real reason + body.
fn youtube_http_error(status: reqwest::StatusCode, body: String) -> PlatformError {
    let reason = serde_json::from_str::<serde_json::Value>(&body)
        .ok()
        .and_then(|v| v["error"]["errors"][0]["reason"].as_str().map(str::to_string))
        .unwrap_or_default();
    let code = status.as_u16();
    match reason.as_str() {
        "quotaExceeded" | "rateLimitExceeded" => PlatformError::QuotaExceeded(
            "YouTube API quota exhausted (resets daily ~midnight US Pacific). For sustained \
             chat, request a higher quota in Google Cloud Console (APIs & Services → YouTube \
             Data API v3 → Quotas)."
                .to_string(),
        ),
        "authError" => PlatformError::Authentication(
            "YouTube rejected your sign-in (invalid credentials). Sign out of YouTube here and \
             sign back in. If your Google app is still in \"Testing\", publish it — testing-mode \
             tokens expire after 7 days."
                .to_string(),
        ),
        _ if code == 401 => PlatformError::Authentication(
            "YouTube authentication expired. Sign out of YouTube here and sign back in.".to_string(),
        ),
        _ => PlatformError::Platform(format!("YouTube API error ({code} {reason}): {body}")),
    }
}

/// Find the live chat for the user's CURRENTLY-LIVE broadcast, returning
/// `(live_chat_id, owner_channel_id)`. The owner id (the broadcast's canonical
/// `UCxxxx`) is what the poll loop uses to suppress the streamer's OWN echoed
/// messages — it must NOT be the user-typed channel setting (a handle/URL won't
/// match `authorDetails.channelId`).
///
/// ACTIVE-ONLY by design: a broadcast with `broadcastStatus=active` is LIVE,
/// so its chat is both readable AND postable. An `upcoming`/scheduled broadcast
/// has a readable chat too, but `liveChatMessages.insert` rejects it with HTTP
/// 400 `INVALID_REQUEST_METADATA` until it goes live — so attaching to one
/// gives a "connected but can't send" trap. We refuse it and fail loud as
/// `NotLive` instead.
pub(super) async fn find_live_chat_id(
    client: &reqwest::Client,
    auth: &AuthMode,
    channel_id: &str,
    api_base: &str,
) -> Result<(String, Option<String>), PlatformError> {
    match auth {
        AuthMode::OAuth { .. } => {
            // `liveBroadcasts.list?broadcastStatus=active` (5 quota units) —
            // the status filter already scopes to the authenticated user's
            // broadcasts, so it returns this account's live broadcast.
            let url = format!("{}/liveBroadcasts", api_base);
            let resp = auth
                .apply(client.get(&url))
                .query(&[
                    ("part", "snippet"),
                    ("broadcastStatus", "active"),
                    ("broadcastType", "all"),
                ])
                .send()
                .await
                .map_err(|e| PlatformError::Network(format!("Failed to fetch broadcasts: {}", e)))?;

            if !resp.status().is_success() {
                let status = resp.status();
                let body = resp
                    .text()
                    .await
                    .unwrap_or_else(|e| format!("<failed to read response body: {e}>"));
                return Err(youtube_http_error(status, body));
            }

            let data: serde_json::Value = resp.json().await.map_err(|e| {
                PlatformError::Network(format!("Failed to parse broadcasts response: {}", e))
            })?;

            data["items"]
                .as_array()
                .and_then(|items| items.first())
                .and_then(|item| {
                    let snippet = &item["snippet"];
                    let chat_id = snippet["liveChatId"].as_str()?.to_string();
                    let owner = snippet["channelId"].as_str().map(|s| s.to_string());
                    Some((chat_id, owner))
                })
                .ok_or_else(|| {
                    PlatformError::NotLive(
                        "No active YouTube broadcast found. Go live on YouTube, then connect chat."
                            .to_string(),
                    )
                })
        }
        AuthMode::ApiKey { .. } => {
            // API key mode: search for live videos, then get liveStreamingDetails
            // Find live video for the channel (100 quota units)
            let search_url = format!("{}/search", api_base);
            let resp = auth
                .apply(client.get(&search_url))
                .query(&[
                    ("part", "id"),
                    ("channelId", channel_id),
                    ("type", "video"),
                    ("eventType", "live"),
                    ("maxResults", "1"),
                ])
                .send()
                .await
                .map_err(|e| {
                    PlatformError::Network(format!("Failed to search live videos: {}", e))
                })?;

            if !resp.status().is_success() {
                let status = resp.status();
                let body = resp
                    .text()
                    .await
                    .unwrap_or_else(|e| format!("<failed to read response body: {e}>"));
                return Err(youtube_http_error(status, body));
            }

            let search_data: serde_json::Value = resp.json().await.map_err(|e| {
                PlatformError::Network(format!("Failed to parse search response: {}", e))
            })?;

            let video_id = search_data["items"]
                .as_array()
                .and_then(|items| items.first())
                .and_then(|item| item["id"]["videoId"].as_str())
                .ok_or_else(|| {
                    PlatformError::NotLive(
                        "No active live stream found for this channel. Make sure the channel is currently live streaming.".to_string(),
                    )
                })?
                .to_string();

            // Get liveStreamingDetails for the video (1 quota unit)
            let videos_url = format!("{}/videos", api_base);
            let resp = auth
                .apply(client.get(&videos_url))
                .query(&[("part", "liveStreamingDetails"), ("id", &video_id)])
                .send()
                .await
                .map_err(|e| {
                    PlatformError::Network(format!("Failed to fetch video details: {}", e))
                })?;

            if !resp.status().is_success() {
                let status = resp.status();
                let body = resp
                    .text()
                    .await
                    .unwrap_or_else(|e| format!("<failed to read response body: {e}>"));
                return Err(youtube_http_error(status, body));
            }

            let video_data: serde_json::Value = resp.json().await.map_err(|e| {
                PlatformError::Network(format!("Failed to parse video response: {}", e))
            })?;

            video_data["items"]
                .as_array()
                .and_then(|items| items.first())
                .and_then(|item| item["liveStreamingDetails"]["activeLiveChatId"].as_str())
                // API-key mode is read-only — no outbound messages to
                // self-suppress, so the owner id is irrelevant here.
                .map(|s| (s.to_string(), None))
                .ok_or_else(|| {
                    PlatformError::Platform(
                        "Live stream found but no active chat. Chat may be disabled for this stream.".to_string(),
                    )
                })
        }
    }
}
