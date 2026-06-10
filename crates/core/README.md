# spiritstream-core

Transport-agnostic Rust library that holds every business rule in
SpiritStream. Built for multi-output RTMP streaming targeted at
vulnerable creators — sex workers, harassment-prone streamers,
trans / queer / journalist communities, disabled streamers, and new
non-technical streamers.

This crate has no HTTP, no Tauri, no Veilid. It compiles standalone.
Every transport adapter (HTTP, CLI, future Veilid) builds the same
`ServiceRegistry` and hands it to its own request layer.

## What lives here

```
src/
├── models/      Domain types (ts-rs exports under `#[ts(export)]`)
├── services/    Business logic — one module per service
├── traits/      Cross-cutting contracts (EventSink, SecretStore,
│                IdentityProvider, MediaProcessor, Transport, Clock)
├── errors.rs    Structured `CoreError` enum + `ValidationIssue`
├── commands/    Re-exports + utility commands (encoder probe, etc.)
└── registry.rs  `ServiceRegistry` — single home for service construction
```

### Service catalog

| Service | Responsibility |
|---|---|
| `profile_manager` | Profile CRUD, validation, port-conflict, encryption boundaries |
| `settings_manager` | Global settings, bounds-checking, atomic owner-only writes |
| `stream_service` (in `ffmpeg_handler.rs`) | Stream lifecycle, retry policy |
| `chat_manager` | Multi-platform chat connect / send / receive; PII filter; anonymous-mode pseudonymizer |
| `obs_websocket` | OBS integration, trigger orchestration |
| `oauth` | OAuth 2.0 flows with token refresh |
| `theme_manager` | Theme catalog + hot-reload |
| `encryption` | AES-256-GCM-SIV (V2) + AES-256-GCM (V1) machine-key + password-based encryption |
| `auth` | Login brute-force defense (exponential backoff + lockout) |
| `auth_surveillance` | OAuth refresh frequency anomaly detection |
| `audit_log` | Append-only HMAC chain, per-day HKDF keys, SecretStore tail anchor (async — see anchor-lag note below), startup quarantine of corrupt logs |
| `confirm_token` | One-shot intent-scoped confirmation tokens for destructive ops |
| `pii_filter` | Strict + fuzzy chat-message matcher |
| `pseudonymizer` | HMAC-SHA256 keyed-hash for anonymous-mode usernames |
| `safety` | Coordinates the panic-disconnect flow |
| `secret_store/` | Platform-detected `KeyringSecretStore` or `EncryptedFileSecretStore` |
| `media_sanitizer` | EXIF / IPTC / PNG-text strip for uploaded images |
| `secure_io` | `write_owner_only_atomic` — every sensitive write lands at mode 0600 |

## Architectural rules

* **No transport types in this crate.** Adding a dependency on
  `axum`, `tower`, `tauri`, `clap`, or any HTTP/CLI/Tauri framework
  is a contract bug — see `.claude/rules/architecture.md`.
* **Errors are structured.** Use `CoreError` variants; never return
  `Result<T, String>` from new code.
* **Domain types live once.** Define under `models/`, derive
  `ts_rs::TS`, consume from `packages/types` on the TS side. Don't
  redeclare in TypeScript.
* **Every operation reachable via `spiritstream-cli`.** If it isn't,
  that's a contract bug — fix the core, not the CLI.

## Tests

```bash
cargo test -p spiritstream-core
```

The unit + integration suite covers the encryption envelope migration,
audit-log HMAC chain tamper detection, login brute-force defense,
confirm-token issuance, PII filter matchers, pseudonymizer
deterministic-with-misuse-resistance properties, media sanitizer
chunk-removal, and the safety panic orchestrator end-to-end.

## Audit-log integrity semantics (honest edition)

* **Per-day keys**: each entry is HMAC'd with an HKDF subkey derived
  from the master audit key and the entry's UTC date
  (`spiritstream/audit-log/hmac/v1/{YYYY-MM-DD}`), so one
  compromised day key can't forge other days. Legacy static-key
  chains are archived to `audit.log.v1-archive` on first startup
  after upgrade (`ChainMigrated` opens the new chain); `spiritstream-cli
  audit verify` still verifies archives.
* **Tail anchor**: after each append, `{seq, hmac}` is written to the
  SecretStore by a dedicated async task. Verification fails loud when
  the log's tail is older than the anchor (truncation). Because the
  anchor write is asynchronous, there is a small window (typically
  <1s) where entries exist that the anchor doesn't cover yet — an
  attacker with disk access in exactly that window could truncate
  those unanchored entries undetected. Anchor-write failures degrade
  loudly (`anchorState: "degraded"` in the status DTO), never block
  appends.
* **Corruption never bricks startup**: an unverifiable or malformed
  log is quarantined to `audit.log.quarantined-{timestamp}` and a
  fresh chain opens with `ChainQuarantined` — the tamper evidence is
  preserved, the app still starts, and the status surface says so.

## Machine-key rotation recovery

Rotation journals the new key to `.stream_key.new` before touching
any profile, then rewrites profiles, shreds the old key to zero
bytes, and renames the journal into place. On startup,
`recover_interrupted_rotation()` resolves a crash in any window:
valid old key + journal present → roll back from backup; empty/missing
old key + journal present → promote the journal. Both paths record an
audit entry (`KeyRotationRolledBack` / `KeyRotationRecovered`).
Rotation refuses to start while any stream is live.

## See also

* Workspace `README.md` for the bigger picture.
* `crates/transport-http/`, `crates/transport-cli/`,
  `crates/transport-veilid/` — the three transport adapters.
