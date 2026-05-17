//! `spiritstream-cli audit …` — read the local HMAC-chained audit log.

use clap::Subcommand;

use crate::error::CliError;
use crate::output::Output;
use spiritstream_core::ServiceRegistry;

#[derive(Debug, Subcommand)]
pub enum AuditCmd {
    /// Print recent audit entries as NDJSON.
    Log {
        /// Earliest entry to include (ISO 8601 timestamp). Entries
        /// with a `timestamp` strictly older are dropped from the output.
        #[arg(long)]
        since: Option<String>,
        /// Only emit entries whose `action.kind` matches this string.
        #[arg(long)]
        filter: Option<String>,
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
        AuditCmd::Log {
            since,
            filter,
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
                    if let Some(ref f) = filter {
                        let kind = serde_json::to_value(&e.action)
                            .ok()
                            .and_then(|v| v.get("kind").and_then(|k| k.as_str()).map(String::from))
                            .unwrap_or_default();
                        if kind != *f {
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
