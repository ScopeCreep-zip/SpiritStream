//! `spiritstream-cli audit …` — read the local HMAC-chained audit log.

use clap::Subcommand;

use crate::error::CliError;
use crate::output::Output;
use spiritstream_core::ServiceRegistry;

#[derive(Debug, Subcommand)]
pub enum AuditCmd {
    /// Verify the HMAC chain (live log + the pre-migration archive,
    /// when present) and report the tail-anchor state. Exits non-zero
    /// when either chain is tampered.
    Verify,
    /// Print recent audit entries as NDJSON.
    Log {
        /// Earliest entry to include (ISO 8601 timestamp). Entries
        /// with a `timestamp` strictly older are dropped from the output.
        #[arg(long)]
        since: Option<String>,
        /// Only emit entries whose `action.kind` matches this string.
        #[arg(long)]
        filter: Option<String>,
        /// Only emit entries whose `action.platform` matches this string.
        /// Combine with `--filter ChatPlatformConnected` to reconstruct
        /// connect history for a specific platform — e.g. operators
        /// auditing whether Facebook was ever enabled run
        /// `audit log --filter ChatPlatformConnected --platform facebook`.
        #[arg(long)]
        platform: Option<String>,
        /// Maximum entries to print (most-recent first). Default 200.
        #[arg(long, default_value_t = 200)]
        limit: usize,
    },
}

pub async fn run(
    cmd: AuditCmd,
    registry: &ServiceRegistry,
    out: &mut Output,
) -> Result<(), CliError> {
    match cmd {
        AuditCmd::Verify => {
            let chain = registry.audit.verify_chain()?;
            let archive = registry.audit.verify_archive()?;
            let anchor_state = registry.audit.anchor_state();
            #[derive(serde::Serialize)]
            #[serde(rename_all = "camelCase")]
            struct VerifyResponse {
                chain: spiritstream_core::services::AuditChainStatus,
                archive: spiritstream_core::services::AuditChainStatus,
                anchor_state: String,
            }
            let tampered = matches!(
                chain,
                spiritstream_core::services::AuditChainStatus::Tampered { .. }
            ) || matches!(
                archive,
                spiritstream_core::services::AuditChainStatus::Tampered { .. }
            );
            out.emit(&VerifyResponse {
                chain,
                archive,
                anchor_state: anchor_state.to_string(),
            })?;
            if tampered {
                return Err(CliError::Unavailable(
                    "audit chain verification failed — see JSON output".into(),
                ));
            }
            Ok(())
        }
        AuditCmd::Log {
            since,
            filter,
            platform,
            limit,
        } => {
            let entries = registry.audit.entries()?;
            let since_ts = since
                .as_deref()
                .map(chrono::DateTime::parse_from_rfc3339)
                .transpose()
                .map_err(|e| CliError::Io(format!("invalid --since timestamp: {e}")))?
                .map(|dt| dt.with_timezone(&chrono::Utc));
            let filtered: Vec<_> = entries
                .into_iter()
                .rev()
                .filter(|e| {
                    if let Some(ts) = since_ts {
                        if e.timestamp < ts {
                            return false;
                        }
                    }
                    let action_json = serde_json::to_value(&e.action).ok();
                    if let Some(ref f) = filter {
                        let kind = action_json
                            .as_ref()
                            .and_then(|v| v.get("kind").and_then(|k| k.as_str()).map(String::from))
                            .unwrap_or_default();
                        if kind != *f {
                            return false;
                        }
                    }
                    if let Some(ref p) = platform {
                        let entry_platform = action_json
                            .as_ref()
                            .and_then(|v| {
                                v.get("platform").and_then(|x| x.as_str()).map(String::from)
                            })
                            .unwrap_or_default();
                        if entry_platform != *p {
                            return false;
                        }
                    }
                    true
                })
                .take(limit)
                .collect();
            out.emit(&filtered)?;
            Ok(())
        }
    }
}
