//! `spiritstream-cli session …` — manage HTTP sessions.
//!
//! Sessions live in the cross-process store
//! (`DATA_DIR/run/sessions.json`, hashed values), so `revoke-all` here
//! genuinely logs out every browser/webview session of a RUNNING
//! server: the server revalidates the store on its next request. This
//! is the safety affordance for "someone else may have my session" —
//! usable even when the attacker controls the only logged-in browser.

use clap::Subcommand;
use serde::Serialize;
use spiritstream_core::ServiceRegistry;

use crate::error::CliError;
use crate::output::Output;

#[derive(Debug, Subcommand)]
pub enum SessionCmd {
    /// Print the number of live (unexpired) sessions.
    List,
    /// Revoke every active session. Running servers honor this on their
    /// next request.
    RevokeAll {
        /// Required confirmation flag — this logs out every device.
        #[arg(long)]
        yes: bool,
    },
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SessionListResponse {
    active_sessions: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RevokeAllResponse {
    revoked: usize,
}

pub async fn run(
    cmd: SessionCmd,
    registry: &ServiceRegistry,
    out: &mut Output,
) -> Result<(), CliError> {
    match cmd {
        SessionCmd::List => {
            let active_sessions = registry.sessions.active_count()?;
            out.emit(&SessionListResponse { active_sessions })?;
            Ok(())
        }
        SessionCmd::RevokeAll { yes } => {
            if !yes {
                return Err(CliError::Argument(
                    "revoke-all logs out every device — pass --yes to confirm".into(),
                ));
            }
            let revoked = registry.sessions.revoke_all()?;
            if let Err(e) = registry
                .audit
                .record(spiritstream_core::services::AuditAction::SessionRevoked { count: revoked })
            {
                log::error!("failed to append SessionRevoked audit entry: {e}");
            }
            out.emit(&RevokeAllResponse { revoked })?;
            Ok(())
        }
    }
}
