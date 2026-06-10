//! `spiritstream-cli confirm-token …` — one-shot confirmation tokens
//! for destructive operations.
//!
//! Three intents currently gate destructive endpoints / CLI flows:
//! `clear_data`, `rotate_machine_key`, `revoke_all_sessions`. Issuing
//! a token via the CLI lets scripted/automation flows perform the
//! destructive action without an interactive UI step — same token is
//! consumed by the HTTP transport (Q6: registry-shared service so the
//! CLI and HTTP see the same ConfirmTokenService instance).

use clap::Subcommand;
use serde::Serialize;

use crate::error::CliError;
use crate::output::Output;
use spiritstream_core::ServiceRegistry;

#[derive(Debug, Subcommand)]
pub enum ConfirmTokenCmd {
    /// Issue a fresh one-shot token scoped to `--intent`. Token TTL is
    /// 30 seconds — re-issue if your script takes longer than that.
    Issue {
        /// Intent the token authorises. Must match the destructive op
        /// (e.g. `rotate_machine_key`, `clear_data`,
        /// `revoke_all_sessions`, `enable_facebook_chat`). Tokens
        /// issued for one intent are rejected when presented for
        /// another.
        #[arg(long)]
        intent: String,
    },
}

#[derive(Serialize)]
struct IssueResponse {
    intent: String,
    token: String,
    ttl_secs: u64,
}

pub async fn run(
    cmd: ConfirmTokenCmd,
    registry: &ServiceRegistry,
    out: &mut Output,
) -> Result<(), CliError> {
    match cmd {
        ConfirmTokenCmd::Issue { intent } => {
            let token = registry.confirm_tokens.issue(&intent)?;
            out.emit(&IssueResponse {
                intent,
                token,
                ttl_secs: registry.confirm_tokens.ttl().as_secs(),
            })?;
            Ok(())
        }
    }
}
