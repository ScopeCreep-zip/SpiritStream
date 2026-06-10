//! `spiritstream-cli safety …` — safety features for vulnerable users.
//!
//! `safety panic` runs the full disconnect flow in-process
//! (no HTTP shell-out). `safety blocklist` manages the PII blocklist.

use clap::Subcommand;
use serde::Serialize;

use crate::error::CliError;
use crate::output::Output;
use spiritstream_core::ServiceRegistry;

#[derive(Debug, Subcommand)]
pub enum SafetyCmd {
    /// Trigger panic disconnect: stop every stream, disconnect chat + OBS,
    /// wipe in-memory secret caches, append an audit-log entry.
    Panic,
    /// Manage the per-profile PII blocklist. Operates on the
    /// profile named by `--profile`. Password-protected profiles need
    /// `--password` to load and re-save.
    Blocklist {
        /// Profile to operate on.
        #[arg(long)]
        profile: String,
        /// Password for encrypted profiles. Ignored for plaintext.
        #[arg(long)]
        password: Option<String>,
        #[command(subcommand)]
        action: BlocklistAction,
    },
}

#[derive(Debug, Subcommand)]
pub enum BlocklistAction {
    /// List current blocklist phrases.
    List,
    /// Add a phrase. Match is case-insensitive substring.
    Add { phrase: String },
    /// Remove a phrase.
    Remove { phrase: String },
}

#[derive(Serialize)]
struct SafetyPanicCliResponse {
    streams_stopped: usize,
    elapsed_ms: u64,
    /// FFmpeg processes from OTHER SpiritStream processes (a running
    /// desktop server, a crashed prior run) killed via the cross-process
    /// registry. The in-process counter above can be 0 while this is
    /// non-zero — exactly the case the old CLI panic silently no-op'd.
    orphans_killed: usize,
    /// Registry records dropped because the pid was dead or no longer
    /// an FFmpeg process (never killed).
    orphans_stale: usize,
}

#[derive(Serialize)]
struct BlocklistResponse {
    profile: String,
    phrases: Vec<String>,
}

pub async fn run(
    cmd: SafetyCmd,
    registry: &ServiceRegistry,
    out: &mut Output,
) -> Result<(), CliError> {
    match cmd {
        SafetyCmd::Panic => {
            // In-process flow first: stops anything THIS process started,
            // disconnects chat/OBS, purges secret caches, records audit.
            let result = registry.safety.panic().await?;

            // Then reach across processes: kill FFmpeg children recorded
            // by any other SpiritStream process (a running desktop
            // server, a crashed prior CLI run). The CLI is one-shot —
            // without this, `safety panic` reported success while the
            // user's stream kept broadcasting.
            let run_dir = registry.data_dir.join("run");
            let orphans = tokio::task::spawn_blocking(move || {
                spiritstream_core::services::ffmpeg_handler::process_registry::kill_recorded_processes(&run_dir)
            })
            .await
            .map_err(|e| CliError::Io(format!("orphan-kill join: {e}")))??;
            if orphans.killed > 0 || orphans.stale > 0 {
                if let Err(e) = registry.audit.record(
                    spiritstream_core::services::AuditAction::PanicKilledOrphans {
                        killed: orphans.killed,
                        stale: orphans.stale,
                    },
                ) {
                    log::error!("failed to append PanicKilledOrphans audit entry: {e}");
                }
            }

            out.emit(&SafetyPanicCliResponse {
                streams_stopped: result.streams_stopped,
                elapsed_ms: result.elapsed_ms,
                orphans_killed: orphans.killed,
                orphans_stale: orphans.stale,
            })?;
            Ok(())
        }
        SafetyCmd::Blocklist {
            profile,
            password,
            action,
        } => {
            let mut p = registry
                .profiles
                .load_with_key_decryption(&profile, password.as_deref())
                .await?;
            match action {
                BlocklistAction::List => {}
                BlocklistAction::Add { phrase } => {
                    let trimmed = phrase.trim().to_string();
                    if !trimmed.is_empty() && !p.pii_blocklist.iter().any(|x| x == &trimmed) {
                        p.pii_blocklist.push(trimmed);
                        registry
                            .profiles
                            .save_with_key_encryption(&p, password.as_deref())
                            .await?;
                    }
                }
                BlocklistAction::Remove { phrase } => {
                    let before = p.pii_blocklist.len();
                    p.pii_blocklist.retain(|x| x != &phrase);
                    if p.pii_blocklist.len() != before {
                        registry
                            .profiles
                            .save_with_key_encryption(&p, password.as_deref())
                            .await?;
                    }
                }
            }
            out.emit(&BlocklistResponse {
                profile: p.name.clone(),
                phrases: p.pii_blocklist,
            })?;
            Ok(())
        }
    }
}
