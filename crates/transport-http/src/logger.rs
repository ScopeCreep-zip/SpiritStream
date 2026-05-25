use std::fs::OpenOptions;
use std::io::Write;
use std::sync::Mutex;

use chrono::Local;
use log::{Level, LevelFilter, Log, Metadata, Record};
use serde_json::json;

use crate::events::EventBus;
use crate::redaction::mask_sensitive;
use spiritstream_core::services::EventSink;

pub(crate) struct ServerLogger {
    file: Mutex<std::fs::File>,
    event_bus: EventBus,
    level: LevelFilter,
}

impl ServerLogger {
    pub(crate) fn new(
        log_dir: &std::path::Path,
        event_bus: EventBus,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let log_path = log_dir.join("spiritstream-server.log");
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_path)?;
        Ok(Self {
            file: Mutex::new(file),
            event_bus,
            level: LevelFilter::Info,
        })
    }
}

impl Log for ServerLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= self.level
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }

        let timestamp = Local::now();
        let target = record.target();
        let level = record.level();
        let raw_message = format!("{}", record.args());
        // Every log line passes through `mask_sensitive`
        // before it hits disk so token-shaped strings, RTMP keys, and
        // ${TOKEN} expansions can't leak via the log file. Enforced at
        // the boundary, not per call site.
        let message = mask_sensitive(&raw_message);

        // Structured JSON output when SPIRITSTREAM_LOG_FORMAT=json.
        // Default stays bracketed-text for human-readable tail-following
        // during development. Both formats apply redaction identically.
        let json_format = std::env::var("SPIRITSTREAM_LOG_FORMAT")
            .map(|v| v.eq_ignore_ascii_case("json"))
            .unwrap_or(false);
        let line = if json_format {
            // Hand-roll JSON so we don't take a serde dep for an
            // already-load-bearing fast path. Field order is stable
            // for downstream log shippers.
            format!(
                r#"{{"ts":"{}","level":"{}","target":"{}","msg":{}}}"#,
                timestamp.to_rfc3339(),
                level,
                target.replace('"', "\\\""),
                serde_json::to_string(&message).unwrap_or_else(|_| "\"\"".into()),
            )
        } else {
            let date = timestamp.format("%Y-%m-%d");
            let time = timestamp.format("%H:%M:%S");
            format!("[{date}][{time}][{target}][{level}] {message}")
        };

        if let Ok(mut file) = self.file.try_lock() {
            if let Err(e) = writeln!(file, "{line}") {
                eprintln!("Failed to write log: {e}");
            }
            // Flush after every write to ensure logs persist on crash.
            if let Err(e) = file.flush() {
                eprintln!("Failed to flush log: {e}");
            }
        }

        let level_number = match level {
            Level::Error => 1,
            Level::Warn => 2,
            Level::Info => 3,
            Level::Debug => 4,
            Level::Trace => 5,
        };

        self.event_bus.emit(
            "log://log",
            json!({ "level": level_number, "message": message, "target": target }),
        );
    }

    fn flush(&self) {
        if let Ok(mut file) = self.file.try_lock() {
            let _ = file.flush();
        }
    }
}

pub(crate) fn init_logger(
    log_dir: &std::path::Path,
    event_bus: EventBus,
) -> Result<(), Box<dyn std::error::Error>> {
    let logger = ServerLogger::new(log_dir, event_bus)?;
    log::set_boxed_logger(Box::new(logger))?;
    log::set_max_level(LevelFilter::Info);
    Ok(())
}
