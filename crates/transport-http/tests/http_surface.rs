//! End-to-end integration test: boot the real `spiritstream-server` binary
//! on a free port against a temp data dir, exercise the `/api/v1/*` surface,
//! and confirm the OpenAPI document is well-formed.
//!
//! These tests pin the HTTP transport's verification criteria from the rewrite plan:
//!   - Every route SpiritStream serves lives under `/api/v1/*` — no legacy
//!     aliases. Unversioned URLs (`/health`, `/ready`, `/auth/*`, `/ws`,
//!     `/api/files/*`, `/api/invoke/*`) return 404. Asserted below.
//!   - The OpenAPI document at `/api/v1/openapi.json` parses and lists every
//!     typed handler the v1 module registers.
//!   - The transitional `POST /api/v1/invoke/:command` dispatch bridge works
//!     until it is retired.
//!
//! `spiritstream-cli` and CLI-driven golden tests under
//! `tests/integration/` at the workspace root are the test substrate
//! for service behaviour. This test stays focused on the HTTP transport.

use std::{
    net::TcpListener,
    path::PathBuf,
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use serde_json::Value;
use tempfile::TempDir;

const READY_TIMEOUT: Duration = Duration::from_secs(15);
const POLL_INTERVAL: Duration = Duration::from_millis(150);

struct ServerHandle {
    child: Child,
    base: String,
    _data_dir: TempDir,
}

impl Drop for ServerHandle {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Locate the built `spiritstream-server` binary by walking up from this
/// test's source dir until we find `target/debug/spiritstream-server`. Cargo
/// builds the workspace binary before running integration tests, so it must
/// exist by the time this code runs.
fn server_binary() -> PathBuf {
    let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    loop {
        let candidate = dir.join("target").join("debug").join(server_binary_name());
        if candidate.exists() {
            return candidate;
        }
        if !dir.pop() {
            panic!(
                "could not find spiritstream-server binary; run `cargo build -p spiritstream-server` first"
            );
        }
    }
}

#[cfg(windows)]
fn server_binary_name() -> &'static str {
    "spiritstream-server.exe"
}

#[cfg(not(windows))]
fn server_binary_name() -> &'static str {
    "spiritstream-server"
}

fn allocate_port() -> u16 {
    // Bind to port 0 to let the OS choose, then close so the server can take it.
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind ephemeral port");
    let port = listener.local_addr().expect("local_addr").port();
    drop(listener);
    port
}

/// Boot the server, retrying with a fresh port if the child fails its
/// readiness probe. Parallel tests race on the close-then-rebind pattern in
/// `allocate_port`; rather than serializing the suite we retry transient
/// failures so adding more tests over time doesn't make the harness
/// brittle.
fn boot() -> ServerHandle {
    boot_inner(None)
}

/// Boot the server with `SPIRITSTREAM_API_TOKEN` configured — used by the
/// brute-force tests which need an enabled auth path.
fn boot_with_token(token: &str) -> ServerHandle {
    boot_inner(Some(token))
}

fn boot_inner(token: Option<&str>) -> ServerHandle {
    const MAX_ATTEMPTS: u32 = 5;
    let mut last_err: Option<String> = None;

    for attempt in 1..=MAX_ATTEMPTS {
        let data_dir = TempDir::new().expect("tempdir");
        let port = allocate_port();
        let base = format!("http://127.0.0.1:{port}");

        let mut cmd = Command::new(server_binary());
        cmd.env("SPIRITSTREAM_HOST", "127.0.0.1")
            .env("SPIRITSTREAM_PORT", port.to_string())
            .env("SPIRITSTREAM_DATA_DIR", data_dir.path())
            .env("SPIRITSTREAM_UI_ENABLED", "0")
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        match token {
            Some(t) => {
                cmd.env("SPIRITSTREAM_API_TOKEN", t);
            }
            None => {
                cmd.env_remove("SPIRITSTREAM_API_TOKEN");
            }
        }
        let child = cmd.spawn().expect("spawn spiritstream-server");

        let mut handle = ServerHandle {
            child,
            base,
            _data_dir: data_dir,
        };
        match try_wait_for_ready(&handle.base) {
            Ok(()) => return handle,
            Err(err) => {
                let _ = handle.child.kill();
                let _ = handle.child.wait();
                last_err = Some(format!("attempt {attempt}: {err}"));
            }
        }
    }
    panic!(
        "server failed to come up after {MAX_ATTEMPTS} attempts: {}",
        last_err.unwrap_or_else(|| "<no error>".into())
    );
}

