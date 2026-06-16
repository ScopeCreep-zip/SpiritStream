//! Typed REST surface for the Profile resource: list / show / put / delete
//! + activate-related lock endpoints. Validation error shapes pin the
//!   `kind` field downstream clients branch on.

use super::common::{boot, delete, get, post_invoke, put_json, sample_profile};
use serde_json::Value;

#[test]
fn versioned_profiles_lists_when_empty() {
    let server = boot();
    let (status, body) = get(&server, "/api/v1/profiles");
    assert_eq!(status, 200, "/api/v1/profiles failed: {body}");
    let json: Value = serde_json::from_str(&body).expect("/api/v1/profiles emits JSON");
    let names = json["names"].as_array().expect("names array");
    assert!(
        names.is_empty(),
        "fresh data dir should have no profiles: {json}"
    );
}

#[test]
fn profile_show_missing_returns_typed_404_not_legacy_500() {
    let server = boot();
    let (status, body) = get(&server, "/api/v1/profiles/doesnotexist");
    assert_eq!(
        status, 404,
        "expected typed 404 from ProfileNotFound: {body}"
    );
    let json: Value = serde_json::from_str(&body).expect("JSON body");
    assert_eq!(
        json["kind"], "profile_not_found",
        "expected typed kind: {json}"
    );
}

#[test]
fn profile_put_then_get_round_trips() {
    let server = boot();
    let body = serde_json::json!({
        "profile": sample_profile("roundtrip", 1935),
        "password": null,
    });
    let (put_status, put_resp) = put_json(&server, "/api/v1/profiles/roundtrip", &body);
    assert_eq!(put_status, 200, "PUT failed: {put_resp}");
    let put_json: Value = serde_json::from_str(&put_resp).unwrap();
    assert_eq!(put_json["saved"], true);
    // The response carries the CANONICAL persisted document — clients
    // adopt it so server-computed fields (input.url) never go stale in
    // memory. Regression: a client that kept its request copy (url: "")
    // later started a stream with an empty ingest URL.
    let canonical_url = put_json["profile"]["input"]["url"]
        .as_str()
        .expect("save response carries the canonical profile");
    assert!(
        canonical_url.starts_with("rtmp://"),
        "input.url must be recomputed server-side, got {canonical_url:?}"
    );

    let (get_status, get_body) = get(&server, "/api/v1/profiles/roundtrip");
    assert_eq!(get_status, 200, "GET failed: {get_body}");
    let shown: Value = serde_json::from_str(&get_body).unwrap();
    assert_eq!(shown["name"], "roundtrip");

    let (del_status, del_body) = delete(&server, "/api/v1/profiles/roundtrip");
    assert_eq!(del_status, 200, "DELETE failed: {del_body}");
    let del_json: Value = serde_json::from_str(&del_body).unwrap();
    assert_eq!(del_json["deleted"], true);
}

#[test]
fn profile_put_allows_duplicate_inactive_input_ports() {
    let server = boot();
    let body1 = serde_json::json!({
        "profile": sample_profile("first", 1935),
        "password": null,
    });
    let (s1, _) = put_json(&server, "/api/v1/profiles/first", &body1);
    assert_eq!(s1, 200);

    let body2 = serde_json::json!({
        "profile": sample_profile("second", 1935),
        "password": null,
    });
    let (s2, body2_resp) = put_json(&server, "/api/v1/profiles/second", &body2);
    assert_eq!(
        s2, 200,
        "inactive profiles may share RTMP input ports; runtime bind remains authoritative: {body2_resp}"
    );
}

#[test]
fn profile_put_rejects_name_path_mismatch_with_typed_400() {
    let server = boot();
    let body = serde_json::json!({
        "profile": sample_profile("body-says-x", 1935),
        "password": null,
    });
    let (status, resp) = put_json(&server, "/api/v1/profiles/url-says-y", &body);
    assert_eq!(status, 400, "expected 400 ValidationFailed: {resp}");
    let json: Value = serde_json::from_str(&resp).unwrap();
    assert_eq!(json["kind"], "validation_failed");
}

#[test]
fn profile_locked_list_starts_empty() {
    let server = boot();
    let (status, body) = get(&server, "/api/v1/profiles/locked");
    assert_eq!(status, 200);
    let json: Value = serde_json::from_str(&body).unwrap();
    assert!(json["unlocked"].as_array().unwrap().is_empty());
}

#[test]
fn profile_lock_endpoint_is_idempotent() {
    let server = boot();
    let body = serde_json::json!({});
    let (status, _) = post_invoke(&server, "/api/v1/profiles/anything/lock", &body.to_string());
    assert_eq!(status, 200);
}
