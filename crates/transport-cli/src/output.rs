//! Stdout output formatting. JSON by default; `--pretty` for indented JSON.
//!
//! Every command emits **exactly one** JSON document on stdout. Errors go to
//! stderr via env_logger plus a structured JSON document on stdout (so
//! callers can pipe stdout into `jq` regardless of success). The exit code
//! is the authoritative signal.

use serde::Serialize;

use crate::error::CliError;

pub struct Output {
    pretty: bool,
}

impl Output {
    pub fn new(pretty: bool) -> Self {
        Self { pretty }
    }

    /// Emit the success payload as a single JSON document on stdout.
    pub fn emit<T: Serialize>(&self, value: &T) -> Result<(), CliError> {
        let s = if self.pretty {
            serde_json::to_string_pretty(value)
        } else {
            serde_json::to_string(value)
        }
        .map_err(|e| CliError::Serialization(e.to_string()))?;
        println!("{s}");
        Ok(())
    }

    /// Emit an error document on stdout. The exit code carries the real
    /// signal; the JSON body lets script consumers extract details.
    ///
    /// For `CoreError`-backed failures the inner variant's `details` payload
    /// (e.g. `reasons` for `ValidationFailed`, `port`/`owner` for
    /// `PortConflict`) is folded into the body so scripts can branch on a
    /// specific field without parsing the message string.
    pub fn emit_error(&self, err: &CliError) {
        let mut body = serde_json::json!({
            "ok": false,
            "kind": err.kind(),
            "message": err.to_string(),
        });

        if let CliError::Core(core) = err {
            if let Ok(serde_json::Value::Object(map)) = serde_json::to_value(core) {
                if let Some(details) = map.get("details") {
                    body["details"] = details.clone();
                }
            }
        }

        // Best effort — if serialization fails, fall back to the Display impl.
        let rendered = if self.pretty {
            serde_json::to_string_pretty(&body).unwrap_or_else(|_| err.to_string())
        } else {
            serde_json::to_string(&body).unwrap_or_else(|_| err.to_string())
        };
        println!("{rendered}");
    }
}
