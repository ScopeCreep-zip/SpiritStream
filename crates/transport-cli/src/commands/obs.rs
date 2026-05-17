//! `spiritstream-cli obs …` — OBS Studio WebSocket integration.

use clap::Subcommand;
use spiritstream_core::ServiceRegistry;

use crate::error::CliError;
use crate::output::Output;

#[derive(Debug, Subcommand)]
pub enum ObsCmd {
    /// Snapshot of OBS connection + streaming state.
    State,
    /// Show the current OBS WebSocket config (password masked).
    Config,
    /// Connect to the configured OBS WebSocket.
    Connect,
    /// Disconnect from OBS.
    Disconnect,
    /// Start streaming in OBS (requires an active connection).
    StreamStart,
    /// Stop streaming in OBS.
    StreamStop,
    /// Replace the OBS WebSocket configuration. `--password` is encrypted
    /// before persisting; omit to keep the existing password.
    SetConfig {
        #[arg(long)]
        host: String,
        #[arg(long)]
        port: u16,
        #[arg(long)]
        password: Option<String>,
        #[arg(long, default_value_t = false)]
        use_auth: bool,
        /// One of `obs-to-spiritstream`, `spiritstream-to-obs`, `bidirectional`, `disabled`.
        #[arg(long, default_value = "disabled")]
        direction: String,
        #[arg(long, default_value_t = false)]
        auto_connect: bool,
    },
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
        ObsCmd::Config => {
            let config = registry.obs.get_config().await;
            // Password is masked at the model layer — `ObsConfig` ts-rs export
            // already strips it for the wire shape.
            out.emit(&config)?;
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
        ObsCmd::SetConfig {
            host,
            port,
            password,
            use_auth,
            direction,
            auto_connect,
        } => {
            use spiritstream_core::services::IntegrationDirection;
            let current = registry.obs.get_config().await;
            let encrypted = if let Some(p) = password.as_deref() {
                if p.is_empty() {
                    String::new()
                } else {
                    registry.obs.encrypt_password(p)?
                }
            } else {
                current.password
            };
            let dir = match direction.as_str() {
                "obs-to-spiritstream" => IntegrationDirection::ObsToSpiritstream,
                "spiritstream-to-obs" => IntegrationDirection::SpiritstreamToObs,
                "bidirectional" => IntegrationDirection::Bidirectional,
                _ => IntegrationDirection::Disabled,
            };
            let config = spiritstream_core::services::ObsConfig {
                host,
                port,
                password: encrypted,
                use_auth,
                direction: dir,
                auto_connect,
            };
            registry.obs.set_config(config).await;
            out.emit(&serde_json::json!({ "saved": true }))?;
            Ok(())
        }
    }
}
