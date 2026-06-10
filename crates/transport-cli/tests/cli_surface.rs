//! End-to-end integration tests for `spiritstream-cli`.
//!
//! Every test spawns the built binary against a fresh tempdir, asserts on
//! stdout (parsed as JSON) plus exit code, and shuts down. These tests are
//! the Rust mirror of the shell-based golden suite under
//! `tests/integration/` at the workspace root — both surfaces must stay in
//! sync because they pin the same contract.

use std::{
    path::PathBuf,
    process::{Command, Output},
};

use serde_json::Value;
use tempfile::TempDir;

fn cli_binary() -> PathBuf {
    let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    loop {
        let candidate = dir.join("target").join("debug").join(binary_name());
        if candidate.exists() {
            return candidate;
        }
        if !dir.pop() {
            panic!(
                "could not find spiritstream-cli binary; run `cargo build -p spiritstream-cli` first"
            );
        }
    }
}

#[cfg(windows)]
fn binary_name() -> &'static str {
    "spiritstream-cli.exe"
}

#[cfg(not(windows))]
fn binary_name() -> &'static str {
    "spiritstream-cli"
}

fn run_cli(args: &[&str], data_dir: &std::path::Path) -> Output {
    Command::new(cli_binary())
        .arg("--data-dir")
        .arg(data_dir)
        .arg("--quiet")
        .args(args)
        .output()
        .expect("spawn spiritstream-cli")
}

/// Run the CLI with a secret piped on stdin — the only scripted way to
/// supply secrets now that plaintext secret flags are gone.
fn run_cli_with_stdin(args: &[&str], data_dir: &std::path::Path, stdin_body: &str) -> Output {
    use std::io::Write;
    use std::process::Stdio;
    let mut child = Command::new(cli_binary())
        .arg("--data-dir")
        .arg(data_dir)
        .arg("--quiet")
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn spiritstream-cli");
    child
        .stdin
        .as_mut()
        .expect("piped stdin")
        .write_all(stdin_body.as_bytes())
        .expect("write stdin");
    child.wait_with_output().expect("wait for spiritstream-cli")
}

fn stdout_json(out: &Output) -> Value {
    let body = std::str::from_utf8(&out.stdout).expect("stdout is utf8");
    serde_json::from_str(body).unwrap_or_else(|e| panic!("stdout not JSON: {e}\n---\n{body}"))
}

// ---------------------------------------------------------------------------

#[test]
fn profile_list_is_empty_on_fresh_install() {
    let tmp = TempDir::new().expect("tempdir");
    let out = run_cli(&["profile", "list"], tmp.path());
    assert!(
        out.status.success(),
        "expected success, got {:?}",
        out.status
    );
    let body = stdout_json(&out);
    assert_eq!(body["names"], serde_json::json!([]));
}

#[test]
fn profile_exists_reports_missing_profile() {
    let tmp = TempDir::new().expect("tempdir");
    let out = run_cli(&["profile", "exists", "nonesuch"], tmp.path());
    assert!(out.status.success());
    let body = stdout_json(&out);
    assert_eq!(body["name"], "nonesuch");
    assert_eq!(body["exists"], false);
}

#[test]
fn settings_get_returns_defaults() {
    let tmp = TempDir::new().expect("tempdir");
    let out = run_cli(&["settings", "get"], tmp.path());
    assert!(out.status.success());
    let body = stdout_json(&out);
    // Defaults: 30-day log retention, no last profile.
    assert_eq!(body["logRetentionDays"], 30);
    assert!(body["lastProfile"].is_null());
}

#[test]
fn theme_list_includes_bundled_themes() {
    let tmp = TempDir::new().expect("tempdir");
    let out = run_cli(&["theme", "list"], tmp.path());
    assert!(out.status.success());
    let body = stdout_json(&out);
    let themes = body.as_array().expect("themes array");
    assert!(
        themes.len() >= 4,
        "expected at least the bundled themes, got {}",
        themes.len()
    );
    let ids: Vec<&str> = themes.iter().filter_map(|t| t["id"].as_str()).collect();
    for required in ["spirit-light", "spirit-dark"] {
        assert!(
            ids.contains(&required),
            "missing required theme: {required}"
        );
    }
}

#[test]
fn profile_show_on_missing_profile_returns_typed_not_found() {
    // migrated ProfileManager to typed CoreError. Missing profiles
    // surface as `CoreError::ProfileNotFound` → CLI exit code 4, JSON kind
    // `profile_not_found`.
    let tmp = TempDir::new().expect("tempdir");
    let out = run_cli(&["profile", "show", "missing"], tmp.path());
    assert_eq!(
        out.status.code(),
        Some(4),
        "expected profile_not_found exit code 4, got {:?}",
        out.status
    );
    let body = stdout_json(&out);
    assert_eq!(body["ok"], false);
    assert_eq!(body["kind"], "profile_not_found");
}

