# SpiritStream Architecture

## Layered architecture

```text
┌──────────────────────────────────────────────────────────────────┐
│  ONE React frontend, shipped through THREE shells                │
│    • Tauri 2 desktop (macOS, Linux, Windows)                     │
│    • Tauri 2 mobile  (iOS, Android)                              │
│    • Docker / browser (HTTP)                                     │
│  Plus: spiritstream-cli (headless, integration-test substrate)   │
├──────────────────────────────────────────────────────────────────┤
│  Transport adapters — implement core::traits::transport::Transport│
│    • HttpTransport     (Axum + utoipa, /api/v1/*)                │
│    • CliTransport      (in-process dispatch)                     │
│    • VeilidTransport   (contract-validation spike; see BLOCKERS) │
├──────────────────────────────────────────────────────────────────┤
│  crates/core/spiritstream-core  (Rust library, NO transport deps)│
│    • services/, models/, traits/, errors/                        │
└──────────────────────────────────────────────────────────────────┘
```

**Frontends are thin.** UI-only concerns stay in React: focus management, animations, modal open/close, form input state during editing. Every business rule — validation, orchestration, side-effect decisions — lives in `crates/core`.

## Monorepo layout

```text
spiritstream/
├── apps/
│   ├── web/                       # React frontend
│   └── tauri/                     # Tauri 2 shell — desktop + mobile
│       └── src-tauri/             # gen/apple + gen/android for mobile targets
├── crates/
│   ├── core/                      # spiritstream-core (transport-agnostic library)
│   ├── transport-http/            # Axum + utoipa adapter
│   ├── transport-cli/             # spiritstream-cli binary
│   └── transport-veilid/          # Contract-validation spike — see BLOCKERS.md
├── server/                        # Thin binary wiring core + transport-http
├── packages/
│   ├── types/                     # Rust → TS via ts-rs (auto-generated)
│   ├── api-client/                # ApiClient interface + HttpClient impl
│   ├── validation/                # JSON Schemas from utoipa
│   └── ui/                        # Shared React primitives
├── deploy/
│   ├── docker/                    # Docker image build context
│   ├── compose/                   # docker-compose with Caddy + Let's Encrypt
│   └── helm/                      # K8s Helm chart
├── tests/
│   ├── integration/               # CLI-driven golden-file tests
│   ├── threat-model/              # Doxxing/EXIF/PII fixtures
│   └── a11y/                      # Playwright + axe-core
├── docs/                          # User and operator docs
└── themes/                        # Theme bundle data
```

## Rules for any new code

1. **No business logic in the frontend.** If the code answers "should this action happen?" it belongs in `crates/core`. The frontend answers "how should this look?"
2. **No transport types in core.** `crates/core` must compile without `axum`, `tower`, `hyper`, `tauri`, `clap`, or any HTTP/CLI/Tauri crate.
3. **All API calls through `@spiritstream/api-client`.** Never `fetch()` directly from a component.
4. **Domain types live once.** Define in `crates/core/src/models/`, derive `ts_rs::TS`, consume from `@spiritstream/types`. Do not redeclare in TypeScript.
5. **Errors are structured.** Use `CoreError` variants; do not return new `Result<T, String>` from core services.
6. **Transports map errors.** `transport-http` maps `CoreError → HTTP status`; `transport-cli` maps `CoreError → exit code`. Mapping lives in the transport, not in core.
7. **Every UI operation must be reachable from `spiritstream-cli`.** If it isn't, that's a contract bug — fix core first.
8. **Mobile constraints inform core design.** Sidecar binaries are restricted on iOS/Android; the core links into the Tauri 2 mobile shell as a library and the in-process Axum binds `localhost:8008`.
9. **No runtime fallback chains.** Where a primary/secondary impl exists (e.g., keyring vs. encrypted-file secret store), the choice is made once at startup via platform probe or env override and held for the process lifetime. Never `try_primary().or_else(secondary)` at the call site.
10. **No legacy/transitional dual-mount.** Forward-only architecture. There are no unversioned URL aliases, no `/api/invoke/:command` fallback, no schema-version migration framework.
11. **No `#[allow(dead_code)]`; no TODO/FIXME markers in source.** Adapters, fields, and helpers must have a live caller in the same change. Future work tracks in plans, issues, or `BLOCKERS.md`.
12. **Per-transport security hardening is allowed even though business logic isn't.** CSRF, cookie attributes, and CORS are HTTP-shaped concerns that legitimately live in `transport-http`. The Veilid analogue (peer-keypair verification) would live in its transport. See `crates/transport-veilid/BLOCKERS.md` for the boundary discussion.

## Service catalog (`crates/core/src/services/`)

