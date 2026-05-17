//! Veilid transport adapter — **contract-validation spike**.
//!
//! # What this crate is
//!
//! A minimal stub that implements `spiritstream_core::traits::Transport`
//! to prove the trait surface holds for a non-HTTP transport. The
//! crate compiles against `crates/core` alone — no Axum, no Tauri,
//! no Veilid SDK. The spike's real deliverable is the **list of
//! blockers** in `BLOCKERS.md` (sibling file): every HTTP-shaped
//! assumption in the existing API that a real Veilid implementation
//! would have to revisit.
//!
//! # What this crate is NOT
//!
//! - Not a working transport. `VeilidTransport::serve` returns
//!   `Err(CoreError::NotImplemented)`. No DHT routing, no keypair
//!   identity, no async-rt-tied loop.
//! - Not a dependency of `server/` or `apps/tauri/`. The HTTP
//!   transport is the only production path. This crate is purely
//!   architectural verification.
//!
//! # Why a stub helps now
//!
//! The core's design promise was "transport-agnostic core". The HTTP
//! transport happened first and `crates/core` may have accidentally
//! grown HTTP-shaped assumptions (cookie-keyed sessions, URL-path
//! routing, request/response request/response semantics). Compiling
//! a non-HTTP `Transport` impl against the same `crates/core` is the
//! cheapest way to surface those assumptions before the real Veilid
//! implementation invests in fixing them.

use std::sync::Arc;

use async_trait::async_trait;
use spiritstream_core::traits::Transport;
use spiritstream_core::CoreError;

/// Stub Veilid transport. Construction succeeds; `serve` does not.
pub struct VeilidTransport;

impl VeilidTransport {
    pub fn new() -> Self {
        Self
    }
}

impl Default for VeilidTransport {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Transport for VeilidTransport {
    async fn serve(self: Arc<Self>) -> Result<(), CoreError> {
        log::warn!(
            "VeilidTransport::serve is a contract-validation spike — no DHT routing. \
             See crates/transport-veilid/BLOCKERS.md for the contract gaps a \
             real implementation must close."
        );
        Err(CoreError::NotImplemented {
            feature: "VeilidTransport::serve — contract-validation spike, no working transport".into(),
        })
    }

    async fn shutdown(&self) {
        // No-op: the spike never started.
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Compile-time proof that `VeilidTransport` implements `Transport`
    /// against `crates/core` alone. The body of this test is
    /// intentionally small; **the value is that this file compiles**.
    /// If `crates/core` ever grows an HTTP-shaped trait method that
    /// can't be satisfied by a Veilid impl, this build fails.
    #[tokio::test]
    async fn veilid_transport_satisfies_the_core_transport_trait() {
        let transport: Arc<dyn Transport> = Arc::new(VeilidTransport::new());
        let err = transport.serve().await.unwrap_err();
        assert!(matches!(err, CoreError::NotImplemented { .. }));
    }
}