/// Poll the health endpoint until it responds or the deadline expires.
/// Returns `Err` instead of panicking so `boot()` can retry on a fresh port
/// when a sibling parallel test wins the bind race.
fn try_wait_for_ready(base: &str) -> Result<(), String> {
    let client = reqwest::blocking::Client::new();
    let url = format!("{base}/api/v1/health");
    let deadline = Instant::now() + READY_TIMEOUT;
    let mut last_err: Option<String> = None;
    while Instant::now() < deadline {
        match client.get(&url).timeout(Duration::from_secs(1)).send() {
            Ok(resp) if resp.status().is_success() => return Ok(()),
            Ok(resp) => last_err = Some(format!("status {}", resp.status())),
            Err(e) => last_err = Some(e.to_string()),
        }
        thread::sleep(POLL_INTERVAL);
    }
    Err(last_err.unwrap_or_else(|| "no response".into()))
}

fn get(server: &ServerHandle, path: &str) -> (u16, String) {
    let resp = reqwest::blocking::Client::new()
        .get(format!("{}{}", server.base, path))
        .send()
        .expect("GET");
    let status = resp.status().as_u16();
    let body = resp.text().expect("body");
    (status, body)
}

fn post_invoke(server: &ServerHandle, path: &str, payload: &str) -> (u16, String) {
    let resp = reqwest::blocking::Client::new()
        .post(format!("{}{}", server.base, path))
        .header("content-type", "application/json")
        .body(payload.to_owned())
        .send()
        .expect("POST");
    let status = resp.status().as_u16();
    let body = resp.text().expect("body");
    (status, body)
}

/// POST with a custom set of headers — used by CSRF tests to set
/// `Sec-Fetch-Site` and `Origin`.
fn post_with_headers(
    server: &ServerHandle,
    path: &str,
    payload: &str,
    headers: &[(&str, &str)],
) -> (u16, String) {
    let mut req = reqwest::blocking::Client::new()
        .post(format!("{}{}", server.base, path))
        .header("content-type", "application/json")
        .body(payload.to_owned());
    for (k, v) in headers {
        req = req.header(*k, *v);
    }
    let resp = req.send().expect("POST");
    let status = resp.status().as_u16();
    let body = resp.text().expect("body");
    (status, body)
}

