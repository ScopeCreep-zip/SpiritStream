//! `spiritstream-server` binary entrypoint.
//!
//! All HTTP transport logic lives in `spiritstream-transport-http`. This
//! binary instantiates `HttpTransport` and asks it to `serve`. Future
//! transports (CLI, Veilid) follow the same pattern.

use std::sync::Arc;

use spiritstream_core::traits::Transport;
use spiritstream_transport_http::HttpTransport;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let transport = Arc::new(HttpTransport::new());
    transport.serve().await?;
    Ok(())
}
