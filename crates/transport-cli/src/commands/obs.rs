//! `spiritstream-cli obs …` — OBS Studio WebSocket integration (runtime only).
//!
//! Connection lifecycle + stream control. OBS *settings* are owned by the active
//! profile (the single source of truth): edit them with
//! `profile set <name> --set obs.host=… --set obs.autoConnect=…` and read them
//! with `profile show <name>`. There is no `obs config` / `obs set-config` —
//! those mutated an ephemeral per-invocation handler that never persisted.

use clap::Subcommand;
use spiritstream_core::ServiceRegistry;

use crate::error::CliError;
use crate::output::Output;

#[derive(Debug, Subcommand)]
pub enum ObsCmd {
    /// Snapshot of OBS connection + streaming state.
    State,
    /// Connect to the OBS WebSocket using the active profile's settings.
    Connect,
    /// Disconnect from OBS.
    Disconnect,
    /// Start streaming in OBS (requires an active connection).
    StreamStart,
    /// Stop streaming in OBS.
    StreamStop,
}

pub async fn run(
    cmd: ObsCmd,
    registry: &ServiceRegistry,
    out: &mut Output,
) -> Result<(), CliError> {
    match cmd {
        ObsCmd::State => {
            let state = registry.obs.get_state().await;
            out.emit(&state)?;
            Ok(())
        }
        ObsCmd::Connect => {
            registry.obs.connect(registry.events.clone()).await?;
            out.emit(&serde_json::json!({ "connected": true }))?;
            Ok(())
        }
        ObsCmd::Disconnect => {
            registry.obs.disconnect(registry.events.clone()).await?;
            out.emit(&serde_json::json!({ "disconnected": true }))?;
            Ok(())
        }
        ObsCmd::StreamStart => {
            registry.obs.start_stream().await?;
            out.emit(&serde_json::json!({ "started": true }))?;
            Ok(())
        }
        ObsCmd::StreamStop => {
            registry.obs.stop_stream().await?;
            out.emit(&serde_json::json!({ "stopped": true }))?;
            Ok(())
        }
    }
}
