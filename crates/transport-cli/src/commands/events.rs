//! `spiritstream-cli events watch` — stream service events to stdout as JSON
//! Lines (NDJSON), one JSON object per line.
//!
//! Subscribes via a stdout `EventSink` that the registry already holds. The
//! command blocks until the user interrupts (Ctrl+C). For tests, `--for-ms`
//! exits after a fixed duration so the suite stays deterministic.
//!
//! Only events whose `event` field starts with one of the `--filter`
//! prefixes are emitted; the default is no filter (every event).

use std::sync::Arc;
use std::time::Duration;

use clap::Subcommand;
use serde_json::Value;
use spiritstream_core::traits::EventSink;
use spiritstream_core::{ServiceRegistry, ServiceRegistryOptions};

use crate::error::CliError;
use crate::output::Output;

#[derive(Debug, Subcommand)]
pub enum EventsCmd {
    /// Block and print events as NDJSON until interrupted (or `--for-ms`
    /// elapses).
    Watch {
        /// Prefix filter. Comma-separated or repeated:
        ///   `--filter stream_stats,chat_message`
        ///   `--filter stream_stats --filter chat_message`
        #[arg(long = "filter", value_delimiter = ',')]
        filter: Vec<String>,
        /// Exit after this many milliseconds (test plumbing). Defaults to
        /// blocking forever.
        #[arg(long = "for-ms")]
        for_ms: Option<u64>,
    },
}

pub async fn run(
    cmd: EventsCmd,
    registry: &ServiceRegistry,
    _out: &mut Output,
) -> Result<(), CliError> {
    match cmd {
        EventsCmd::Watch { filter, for_ms } => watch(registry, filter, for_ms).await,
    }
}

async fn watch(
    registry: &ServiceRegistry,
    filters: Vec<String>,
    for_ms: Option<u64>,
) -> Result<(), CliError> {
    // Build a *new* registry that wires a `StdoutEventSink` instead of
    // `NoopEventSink`, against the SAME paths the outer registry resolved
    // (`--data-dir` / `SPIRITSTREAM_DATA_DIR` / `--themes-dir` included).
    // The previous implementation rebuilt from the platform default — a
    // `--data-dir`-isolated invocation silently watched (and created
    // services against) the user's real install.
    let data_dir = registry.data_dir.clone();
    let log_dir = registry.log_dir.clone();
    let themes_dir = registry.themes_dir.clone();
    let events: Arc<dyn EventSink> = Arc::new(StdoutEventSink::new(filters));
    let override_kind = std::env::var("SPIRITSTREAM_SECRET_STORE").ok();
    let secret_store =
        spiritstream_core::services::build_secret_store(&data_dir, override_kind.as_deref());
    let _registry = spiritstream_core::ServiceRegistry::build(ServiceRegistryOptions {
        data_dir,
        themes_dir,
        log_dir,
        custom_ffmpeg_path: None,
        events,
        secret_store,
    })
    .map_err(CliError::from)?;

    // Sit and let background tasks (theme watcher, etc.) emit. The
    // StdoutEventSink does its own stdout writes; we don't loop here on a
    // channel because the EventSink trait doesn't expose subscription.
    match for_ms {
        Some(ms) => tokio::time::sleep(Duration::from_millis(ms)).await,
        None => {
            let _ = tokio::signal::ctrl_c().await;
        }
    }
    // Suppress the standard one-document stdout emit — every event is a
    // separate line.
    Ok(())
}

/// `EventSink` that writes each event as a single NDJSON line to stdout.
struct StdoutEventSink {
    filters: Vec<String>,
}

impl StdoutEventSink {
    fn new(filters: Vec<String>) -> Self {
        Self { filters }
    }

    fn passes_filter(&self, event: &str) -> bool {
        self.filters.is_empty() || self.filters.iter().any(|f| event.starts_with(f))
    }
}

impl EventSink for StdoutEventSink {
    fn emit(&self, event: &str, payload: Value) {
        if !self.passes_filter(event) {
            return;
        }
        let line = serde_json::json!({ "event": event, "payload": payload });
        if let Ok(s) = serde_json::to_string(&line) {
            println!("{s}");
        }
    }
}