// ---------------------------------------------------------------------------

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
    // /health now returns a typed per-subsystem report.
    // The aggregate `status` field stays for backwards compat.
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
    // When the client sends `X-Request-Id`, the server
    // must echo the same value back so external correlation works.
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
    // `GET /api/v1/audit/log` carries a `chain` field with
    // the HMAC chain status (Empty / Ok / Tampered). A fresh server
    // with no events should report Empty.
    let server = boot();
    let (status, body) = get(&server, "/api/v1/audit/log");
    assert_eq!(status, 200, "audit log failed: {body}");
    let json: Value = serde_json::from_str(&body).unwrap();
    let chain = &json["chain"];
    // `AuditChainStatus` is an externally-tagged serde enum: variants
    // with no payload serialize as the bare lowercase string, payload
    // variants serialize as a single-key object. Both shapes are
    // acceptable; we just confirm a recognised tag.
    let chain_str = chain.as_str();
    let chain_obj_keys: Vec<&str> = chain
        .as_object()
        .map(|m| m.keys().map(String::as_str).collect())
        .unwrap_or_default();
    let recognised = matches!(chain_str, Some("empty") | Some("ok") | Some("tampered"))
        || chain_obj_keys
            .iter()
            .any(|k| matches!(*k, "ok" | "empty" | "tampered"));
    assert!(recognised, "unrecognised chain shape: {chain}");
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
fn openapi_document_is_well_formed_and_lists_typed_paths() {
    let server = boot();
    let (status, body) = get(&server, "/api/v1/openapi.json");
    assert_eq!(status, 200, "/api/v1/openapi.json failed: {body}");
    let doc: Value = serde_json::from_str(&body).expect("openapi must parse");

    // Version line must be present (3.0.x today; 3.1 after the utoipa 5 / axum 0.8 upgrade).
    let version = doc["openapi"].as_str().expect("openapi version string");
    assert!(
        version.starts_with("3.0") || version.starts_with("3.1"),
        "unexpected openapi version: {version}"
    );

    // Every typed path the v1 router exposes must appear in the document.
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
    // the rewrite retired the public bridge. The frontend now uses typed REST
    // URLs for every operation; the dispatch route should no longer exist.
    let server = boot();
    let (status, _) = post_invoke(&server, "/api/v1/invoke/get_all_profiles", "{}");
    assert_eq!(
        status, 404,
        "the /api/v1/invoke bridge should no longer be mounted"
    );
}

// ---------------------------------------------------------------------------
// typed REST handlers for the Profile resource.
// ---------------------------------------------------------------------------

fn put_json(server: &ServerHandle, path: &str, body: &Value) -> (u16, String) {
    let resp = reqwest::blocking::Client::new()
        .put(format!("{}{}", server.base, path))
        .header("content-type", "application/json")
        .body(serde_json::to_string(body).unwrap())
        .send()
        .expect("PUT");
    let status = resp.status().as_u16();
    let body = resp.text().expect("body");
    (status, body)
}

fn delete(server: &ServerHandle, path: &str) -> (u16, String) {
    let resp = reqwest::blocking::Client::new()
        .delete(format!("{}{}", server.base, path))
        .send()
        .expect("DELETE");
    let status = resp.status().as_u16();
    let body = resp.text().expect("body");
    (status, body)
}