#[test]
fn pretty_flag_indents_output() {
    let tmp = TempDir::new().expect("tempdir");
    let out = Command::new(cli_binary())
        .arg("--data-dir")
        .arg(tmp.path())
        .arg("--quiet")
        .arg("--pretty")
        .args(["settings", "get"])
        .output()
        .expect("spawn");
    assert!(out.status.success());
    let body = std::str::from_utf8(&out.stdout).expect("utf8");
    // Pretty output spans multiple lines.
    assert!(
        body.contains('\n'),
        "pretty output should span multiple lines: {body}"
    );
}

#[test]
fn unknown_command_exits_with_usage_error() {
    let tmp = TempDir::new().expect("tempdir");
    let out = Command::new(cli_binary())
        .arg("--data-dir")
        .arg(tmp.path())
        .arg("flarble")
        .output()
        .expect("spawn");
    // clap's default exit code for usage errors is 2.
    assert!(!out.status.success());
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn stream_status_is_empty_on_fresh_install() {
    let tmp = TempDir::new().expect("tempdir");
    let out = run_cli(&["stream", "status"], tmp.path());
    assert!(out.status.success());
    let body = stdout_json(&out);
    assert_eq!(body["active"], serde_json::json!([]));
    assert_eq!(body["count"], 0);
}

#[test]
fn stream_stop_without_group_stops_all() {
    // Plan UX: `stream stop [--group <id>]`. Omitting --group stops every
    // active group (and is a no-op on a fresh install).
    let tmp = TempDir::new().expect("tempdir");
    let out = run_cli(&["stream", "stop"], tmp.path());
    assert!(
        out.status.success(),
        "stop with no args should succeed: {:?}",
        out.status
    );
    let body = stdout_json(&out);
    assert_eq!(body["stopped"], serde_json::json!([]));
}

#[test]
fn stream_stop_with_explicit_group_targets_just_that_group() {
    let tmp = TempDir::new().expect("tempdir");
    // stop() is idempotent for non-active groups (no-op success). We assert
    // the flag plumbing routes to single-group stop, not stop-all.
    let out = run_cli(&["stream", "stop", "--group", "doesnotexist"], tmp.path());
    assert!(
        out.status.success(),
        "stop on idle group should be no-op: {:?}",
        out.status
    );
    let body = stdout_json(&out);
    assert_eq!(body["stopped"], serde_json::json!(["doesnotexist"]));
}

#[test]
fn system_encoders_returns_structured_payload() {
    let tmp = TempDir::new().expect("tempdir");
    let out = run_cli(&["system", "encoders"], tmp.path());
    // On a host without FFmpeg the underlying call errors; on a host with
    // FFmpeg the response is a structured Encoders payload. We accept both
    // so the test is portable, but the body must be JSON in either case.
    let body: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap_or_else(|e| {
        panic!(
            "system encoders must emit JSON: {e}\nstdout={}",
            String::from_utf8_lossy(&out.stdout)
        )
    });
    if out.status.success() {
        // Successful response has video + audio arrays.
        assert!(
            body["video"].is_array()
                || body["videoEncoders"].is_array()
                || body["encoders"].is_object(),
            "expected structured encoders body, got {body}"
        );
    } else {
        // Failure response has the standard error envelope.
        assert_eq!(body["ok"], false, "error response: {body}");
        assert!(body["kind"].is_string(), "error must have kind: {body}");
    }
}

#[test]
fn data_export_actually_creates_a_zip() {
    let tmp = TempDir::new().expect("tempdir");
    let target = tmp.path().join("export.zip");
    let out = run_cli(&["data", "export", target.to_str().unwrap()], tmp.path());
    assert!(out.status.success(), "data export failed: {:?}", out.status);
    let body = stdout_json(&out);
    assert_eq!(body["exported"], true);
    assert!(
        target.exists(),
        "export should create the zip file at {target:?}"
    );
    assert!(
        target.metadata().unwrap().len() > 0,
        "exported zip should be non-empty"
    );
}

#[test]
fn encrypted_profile_round_trips_with_password() {
    let tmp = TempDir::new().expect("tempdir");
    let fixture = tmp.path().join("encrypted.json");
    let minimal = serde_json::json!({
        "id": "lock-001",
        "name": "lockedprofile",
        "encrypted": false,
        "input": {
            "type": "rtmp",
            "bindAddress": "127.0.0.1",
            "port": 1935,
            "application": "live"
        },
        "outputGroups": []
    });
    std::fs::write(&fixture, serde_json::to_vec(&minimal).unwrap()).expect("write fixture");

    // Save under a password — piped on stdin, never argv.
    let saved = run_cli_with_stdin(
        &[
            "profile",
            "save",
            fixture.to_str().unwrap(),
            "--password-from",
            "stdin",
        ],
        tmp.path(),
        "correct horse battery staple\n",
    );
    assert!(
        saved.status.success(),
        "encrypted save failed: {:?}",
        saved.status
    );

    // is-encrypted reports true.
    let probe = run_cli(&["profile", "is-encrypted", "lockedprofile"], tmp.path());
    let probe_body = stdout_json(&probe);
    assert_eq!(
        probe_body["encrypted"], true,
        "stored profile should be encrypted"
    );

    // Show without the password fails.
    let denied = run_cli(&["profile", "show", "lockedprofile"], tmp.path());
    assert!(!denied.status.success(), "show without password must fail");

    // Show with the wrong password fails.
    let wrong = run_cli_with_stdin(
        &["profile", "show", "lockedprofile", "--password-from", "stdin"],
        tmp.path(),
        "incorrect\n",
    );
    assert!(
        !wrong.status.success(),
        "show with wrong password must fail"
    );

    // Show with the correct password succeeds and returns the profile body.
    let shown = run_cli_with_stdin(
        &["profile", "show", "lockedprofile", "--password-from", "stdin"],
        tmp.path(),
        "correct horse battery staple\n",
    );
    assert!(
        shown.status.success(),
        "show with correct password failed: {:?}",
        shown.status
    );
    let shown_body = stdout_json(&shown);
    assert_eq!(shown_body["name"], "lockedprofile");
}

#[test]
fn events_watch_accepts_comma_separated_filter() {
    let tmp = TempDir::new().expect("tempdir");
    let out = run_cli(
        &[
            "events",
            "watch",
            "--filter",
            "stream_stats,chat_message",
            "--for-ms",
            "100",
        ],
        tmp.path(),
    );
    assert!(
        out.status.success(),
        "comma-separated --filter should be accepted: {:?}",
        out.status
    );
}

#[test]
fn chat_status_is_empty_on_fresh_install() {
    let tmp = TempDir::new().expect("tempdir");
    let out = run_cli(&["chat", "status"], tmp.path());
    assert!(out.status.success());
    let body = stdout_json(&out);
    assert_eq!(body, serde_json::json!([]));
}

#[test]
fn profile_save_roundtrips_a_minimal_profile() {
    let tmp = TempDir::new().expect("tempdir");
    let profile_path = tmp.path().join("seed.json");
    let minimal = serde_json::json!({
        "id": "seed-001",
        "name": "seedprofile",
        "encrypted": false,
        "input": {
            "type": "rtmp",
            "bindAddress": "127.0.0.1",
            "port": 1935,
            "application": "live"
        },
        "outputGroups": []
    });
    std::fs::write(&profile_path, serde_json::to_vec(&minimal).unwrap())
        .expect("write profile fixture");

    let saved = run_cli(
        &["profile", "save", profile_path.to_str().unwrap()],
        tmp.path(),
    );
    assert!(
        saved.status.success(),
        "save failed: {:?} stdout={}",
        saved.status,
        String::from_utf8_lossy(&saved.stdout),
    );
    let saved_body = stdout_json(&saved);
    assert_eq!(saved_body["saved"], true);
    assert_eq!(saved_body["name"], "seedprofile");

    let listed = run_cli(&["profile", "list"], tmp.path());
    assert!(listed.status.success());
    let listed_body = stdout_json(&listed);
    assert_eq!(listed_body["names"], serde_json::json!(["seedprofile"]));

    let exists = run_cli(&["profile", "exists", "seedprofile"], tmp.path());
    let exists_body = stdout_json(&exists);
    assert_eq!(exists_body["exists"], true);

    let shown = run_cli(&["profile", "show", "seedprofile"], tmp.path());
    assert!(shown.status.success(), "show failed: {:?}", shown.status);
    let shown_body = stdout_json(&shown);
    assert_eq!(shown_body["name"], "seedprofile");

    let deleted = run_cli(&["profile", "delete", "seedprofile"], tmp.path());
    assert!(
        deleted.status.success(),
        "delete failed: {:?}",
        deleted.status
    );
    let deleted_body = stdout_json(&deleted);
    assert_eq!(deleted_body["deleted"], true);

    let after = run_cli(&["profile", "list"], tmp.path());
    let after_body = stdout_json(&after);
    assert_eq!(after_body["names"], serde_json::json!([]));
}

#[test]
fn safety_panic_runs_the_full_flow_and_returns_streams_stopped() {
    // `safety panic` on a fresh install with no streams
    // active should succeed, return `streams_stopped: 0`, and append an
    // entry to the audit log on disk.
    let tmp = TempDir::new().expect("tempdir");
    let out = run_cli(&["safety", "panic"], tmp.path());
    assert!(
        out.status.success(),
        "safety panic must succeed: {:?}",
        out.status
    );
    let body = stdout_json(&out);
    assert_eq!(body["streams_stopped"], 0);
    assert!(body["elapsed_ms"].is_number(), "missing elapsed_ms: {body}");
    let audit_path = tmp.path().join("audit").join("audit.log");
    assert!(audit_path.exists(), "audit log missing");
    let audit = std::fs::read_to_string(&audit_path).unwrap();
    assert!(
        audit.contains("\"kind\":\"panic_triggered\""),
        "audit did not record panic_triggered: {audit}",
    );
}

#[test]
fn audit_log_emits_ndjson_entries() {
    let tmp = TempDir::new().expect("tempdir");
    let out = run_cli(&["audit", "log", "--limit", "10"], tmp.path());
    assert!(out.status.success(), "audit log failed: {:?}", out.status);
    let body = stdout_json(&out);
    // Empty audit log on a fresh tempdir → array is empty (not an error).
    assert!(body.is_array(), "audit log should emit an array: {body}");
}

#[test]
fn system_ffmpeg_has_subcommand_structure() {
    let tmp = TempDir::new().expect("tempdir");
    // `check` works against any registry — returns `available: false` in a
    // fresh tempdir with no FFmpeg on PATH.
    let out = run_cli(&["system", "ffmpeg", "check"], tmp.path());
    assert!(
        out.status.success(),
        "ffmpeg check failed: {:?}",
        out.status
    );
    let body = stdout_json(&out);
    assert!(body["available"].is_boolean(), "expected boolean: {body}");
    // `path` returns the resolved binary location (string) or null.
    let path = run_cli(&["system", "ffmpeg", "path"], tmp.path());
    assert!(
        path.status.success(),
        "ffmpeg path should succeed: {:?}",
        path.status
    );
    let path_body = stdout_json(&path);
    assert!(
        path_body["path"].is_string() || path_body["path"].is_null(),
        "path should be string|null: {path_body}"
    );
    // Option A retired `download` / `cancel` / `delete`: FFmpeg is
    // bundled per-platform at build time. The CLI surface shouldn't
    // accept those subcommands any more.
    for retired in ["download", "cancel", "delete"] {
        let out = run_cli(&["system", "ffmpeg", retired], tmp.path());
        assert!(
            !out.status.success(),
            "`system ffmpeg {retired}` should be rejected post-Option-A; got: {:?}",
            out.status
        );
    }
}

#[test]
fn settings_get_single_key_returns_just_the_value() {
    let tmp = TempDir::new().expect("tempdir");
    let out = run_cli(&["settings", "get", "logRetentionDays"], tmp.path());
    assert!(
        out.status.success(),
        "settings get failed: {:?}",
        out.status
    );
    let body = stdout_json(&out);
    // Default retention is 30; the value is a number, not a wrapping object.
    assert_eq!(body, serde_json::json!(30));
}

#[test]
fn settings_get_unknown_key_exits_with_argument_error() {
    let tmp = TempDir::new().expect("tempdir");
    let out = run_cli(&["settings", "get", "madeUpKey"], tmp.path());
    assert_eq!(out.status.code(), Some(64), "expected EX_USAGE 64");
    let body = stdout_json(&out);
    assert_eq!(body["kind"], "argument");
}

#[test]
fn oauth_start_returns_auth_url_and_callback_port() {
    // `oauth start` is no longer a stub. It returns the auth URL
    // the user must open in their browser plus the callback port the server
    // is listening on, matching the typed REST surface.
    let tmp = TempDir::new().expect("tempdir");
    let out = run_cli(&["oauth", "start", "twitch"], tmp.path());
    assert!(out.status.success(), "oauth start should succeed: {out:?}");
    let body = stdout_json(&out);
    assert!(body["auth_url"]
        .as_str()
        .unwrap_or("")
        .starts_with("https://"));
    assert!(body["callback_port"].as_u64().is_some());
    assert!(body["state"].as_str().is_some());
}

#[test]
fn events_watch_for_a_short_duration_exits_cleanly() {
    let tmp = TempDir::new().expect("tempdir");
    let out = run_cli(&["events", "watch", "--for-ms", "200"], tmp.path());
    assert!(
        out.status.success(),
        "events watch should exit cleanly after timer: {:?}",
        out.status
    );
}
