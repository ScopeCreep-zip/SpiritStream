//! Health / ready / openapi / request-id middleware / system metadata
//! endpoint coverage. Plus the "legacy paths must 404" sweep.

use super::common::{boot, get, post_invoke};
use serde_json::Value;

#[test]
fn versioned_health_responds_ok() {
    let server = boot();
    let (status, body) = get(&server, "/api/v1/health");
    assert_eq!(status, 200, "/api/v1/health failed: {body}");
    let json: Value = serde_json::from_str(&body).expect("/api/v1/health emits JSON");
    assert_eq!(json["status"], Value::String("ok".into()));
}

#[test]
fn health_reports_per_service_status() {
    let server = boot();
    let (status, body) = get(&server, "/api/v1/health");
    assert_eq!(status, 200, "/api/v1/health failed: {body}");
    let json: Value = serde_json::from_str(&body).unwrap();
    let services = json["services"].as_object().expect("services map");
    for key in ["profiles", "settings", "themes", "audit_log"] {
        assert!(
            services.contains_key(key),
            "missing service: {key} ({body})"
        );
        let entry = services.get(key).unwrap();
        assert!(entry.is_object(), "service {key} must be a tagged object");
        assert!(
            entry.get("state").is_some(),
            "service {key} must carry a `state` discriminator (got {entry})",
        );
    }
}

#[test]
fn request_id_middleware_echoes_provided_value() {
    let server = boot();
    let resp = reqwest::blocking::Client::new()
        .get(format!("{}/api/v1/health", server.base))
        .header("X-Request-Id", "test-correlation-1234")
        .send()
        .expect("GET");
    assert_eq!(resp.status().as_u16(), 200);
    let echoed = resp
        .headers()
        .get("x-request-id")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("");
    assert_eq!(echoed, "test-correlation-1234");
}

#[test]
fn request_id_middleware_generates_uuid_when_absent() {
    let server = boot();
    let resp = reqwest::blocking::Client::new()
        .get(format!("{}/api/v1/health", server.base))
        .send()
        .expect("GET");
    let request_id = resp
        .headers()
        .get("x-request-id")
        .and_then(|h| h.to_str().ok())
        .expect("server must always emit X-Request-Id")
        .to_string();
    // UUID v7 string form is 36 chars (8-4-4-4-12 + dashes).
    assert_eq!(request_id.len(), 36, "expected UUID, got {request_id:?}");
    assert_eq!(request_id.matches('-').count(), 4);
}

#[test]
fn audit_log_response_includes_chain_status() {
    let server = boot();
    let (status, body) = get(&server, "/api/v1/audit/log");
    assert_eq!(status, 200, "audit log failed: {body}");
    let json: Value = serde_json::from_str(&body).unwrap();
    let chain = &json["chain"];
    // `AuditChainStatusWire` is `#[serde(tag = "state")]` — every
    // variant serialises as `{"state": "<variant>", ...}`. Bare-string
    // serialisation was the pre-G6 free-form `serde_json::Value`
    // shape; the typed DTO is the contract now.
    let state = chain["state"]
        .as_str()
        .unwrap_or_else(|| panic!("chain.state should be a string: {chain}"));
    assert!(
        matches!(state, "empty" | "ok" | "tampered"),
        "unrecognised chain state: {chain}",
    );
}

#[test]
fn versioned_ready_responds_ok() {
    let server = boot();
    let (status, body) = get(&server, "/api/v1/ready");
    assert_eq!(status, 200, "/api/v1/ready failed: {body}");
    let json: Value = serde_json::from_str(&body).expect("/api/v1/ready emits JSON");
    assert_eq!(
        json["ready"],
        Value::Bool(true),
        "expected ready: true, got {json}"
    );
}

#[test]
fn openapi_document_is_well_formed_and_lists_typed_paths() {
    let server = boot();
    let (status, body) = get(&server, "/api/v1/openapi.json");
    assert_eq!(status, 200, "/api/v1/openapi.json failed: {body}");
    let doc: Value = serde_json::from_str(&body).expect("openapi must parse");

    let version = doc["openapi"].as_str().expect("openapi version string");
    assert!(
        version.starts_with("3.0") || version.starts_with("3.1"),
        "unexpected openapi version: {version}"
    );

    let paths = doc["paths"].as_object().expect("paths object");
    for path in ["/health", "/ready", "/profiles"] {
        assert!(
            paths.contains_key(path),
            "path missing from openapi doc: {path}"
        );
    }
}

#[test]
fn versioned_invoke_dispatch_bridge_returns_404() {
    // The rewrite retired the public bridge. The frontend now uses typed REST
    // URLs for every operation; the dispatch route should no longer exist.
    let server = boot();
    let (status, _) = post_invoke(&server, "/api/v1/invoke/get_all_profiles", "{}");
    assert_eq!(
        status, 404,
        "the /api/v1/invoke bridge should no longer be mounted"
    );
}

#[test]
fn system_encoder_presets_returns_all_codec_lists() {
    let server = boot();
    let (status, body) = get(&server, "/api/v1/system/encoders/presets");
    assert_eq!(status, 200);
    let json: Value = serde_json::from_str(&body).unwrap();
    let presets = json["presets"].as_object().expect("presets object");
    assert!(presets.contains_key("libx264"));
    assert!(presets.contains_key("nvenc"));
    assert!(presets.contains_key("amf"));
    assert!(presets["libx264"].as_array().unwrap().len() >= 5);
    assert!(json["fpsValues"].as_array().unwrap().len() >= 3);
}

#[test]
fn system_client_config_includes_chat_limits() {
    let server = boot();
    let (status, body) = get(&server, "/api/v1/system/client-config");
    assert_eq!(status, 200);
    let json: Value = serde_json::from_str(&body).unwrap();
    assert!(json["chatPopupWidth"].as_u64().unwrap_or(0) > 0);
    let chat_max = json["chatMaxChars"].as_object().expect("chatMaxChars");
    assert_eq!(chat_max["twitch"], 500);
    assert_eq!(chat_max["youtube"], 200);
    assert_eq!(chat_max["tiktok"], 150);
}

/// Every previously-unversioned path must now 404 — there are no legacy
/// aliases. If this test ever fails, a legacy path is leaking back into the
/// router and should be hunted down and removed.
#[test]
fn legacy_paths_return_404() {
    let server = boot();
    for path in [
        "/health",
        "/ready",
        "/auth/login",
        "/auth/logout",
        "/auth/check",
        "/ws",
        "/api/files/browse",
        "/api/files/home",
        "/api/invoke/get_all_profiles",
        "/api/v1/invoke/get_all_profiles",
    ] {
        let (status, _) = get(&server, path);
        assert_eq!(
            status, 404,
            "legacy path {path} unexpectedly responded {status}"
        );
    }
}
