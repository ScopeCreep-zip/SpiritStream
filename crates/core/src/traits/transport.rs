//! Transport adapter contract.
//!
//! `HttpTransport`, `CliTransport`, and the future
//! `VeilidTransport` all implement this trait. Each takes
//! the same `ServiceRegistry`-shaped object built from core services and
//! exposes the same operations via its own protocol.
//!
//! Concrete `ServiceRegistry` lands when service constructors are unified.
//! Until then, transports continue to wire services
//! the way `server/src/main.rs` does today.

use std::sync::Arc;

use async_trait::async_trait;

use crate::CoreError;

#[async_trait]
pub trait Transport: Send + Sync + 'static {
    /// Run the transport until it terminates or is shut down. Implementations
    /// are responsible for binding ports / opening pipes / etc.
    async fn serve(self: Arc<Self>) -> Result<(), CoreError>;

    /// Signal the transport to begin a graceful shutdown. Should return
    /// immediately; `serve` should observe the signal and exit.
    async fn shutdown(&self);
}
