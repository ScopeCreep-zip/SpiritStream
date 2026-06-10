//! Typed payloads for the stream lifecycle events the FFmpeg handler
//! emits (`stream_error`, `stream_retry_attempt`,
//! `stream_retry_exhausted`). Previously ad-hoc `serde_json::json!`
//! blobs that the frontend mirrored by hand — these ts-rs exports close
//! that typed-contract hole.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Emitted when an FFmpeg group process exits unexpectedly.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/types/src/generated/")]
pub struct StreamErrorEvent {
    pub group_id: String,
    pub error: String,
    pub can_retry: bool,
    pub suggestion: String,
}

/// Emitted before each automatic reconnection attempt.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/types/src/generated/")]
pub struct StreamRetryAttemptEvent {
    pub group_id: String,
    pub attempt: u32,
    pub max_attempts: u32,
    pub delay_secs: u64,
}

/// Emitted exactly once when a group exhausts its retry budget —
/// terminal state; the UI stops showing the group as reconnecting.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/types/src/generated/")]
pub struct StreamRetryExhaustedEvent {
    pub group_id: String,
    pub max_attempts: u32,
}