fn sample_profile(name: &str, port: u16) -> Value {
    serde_json::json!({
        "id": format!("id-{name}"),
        "name": name,
        "encrypted": false,
        "input": {
            "type": "rtmp",
            "bindAddress": "127.0.0.1",
            "port": port,
            "application": "live"
        },
        "outputGroups": []
    })
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
fn profile_put_rejects_port_conflict_with_typed_409() {
    let server = boot();
    // First profile claims port 1935.
    let body1 = serde_json::json!({
        "profile": sample_profile("first", 1935),
        "password": null,
    });
    let (s1, _) = put_json(&server, "/api/v1/profiles/first", &body1);
    assert_eq!(s1, 200);

    // Second profile attempting same port + bind must conflict.
    let body2 = serde_json::json!({
        "profile": sample_profile("second", 1935),
        "password": null,
    });
    let (s2, body2_resp) = put_json(&server, "/api/v1/profiles/second", &body2);
    assert_eq!(s2, 409, "expected 409 PortConflict: {body2_resp}");
    let json: Value = serde_json::from_str(&body2_resp).unwrap();
    assert_eq!(json["kind"], "port_conflict", "expected typed kind: {json}");
    assert_eq!(json["details"]["port"], 1935);
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

// ---------------------------------------------------------------------------
// system metadata endpoints (encoder presets + client config).
// ---------------------------------------------------------------------------

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
    // Sanity check that each codec gets at least one preset value.
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

// ---------------------------------------------------------------------------
// profile activate / decrypt / lock typed REST.
// ---------------------------------------------------------------------------

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
    // Locking a profile that isn't unlocked is a no-op success.
    let (status, _) = post_invoke(&server, "/api/v1/profiles/anything/lock", &body.to_string());
    assert_eq!(status, 200);
}

// ---------------------------------------------------------------------------
// typed REST handlers for the Stream resource.
// ---------------------------------------------------------------------------

fn passthrough_validate_body(name: &str) -> Value {
    serde_json::json!({
        "profile": {
            "id": format!("id-{name}"),
            "name": name,
            "encrypted": false,
            "input": { "type": "rtmp", "bindAddress": "127.0.0.1", "port": 1935, "application": "live" },
            "outputGroups": [{
                "id": "g1",
                "name": "Group 1",
                "isDefault": true,
                "generatePts": true,
                "video": {
                    "codec": "copy", "width": 0, "height": 0, "fps": 0,
                    "bitrate": "0k", "preset": null, "profile": null,
                    "keyframeIntervalSeconds": null
                },
                "audio": {
                    "codec": "copy", "bitrate": "0k", "channels": 0, "sampleRate": 0
                },
                "container": { "format": "flv" },
                "streamTargets": [{
                    "id": "t1", "name": "Twitch", "service": "Twitch",
                    "url": "rtmp://x/live", "streamKey": "live_abc"
                }]
            }]
        }
    })
}

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

// ---------------------------------------------------------------------------
// typed REST handlers for the Settings resource.
// ---------------------------------------------------------------------------

#[test]
fn settings_get_returns_resolved_defaults_on_fresh_install() {
    let server = boot();
    let (status, body) = get(&server, "/api/v1/settings");
    assert_eq!(status, 200, "/api/v1/settings GET failed: {body}");
    let json: Value = serde_json::from_str(&body).expect("settings JSON");
    // Defaults the plan pins.
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
// Their bound checks now run in
// `ProfileManager::save_with_key_encryption`, and the equivalent
// HTTP-surface coverage lives under the profile PUT endpoint
// (`profile_put_rejects_out_of_range_*`). The Settings-PUT layer only
// owns `logRetentionDays` after Option A retired the global download
// + connection settings.

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

// ===========================================================================
// CSRF middleware end-to-end behaviour.
//
// State-changing requests (POST/PUT/PATCH/DELETE) are gated on the
// Sec-Fetch-Site header (primary) and the Origin allow-list (fallback).
// The middleware is mounted across the whole router, so any handler that
// accepts a mutating method exercises it. We pick `/api/v1/auth/login`
// because it's available without prior state and returns a stable shape.
// ===========================================================================

#[test]
fn csrf_same_origin_request_passes() {
    let server = boot();
    let (status, _) = post_with_headers(
        &server,
        "/api/v1/auth/login",
        r#"{"token":""}"#,
        &[("Sec-Fetch-Site", "same-origin")],
    );
    assert_eq!(status, 200, "Sec-Fetch-Site: same-origin must be allowed");
}

#[test]
fn csrf_same_site_request_passes() {
    let server = boot();
    let (status, _) = post_with_headers(
        &server,
        "/api/v1/auth/login",
        r#"{"token":""}"#,
        &[("Sec-Fetch-Site", "same-site")],
    );
    assert_eq!(status, 200, "Sec-Fetch-Site: same-site must be allowed");
}

#[test]
fn csrf_none_user_initiated_request_passes() {
    let server = boot();
    // Sec-Fetch-Site: none means the request was initiated by the user
    // (typed URL, bookmark, redirect from the address bar) — these are
    // not CSRF vectors.
    let (status, _) = post_with_headers(
        &server,
        "/api/v1/auth/login",
        r#"{"token":""}"#,
        &[("Sec-Fetch-Site", "none")],
    );
    assert_eq!(status, 200);
}

#[test]
fn csrf_cross_site_without_trusted_origin_is_rejected() {
    let server = boot();
    let (status, body) = post_with_headers(
        &server,
        "/api/v1/auth/login",
        r#"{"token":""}"#,
        &[
            ("Sec-Fetch-Site", "cross-site"),
            ("Origin", "http://evil.example.com"),
        ],
    );
    assert_eq!(
        status, 403,
        "Sec-Fetch-Site: cross-site from untrusted origin must be rejected: {body}"
    );
    let json: Value = serde_json::from_str(&body).expect("CSRF rejection emits JSON");
    assert_eq!(json["ok"], Value::Bool(false));
}

#[test]
fn csrf_cross_site_with_tauri_origin_passes() {
    // Tauri webview requests are technically cross-site (tauri:// → http://)
    // so the cross-site → Origin-allow-list fallback must accept the
    // built-in Tauri origins.
    let server = boot();
    let (status, _) = post_with_headers(
        &server,
        "/api/v1/auth/login",
        r#"{"token":""}"#,
        &[
            ("Sec-Fetch-Site", "cross-site"),
            ("Origin", "tauri://localhost"),
        ],
    );
    assert_eq!(status, 200, "Tauri webview origin must pass via allow-list");
}

#[test]
fn csrf_missing_headers_falls_back_to_cli_pass() {
    // CLI tools (and tests like this harness) send neither Sec-Fetch-Site
    // nor Origin. The middleware lets these through and defers to
    // auth_middleware. This is the same fallback the http_surface tests
    // above already implicitly rely on.
    let server = boot();
    let (status, _) = post_with_headers(&server, "/api/v1/auth/login", r#"{"token":""}"#, &[]);
    assert_eq!(
        status, 200,
        "no CSRF headers + no Origin must pass (CLI fallback)"
    );
}

#[test]
fn csrf_missing_sec_fetch_with_evil_origin_is_rejected() {
    // Older browser path: Sec-Fetch-Site absent → fall back to Origin
    // allow-list. An attacker forging an Origin header pointing at their
    // own domain must still get rejected.
    let server = boot();
    let (status, _) = post_with_headers(
        &server,
        "/api/v1/auth/login",
        r#"{"token":""}"#,
        &[("Origin", "http://evil.example.com")],
    );
    assert_eq!(
        status, 403,
        "untrusted Origin must be rejected when Sec-Fetch-Site is absent"
    );
}

// ===========================================================================
// Per-endpoint rate limit + login brute-force defense.
//
// `POST /api/v1/auth/login` is gated by TWO layers:
//   * IP-keyed rate limit (5/min) — quick to trip, cheap rejection.
//   * AuthService exponential backoff + sliding-window lockout — kicks in
//     once enough failures accumulate; surfaces a Retry-After.
//
// The login limiter trips at 5 attempts which is BELOW the AuthService
// lockout threshold (10), so a slow attacker (one request per minute)
// hits the lockout first, while a fast attacker hits the rate limiter
// first. Both produce 429s the client should obey.
// ===========================================================================

#[test]
fn login_with_correct_token_succeeds() {
    let server = boot_with_token("correct-horse-battery-staple");
    let (status, body) = post_invoke(
        &server,
        "/api/v1/auth/login",
        r#"{"token":"correct-horse-battery-staple"}"#,
    );
    assert_eq!(status, 200, "correct token must succeed: {body}");
    let json: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(json["ok"], Value::Bool(true));
}

#[test]
fn login_with_wrong_token_returns_401_then_eventually_429() {
    let server = boot_with_token("correct");
    // Each wrong-token attempt should return 401 from the AuthService
    // path (the per-IP rate limit is 5/min, so the first 5 attempts
    // exercise the unauthorized path). Subsequent attempts should hit
    // either the rate limiter or the lockout — both surface 429.
    for attempt in 1..=4 {
        let (status, body) = post_invoke(&server, "/api/v1/auth/login", r#"{"token":"wrong"}"#);
        assert_eq!(
            status, 401,
            "attempt {attempt}: expected 401 from wrong-token path, got {status}: {body}",
        );
    }
    // The 5th wrong-token attempt may still be 401 (rate limit is 5/min
    // with burst), but by the 6th it MUST be 429.
    let _ = post_invoke(&server, "/api/v1/auth/login", r#"{"token":"wrong"}"#);
    let (status, body) = post_invoke(&server, "/api/v1/auth/login", r#"{"token":"wrong"}"#);
    assert_eq!(
        status, 429,
        "after 6 wrong-token attempts the 6th must be rate-limited: {body}",
    );
}

#[test]
fn login_429_lockout_carries_retry_after_header() {
    // Drive enough failures to trip the per-IP login rate limit (5/min)
    // and confirm the response carries a Retry-After hint so well-behaved
    // clients can back off.
    let server = boot_with_token("correct");
    for _ in 0..6 {
        let _ = post_invoke(&server, "/api/v1/auth/login", r#"{"token":"wrong"}"#);
    }
    let resp = reqwest::blocking::Client::new()
        .post(format!("{}/api/v1/auth/login", server.base))
        .header("content-type", "application/json")
        .body(r#"{"token":"wrong"}"#)
        .send()
        .expect("POST");
    assert_eq!(resp.status().as_u16(), 429);
    // The body must indicate a non-success.
    let body: Value = resp.json().expect("JSON body");
    assert_eq!(body["ok"], Value::Bool(false));
}

// ===========================================================================
// Confirmation tokens for destructive ops.
//
// `DELETE /api/v1/settings/data` and `POST /api/v1/security/machine-key/rotate`
// require an `X-Confirm-Token` issued by `POST /api/v1/security/confirm-token`.
// Without one, the destructive endpoint must reject with 4xx. With one, the
// operation runs (no-op on a fresh install for clear_data — the assertion is
// that we got past the confirm-token gate).
// ===========================================================================

fn delete_with_headers(
    server: &ServerHandle,
    path: &str,
    headers: &[(&str, &str)],
) -> (u16, String) {
    let mut req = reqwest::blocking::Client::new().delete(format!("{}{}", server.base, path));
    for (k, v) in headers {
        req = req.header(*k, *v);
    }
    let resp = req.send().expect("DELETE");
    (resp.status().as_u16(), resp.text().expect("body"))
}

#[test]
fn destructive_clear_data_without_confirm_token_is_rejected() {
    let server = boot();
    let (status, _body) = delete_with_headers(&server, "/api/v1/settings/data", &[]);
    assert!(
        status == 403 || status == 401 || status == 400,
        "missing X-Confirm-Token must be rejected (got {status})",
    );
}

#[test]
fn destructive_clear_data_with_valid_confirm_token_succeeds() {
    let server = boot();
    // Request a confirm token for the `clear_data` intent.
    let (issue_status, issue_body) = post_invoke(
        &server,
        "/api/v1/security/confirm-token",
        r#"{"intent":"clear_data"}"#,
    );
    assert_eq!(issue_status, 200, "issue endpoint failed: {issue_body}");
    let issued: Value = serde_json::from_str(&issue_body).unwrap();
    let token = issued["token"].as_str().expect("token in response");
    assert!(!token.is_empty());

    // Present the token on the destructive call.
    let (status, body) = delete_with_headers(
        &server,
        "/api/v1/settings/data",
        &[("X-Confirm-Token", token)],
    );
    assert_eq!(
        status, 200,
        "with valid token, clear_data must succeed: {body}"
    );
}

#[test]
fn confirm_token_is_one_shot() {
    let server = boot();
    let (_, body) = post_invoke(
        &server,
        "/api/v1/security/confirm-token",
        r#"{"intent":"clear_data"}"#,
    );
    let token = serde_json::from_str::<Value>(&body).unwrap()["token"]
        .as_str()
        .unwrap()
        .to_string();
    // First use succeeds
    let (status1, _) = delete_with_headers(
        &server,
        "/api/v1/settings/data",
        &[("X-Confirm-Token", &token)],
    );
    assert_eq!(status1, 200);
    // Second use rejected (token already consumed)
    let (status2, _) = delete_with_headers(
        &server,
        "/api/v1/settings/data",
        &[("X-Confirm-Token", &token)],
    );
    assert!(
        status2 == 403 || status2 == 401 || status2 == 400,
        "second use of the same token must fail (got {status2})",
    );
}

#[test]
fn revoke_all_sessions_requires_confirm_token_and_runs_when_present() {
    let server = boot();
    // Without confirm token → 4xx
    let resp = reqwest::blocking::Client::new()
        .post(format!(
            "{}/api/v1/security/sessions/revoke-all",
            server.base
        ))
        .send()
        .expect("POST");
    let bad_status = resp.status().as_u16();
    assert!(
        bad_status == 403 || bad_status == 401 || bad_status == 400,
        "revoke-all without confirm token must be rejected (got {bad_status})",
    );

    // With matching confirm token → 200
    let (_, body) = post_invoke(
        &server,
        "/api/v1/security/confirm-token",
        r#"{"intent":"revoke_all_sessions"}"#,
    );
    let token = serde_json::from_str::<Value>(&body).unwrap()["token"]
        .as_str()
        .unwrap()
        .to_string();
    let resp = reqwest::blocking::Client::new()
        .post(format!(
            "{}/api/v1/security/sessions/revoke-all",
            server.base
        ))
        .header("X-Confirm-Token", &token)
        .send()
        .expect("POST");
    assert_eq!(resp.status().as_u16(), 200);
    let payload: Value = resp.json().expect("JSON body");
    assert_eq!(payload["ok"], Value::Bool(true));
    // `revoked` is the count of sessions that were active before the
    // call. On a fresh-boot server with no token configured nobody
    // has logged in, so this will be zero — but the field must exist.
    assert!(payload.get("revoked").is_some(), "missing revoked field");
}

#[test]
fn confirm_token_intent_scoping() {
    // A token issued for clear_data must not unlock rotate_machine_key.
    let server = boot();
    let (_, body) = post_invoke(
        &server,
        "/api/v1/security/confirm-token",
        r#"{"intent":"clear_data"}"#,
    );
    let token = serde_json::from_str::<Value>(&body).unwrap()["token"]
        .as_str()
        .unwrap()
        .to_string();
    let resp = reqwest::blocking::Client::new()
        .post(format!(
            "{}/api/v1/security/machine-key/rotate",
            server.base
        ))
        .header("X-Confirm-Token", &token)
        .send()
        .expect("POST");
    let status = resp.status().as_u16();
    assert!(
        status == 403 || status == 401 || status == 400,
        "wrong-intent token must be rejected (got {status})",
    );
}

// ===========================================================================
// Safety panic emits an audit entry the read endpoint can find.
// ===========================================================================

#[test]
fn safety_panic_records_an_audit_entry_visible_through_audit_log_endpoint() {
    let server = boot();
    // Trigger a panic to produce an audit entry.
    let (status, _) = post_invoke(&server, "/api/v1/safety/panic", "");
    assert_eq!(status, 200);
    // Read the audit log; the panic_triggered entry must be there.
    let (status, body) = get(&server, "/api/v1/audit/log?kind=panic_triggered");
    assert_eq!(status, 200, "audit/log failed: {body}");
    let payload: Value = serde_json::from_str(&body).unwrap();
    let entries = payload["entries"].as_array().expect("entries array");
    assert!(!entries.is_empty(), "no panic_triggered entries: {payload}");
}

#[test]
fn audit_log_filter_by_kind_excludes_other_kinds() {
    let server = boot();
    let _ = post_invoke(&server, "/api/v1/safety/panic", "");
    // Filter by a kind we know hasn't happened yet — must return empty.
    let (status, body) = get(&server, "/api/v1/audit/log?kind=machine_key_rotated");
    assert_eq!(status, 200);
    let payload: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(payload["total"], 0);
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
