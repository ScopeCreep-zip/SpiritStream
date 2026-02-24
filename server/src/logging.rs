use log::{Level, LevelFilter, Log, Metadata, Record};
use std::fs::OpenOptions;
use std::io::Write;
use std::sync::Mutex;

use crate::events::EventBus;
use serde_json::json;
use spiritstream_server::services::EventSink;

pub(crate) struct ServerLogger {
    file: Mutex<std::fs::File>,
    event_bus: EventBus,
    level: LevelFilter,
}

impl ServerLogger {
    fn new(log_dir: &std::path::Path, event_bus: EventBus) -> Result<Self, Box<dyn std::error::Error>> {
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

        let timestamp = chrono::Local::now();
        let date = timestamp.format("%Y-%m-%d");
        let time = timestamp.format("%H:%M:%S");
        let target = record.target();
        let level = record.level();
        let message = format!("{}", record.args());
        let line = format!("[{date}][{time}][{target}][{level}] {message}");

        if let Ok(mut file) = self.file.try_lock() {
            if let Err(e) = writeln!(file, "{line}") {
                eprintln!("Failed to write log: {e}");
            }
            // Flush after every write to ensure logs persist on crash
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

pub(crate) fn init_logger(log_dir: &std::path::Path, event_bus: EventBus) -> Result<(), Box<dyn std::error::Error>> {
    let logger = ServerLogger::new(log_dir, event_bus)?;
    log::set_boxed_logger(Box::new(logger))?;
    log::set_max_level(LevelFilter::Info);
    Ok(())
}
