//! Shared test fixtures for the http_surface integration suite.
//!
//! Owns the binary boot machinery (`ServerHandle`, `boot`, retry-on-bind-race
//! loop) and the small HTTP client helpers every test reuses. Splitting these
//! into a sibling module lets the per-domain test files stay focused on
//! their own assertions; pre-split everything sat in a single 1133 LOC
//! `tests/http_surface.rs` that the 600 LOC ceiling refused.

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

pub struct ServerHandle {
    child: Child,
    pub base: String,
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
pub fn boot() -> ServerHandle {
    boot_inner(None)
}

/// Boot the server with `SPIRITSTREAM_API_TOKEN` configured — used by the
/// brute-force tests which need an enabled auth path.
pub fn boot_with_token(token: &str) -> ServerHandle {
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
            // Hermetic secrets: without this the platform probe selects
            // keyring on macOS/Windows and store-writing tests (OAuth
            // credentials, audit keys) would touch the developer's REAL
            // keychain instead of the throwaway TempDir.
            .env("SPIRITSTREAM_SECRET_STORE", "file")
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

pub fn get(server: &ServerHandle, path: &str) -> (u16, String) {
    let resp = reqwest::blocking::Client::new()
        .get(format!("{}{}", server.base, path))
        .send()
        .expect("GET");
    let status = resp.status().as_u16();
    let body = resp.text().expect("body");
    (status, body)
}

pub fn post_invoke(server: &ServerHandle, path: &str, payload: &str) -> (u16, String) {
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
pub fn post_with_headers(
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

pub fn put_json(server: &ServerHandle, path: &str, body: &Value) -> (u16, String) {
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

pub fn delete(server: &ServerHandle, path: &str) -> (u16, String) {
    let resp = reqwest::blocking::Client::new()
        .delete(format!("{}{}", server.base, path))
        .send()
        .expect("DELETE");
    let status = resp.status().as_u16();
    let body = resp.text().expect("body");
    (status, body)
}

pub fn delete_with_headers(
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

pub fn sample_profile(name: &str, port: u16) -> Value {
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

pub fn passthrough_validate_body(name: &str) -> Value {
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
