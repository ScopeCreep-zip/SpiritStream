//! Typed REST surface for the global Settings document: get / put with
//! bound checks + the resolved-profiles-path helper endpoint.

use super::common::{boot, get, put_json};
use serde_json::Value;

#[test]
fn settings_get_returns_resolved_defaults_on_fresh_install() {
    let server = boot();
    let (status, body) = get(&server, "/api/v1/settings");
    assert_eq!(status, 200, "/api/v1/settings GET failed: {body}");
    let json: Value = serde_json::from_str(&body).expect("settings JSON");
    assert_eq!(json["logRetentionDays"], 30);
    // `autoDownloadFfmpeg` was retired with Option A — FFmpeg is now
    // bundled at build time on macOS / Windows and via distro dep on
    // Linux; there is no runtime download knob to gate.
    assert!(
        json.get("autoDownloadFfmpeg").is_none(),
        "autoDownloadFfmpeg should be retired, got body: {body}"
    );
}

#[test]
fn settings_put_rejects_log_retention_out_of_range_with_typed_400() {
    let server = boot();
    let mut settings: Value = serde_json::from_str(&get(&server, "/api/v1/settings").1).unwrap();
    settings["logRetentionDays"] = Value::from(0);
    let body = serde_json::json!({ "settings": settings });
    let (status, resp) = put_json(&server, "/api/v1/settings", &body);
    assert_eq!(status, 400, "expected 400 ValidationFailed: {resp}");
    let json: Value = serde_json::from_str(&resp).unwrap();
    assert_eq!(json["kind"], "validation_failed");
    let reasons = json["details"]["reasons"]
        .as_array()
        .expect("reasons array");
    assert!(
        reasons
            .iter()
            .any(|r| r["code"] == "log_retention_days_out_of_range"),
        "missing expected validation issue: {json}"
    );
}

// `backendPort` and `discordCooldownSeconds` moved to `ProfileSettings`.
// Their bound checks now run in `ProfileManager::save_with_key_encryption`,
// and the equivalent HTTP-surface coverage lives under the profile PUT
// endpoint. The Settings-PUT layer only owns `logRetentionDays` after
// Option A retired the global download + connection settings.

#[test]
fn settings_put_rejects_log_retention_below_min_with_typed_400() {
    let server = boot();
    let mut settings: Value = serde_json::from_str(&get(&server, "/api/v1/settings").1).unwrap();
    settings["logRetentionDays"] = Value::from(0);
    let body = serde_json::json!({ "settings": settings });
    let (status, resp) = put_json(&server, "/api/v1/settings", &body);
    assert_eq!(status, 400);
    let json: Value = serde_json::from_str(&resp).unwrap();
    let reasons = json["details"]["reasons"]
        .as_array()
        .expect("reasons array");
    assert!(reasons
        .iter()
        .any(|r| r["code"] == "log_retention_days_out_of_range"));
}

#[test]
fn settings_put_accepts_valid_payload_and_round_trips() {
    let server = boot();
    let mut settings: Value = serde_json::from_str(&get(&server, "/api/v1/settings").1).unwrap();
    settings["logRetentionDays"] = Value::from(60);
    let body = serde_json::json!({ "settings": settings });
    let (status, resp) = put_json(&server, "/api/v1/settings", &body);
    assert_eq!(status, 200, "valid settings PUT failed: {resp}");
    let saved: Value = serde_json::from_str(&resp).unwrap();
    assert_eq!(saved["saved"], true);

    let (status, body) = get(&server, "/api/v1/settings");
    assert_eq!(status, 200);
    let json: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(json["logRetentionDays"], 60, "PUT did not persist: {json}");
}

#[test]
fn settings_profiles_path_returns_absolute_path() {
    let server = boot();
    let (status, body) = get(&server, "/api/v1/settings/profiles-path");
    assert_eq!(status, 200);
    let json: Value = serde_json::from_str(&body).unwrap();
    let path = json["path"].as_str().expect("path string");
    assert!(
        path.ends_with("profiles"),
        "expected path to end with 'profiles': {path}"
    );
    assert!(
        std::path::Path::new(path).is_absolute(),
        "path must be absolute: {path}"
    );
}
