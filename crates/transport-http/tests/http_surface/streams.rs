//! Typed REST surface for the Stream resource: validation kinds + status.

use super::common::{boot, get, passthrough_validate_body, post_invoke};
use serde_json::Value;

#[test]
fn streams_validate_passthrough_profile_returns_200() {
    let server = boot();
    let body = passthrough_validate_body("ok");
    let (status, resp) = post_invoke(&server, "/api/v1/streams/validate", &body.to_string());
    assert_eq!(status, 200, "valid passthrough should return 200: {resp}");
    let json: Value = serde_json::from_str(&resp).unwrap();
    assert_eq!(json["valid"], true);
}

#[test]
fn streams_validate_empty_target_url_returns_typed_400() {
    let server = boot();
    let mut body = passthrough_validate_body("bad-target");
    body["profile"]["outputGroups"][0]["streamTargets"][0]["url"] = Value::String("".into());
    let (status, resp) = post_invoke(&server, "/api/v1/streams/validate", &body.to_string());
    assert_eq!(status, 400, "empty URL should 400: {resp}");
    let json: Value = serde_json::from_str(&resp).unwrap();
    assert_eq!(json["kind"], "invalid_stream_config");
    let reasons = json["details"]["reasons"]
        .as_array()
        .expect("reasons array");
    assert!(reasons.iter().any(|r| r["code"] == "target_missing_url"));
}

#[test]
fn streams_validate_odd_resolution_returns_typed_400() {
    let server = boot();
    let mut body = passthrough_validate_body("non-passthrough");
    body["profile"]["outputGroups"][0]["video"] = serde_json::json!({
        "codec": "libx264", "width": 1281, "height": 720, "fps": 30,
        "bitrate": "6000k", "preset": null, "profile": null, "keyframeIntervalSeconds": 2,
    });
    body["profile"]["outputGroups"][0]["audio"] = serde_json::json!({
        "codec": "aac", "bitrate": "160k", "channels": 2, "sampleRate": 48000,
    });
    let (status, resp) = post_invoke(&server, "/api/v1/streams/validate", &body.to_string());
    assert_eq!(status, 400);
    let json: Value = serde_json::from_str(&resp).unwrap();
    let reasons = json["details"]["reasons"]
        .as_array()
        .expect("reasons array");
    assert!(reasons.iter().any(|r| r["code"] == "video_resolution_odd"));
}

#[test]
fn streams_status_reports_no_active_groups_on_fresh_install() {
    let server = boot();
    let (status, body) = get(&server, "/api/v1/streams");
    assert_eq!(status, 200);
    let json: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(json["activeCount"], 0);
    let ids = json["activeGroupIds"]
        .as_array()
        .expect("activeGroupIds array");
    assert!(ids.is_empty());
}
