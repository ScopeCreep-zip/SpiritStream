//! Media pipeline abstraction.
//!
//! Desktop / server / Docker use `FfmpegProcessor` (wraps the existing
//! `services::ffmpeg_handler` code). Tauri 2 mobile supplies a
//! `PlatformMediaProcessor` impl that delegates to AVFoundation (iOS) or
//! MediaCodec (Android) since FFmpeg is not viable on mobile.
//!
//! Stub for now — the real trait surface is finalized alongside
//! the ts-rs-derived models. Listing here so service code knows the contract
//! exists.

use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::{traits::EventSink, CoreError};

/// Opaque handle returned by `start_stream`, passed to `stop_stream`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamHandle {
    pub group_id: String,
    pub pid: Option<u32>,
}

#[async_trait]
pub trait MediaProcessor: Send + Sync {
    async fn start_stream(
        &self,
        group_id: String,
        incoming_url: String,
        events: Arc<dyn EventSink>,
    ) -> Result<StreamHandle, CoreError>;

    async fn stop_stream(&self, handle: &StreamHandle) -> Result<(), CoreError>;

    async fn list_encoders(&self) -> Result<serde_json::Value, CoreError>;
}