| Service | Responsibility |
|---|---|
| `profile_service` | Profile CRUD, validation, encryption boundaries (delegated to `SecretStore`). |
| `stream_service` | Stream lifecycle, reconnection, retry policy. Uses `MediaProcessor` trait (FFmpeg on desktop; HaishinKit/RootEncoder follow-up branch on mobile). |
| `chat_service` | Multi-platform chat connect/send/receive. Applies the PII filter on inbound and `check_outbound_pii` on outbound. |
| `obs_service` | OBS WebSocket integration. Owns trigger orchestration state machine. |
| `oauth_service` | OAuth 2.0 flows with token refresh. |
| `settings_service` | Global settings persistence with bound-checked setters. |
| `theme_service` | Theme catalog, install, hot-reload. |
| `audit_log_service` | HMAC-chained append-only audit trail. `verify_chain()` detects tamper. |
| `safety_service` | Panic disconnect, PII blocklist, EXIF stripping, anonymous-mode pseudonymizer. |
| `auth` | Brute-force defense (exponential backoff + sliding-window lockout). |
| `auth_surveillance` | OAuth refresh-frequency anomaly detection. |
| `secret_store` | `KeyringSecretStore` or `EncryptedFileSecretStore`, selected once at startup. |
| `encryption` | AES-256-GCM-SIV envelope (V2 writes, V1 read-only) under HKDF-derived keys. |
| `pii_filter` | Strict + fuzzy matchers; returns `phrase_id` for audit, never the phrase. |
| `pseudonymizer` | HMAC-SHA256 keyed-hash for anonymous mode. |
| `media_sanitizer` | EXIF / IPTC / XMP / PNG-text strip, thumbnail removal. |
| `confirm_token` | One-shot intent-scoped 30s-TTL tokens for destructive ops. |
| `secure_io` | `write_owner_only_atomic` (temp + rename + 0600 on Unix). |

Each service is constructed once, held in `ServiceRegistry`, and shared as `Arc<...>` across all active transports.

## Transports

### `transport-http` (Axum + utoipa)

Versioned REST under `/api/v1/*`. utoipa annotations on every handler generate an OpenAPI spec served at `/api/v1/openapi.json`; `@hey-api/openapi-ts` consumes that to produce `@spiritstream/api-client`.

Middleware stack: `request_id_middleware` → `csrf_middleware` (Sec-Fetch-Site + Origin allow-list, WS upgrades guarded) → `rate_limit_middleware` (per-endpoint, keyed by SHA-256 hash of auth subject) → `auth_middleware` (cookie + Bearer, `verify_token` hashes before `ct_eq`).

Cookie attributes via `SessionCookieMode` enum: `SameOrigin` (Tauri webview), `CrossOrigin` (browser-served), `LocalhostDev`. Pure-function `detect(host, deploy_mode, explicit)` for parallel-test safety.

Cloud-mode startup guard `enforce_cloud_mode_preconditions(token, tls_declared)` refuses to start without TLS and a ≥32-char token.

WebSocket: `GET /api/v1/events`, one-way server-push only. CSRF guard runs on upgrade.

### `transport-cli`

`spiritstream-cli` binary calls into `ServiceRegistry` directly — **no HTTP shell-out**. JSON output by default, `--pretty` for humans, `--quiet` for scripting. Exit codes derived from `CoreError` variants. Integration tests under `tests/integration/` are CLI-driven and golden-compared.

### `transport-veilid` (contract-validation spike)

Stub implementing `Transport` to prove the contract holds for a non-HTTP transport. `VeilidTransport::serve` returns `CoreError::NotImplemented`. The deliverable is `crates/transport-veilid/BLOCKERS.md` — 10 enumerated HTTP-shaped contract gaps (the biggest is **URL paths → method namespace**) that a real Veilid implementation must close. The crate compiles against `crates/core` alone with no Axum, Tauri, or Veilid SDK.

## Deployment modes

| Mode | Shell | Transport | Notes |
|---|---|---|---|
| **Desktop** | Tauri 2 | HTTP sidecar (`server/` spawned as a subprocess) | macOS / Linux / Windows. |
| **Mobile** | Tauri 2 | HTTP in-process (core linked into Tauri Rust shell; Axum on `localhost:8008`) | iOS / Android. Sidecars don't work on either. |
| **Docker / browser** | None | HTTP | Same `server/` binary; serves UI bundle + API. Caddy + Let's Encrypt in front. |
| **CLI** | None | In-process dispatch | First-class client; integration-test substrate. |

## Pointers

- Coding standards: `.claude/rules/coding-standards.md`
- Git workflow: `.claude/rules/git-workflow.md`
- Documentation rules: `.claude/rules/documentation.md`
- Veilid contract gaps: `crates/transport-veilid/BLOCKERS.md`
- Per-crate READMEs: `crates/*/README.md`
