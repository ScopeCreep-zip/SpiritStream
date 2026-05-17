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
| `audit_log` | Append-only HMAC-chained audit log |
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

181 unit + integration tests cover the encryption envelope migration,
audit-log HMAC chain tamper detection, login brute-force defense,
confirm-token issuance, PII filter matchers, pseudonymizer
deterministic-with-misuse-resistance properties, media sanitizer
chunk-removal, and the safety panic orchestrator end-to-end.

## See also

* Workspace `README.md` for the bigger picture.
* `crates/transport-http/`, `crates/transport-cli/`,
  `crates/transport-veilid/` — the three transport adapters.
