//! OBS Studio WebSocket integration.
//!
//! Split by concern:
//! - `types`: ObsConfig / ObsState / ObsConnectionStatus /
//!   ObsStreamStatus / IntegrationDirection wire shapes (ts-rs-exported).
//! - `handler`: ObsWebSocketHandler struct + lightweight accessors +
//!   loop-prevention flag.
//! - `connection`: connect / disconnect / spawn_auto_connect + password
//!   encryption surface for the transport.
//! - `commands`: start_stream / stop_stream + the ObsTrigger trait impl.
//! - `cascade`: OBS→SpiritStream and SpiritStream→OBS trigger cascade;
//!   the polling event listener; `ObsCascadeDeps`.

mod cascade;
mod commands;
mod connection;
mod handler;
mod types;

#[cfg(test)]
mod tests;

pub use cascade::ObsCascadeDeps;
pub use handler::ObsWebSocketHandler;
pub use types::{
    IntegrationDirection, ObsConfig, ObsConnectionStatus, ObsState, ObsStreamStatus,
};

use crate::errors::{CoreError, ValidationIssue};

pub(super) fn obs_not_connected() -> CoreError {
    CoreError::ValidationFailed {
        reasons: vec![ValidationIssue {
            code: "obs_not_connected".into(),
            message: "Not connected to OBS".into(),
            path: None,
        }],
    }
}
