//! `HttpTransport` — the typed wrapper that satisfies
//! `spiritstream_core::traits::Transport` for the Axum-backed HTTP surface.
//!
//! The bulk of the runtime logic still lives in `pub async fn run()` in
//! `lib.rs`; this struct exists so the `Transport` trait contract is honored
//! (every transport adapter has a concrete `impl Transport`). The
//! `spiritstream-server` binary delegates to `HttpTransport::serve` which
//! delegates to `run`. When a graceful-shutdown channel and ServiceRegistry-
//! based constructor land, the body of `run` moves
//! into `HttpTransport::serve` and `run` shrinks to a single call.

use std::sync::Arc;

use async_trait::async_trait;
use spiritstream_core::{traits::Transport, CoreError};

/// HTTP transport adapter. Today it owns no state of its own; future phases
/// thread a shutdown signal and an externally-built `ServiceRegistry` here.
#[derive(Debug, Default, Clone)]
pub struct HttpTransport;

impl HttpTransport {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Transport for HttpTransport {
    async fn serve(self: Arc<Self>) -> Result<(), CoreError> {
        crate::run().await.map_err(|e| CoreError::Internal {
            context: format!("http transport: {e}"),
        })
    }

    async fn shutdown(&self) {
        // Wire a watch/Notify-based shutdown handle in a future change.
        // For now `serve` runs until the process receives a signal.
    }
}
