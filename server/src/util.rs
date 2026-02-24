use chrono::{DateTime, Duration, Local, Timelike};
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::net::{IpAddr, Ipv4Addr};

pub(crate) fn get_arg<T: DeserializeOwned>(payload: &Value, key: &str) -> Result<T, String> {
    let obj = payload
        .as_object()
        .ok_or_else(|| "Invalid payload".to_string())?;
    let value = obj
        .get(key)
        .ok_or_else(|| format!("Missing argument: {key}"))?;
    serde_json::from_value(value.clone()).map_err(|e| format!("Invalid {key}: {e}"))
}

pub(crate) fn get_opt_arg<T: DeserializeOwned>(payload: &Value, key: &str) -> Result<Option<T>, String> {
    let obj = payload
        .as_object()
        .ok_or_else(|| "Invalid payload".to_string())?;
    let value = match obj.get(key) {
        Some(value) => value.clone(),
        None => return Ok(None),
    };

    if value.is_null() {
        return Ok(None);
    }

    serde_json::from_value(value)
        .map(Some)
        .map_err(|e| format!("Invalid {key}: {e}"))
}

pub(crate) fn build_hour_keys(start: DateTime<Local>, end: DateTime<Local>) -> Vec<String> {
    let mut keys = Vec::new();

    let start_hour = start
        .with_minute(0)
        .and_then(|d| d.with_second(0))
        .and_then(|d| d.with_nanosecond(0))
        .unwrap_or(start);
    let end_hour = end
        .with_minute(0)
        .and_then(|d| d.with_second(0))
        .and_then(|d| d.with_nanosecond(0))
        .unwrap_or(end);

    let mut cursor = start_hour;
    while cursor <= end_hour {
        keys.push(cursor.format("%Y%m%d-%H").to_string());
        cursor += Duration::hours(1);
    }

    keys
}

pub(crate) fn parse_bool(value: &str) -> Option<bool> {
    match value.trim().to_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}

pub(crate) fn parse_host(host: &str) -> IpAddr {
    host.parse().unwrap_or(IpAddr::V4(Ipv4Addr::LOCALHOST))
}

/// Find themes directory by searching common relative paths from CWD.
/// Used as fallback when SPIRITSTREAM_THEMES_DIR is not set or invalid.
pub(crate) fn find_themes_dir_fallback() -> Option<String> {
    let cwd = std::env::current_dir().ok()?;

    let candidates = [
        cwd.join("themes"),
        cwd.join("../themes"),
        cwd.join("../../themes"),
        cwd.join("../../../themes"),
    ];

    for candidate in candidates {
        if let Ok(canonical) = candidate.canonicalize() {
            if canonical.is_dir() {
                // Verify it has theme files
                if std::fs::read_dir(&canonical)
                    .map(|entries| {
                        entries.flatten().any(|e| {
                            e.path()
                                .extension()
                                .map(|ext| ext == "jsonc" || ext == "json")
                                .unwrap_or(false)
                        })
                    })
                    .unwrap_or(false)
                {
                    return Some(canonical.to_string_lossy().to_string());
                }
            }
        }
    }
    None
}
