# SpiritStream

Multi-output RTMP streaming application built for **vulnerable end users** — adult/sex-work creators, harassment-prone streamers, trans/queer/journalist communities, new and non-technical streamers, and disabled streamers. Safety, audit-ability, and accessibility are first-class concerns, not afterthoughts.

**Repository**: https://github.com/ScopeCreep-zip/SpiritStream

## Hard rules

- **600 LOC ceiling per source file** (non-blank, non-comment). No source file may exceed 600 LOC. Enforced by `scripts/check-loc.sh` via `lefthook` (pre-commit) and CI (`loc-gate` job). Currently-grandfathered files live in `.loc-allowlist` with per-file ceilings; the gate refuses growth even within the allowlist. Splits land per `.claude/rules/coding-standards.md#file-size-limit`.
- **No `#[allow(dead_code)]`. No `TODO`/`FIXME`/`XXX` markers in source.** Future work tracks in plans, GitHub issues, or `crates/transport-veilid/BLOCKERS.md`.
- **No silent fallbacks in safety-sensitive paths.** Pick one path or fail loud (`feedback_no_fallback_streaming.md`).
- **No business logic in the frontend.** If the answer is "should this happen?", it belongs in `crates/core`.

## Architecture

**One React app, one Tauri 2 shell (desktop + mobile), transport-agnostic Rust core.**

```
clients          apps/web (React)  ─── wrapped by ───▶  Tauri 2 desktop (macOS/Linux/Windows)
                                                        Tauri 2 mobile (iOS/Android)
                                                        Docker / browser (HTTP)
                                  spiritstream-cli (headless + integration-test substrate)

transports       transport-http (Axum + utoipa, REST /api/v1/*)
                 transport-cli   (in-process dispatch, golden-file tests)
                 transport-veilid (contract-validation spike — see crates/transport-veilid/BLOCKERS.md)

core             crates/core/spiritstream-core
                   services/, models/, traits/, errors/
                   NO Axum, NO Tauri, NO HTTP — pluggable everywhere
```

Frontends are **thin wrappers** — they own only UI concerns (focus, animations, layout state). Every business rule lives in `crates/core`. See `.claude/rules/architecture.md` for the full layer map.

## Repository layout

```
spiritstream/
├── apps/
│   ├── web/                React frontend
│   └── tauri/              Tauri 2 shell — desktop + mobile
│       └── src-tauri/      gen/apple/ + gen/android/ for mobile targets
├── crates/                 Rust workspace members
│   ├── core/               spiritstream-core (transport-agnostic library)
│   ├── transport-http/     Axum + utoipa REST adapter
│   ├── transport-cli/      spiritstream-cli binary
│   └── transport-veilid/   Contract-validation spike (NotImplemented; see BLOCKERS.md)
├── server/                 Thin binary wiring core + transport-http (Docker / Tauri desktop sidecar)
├── packages/               types (ts-rs), api-client (@hey-api/openapi-ts), validation, ui
├── deploy/                 docker/, compose/ (Caddy + Let's Encrypt), helm/
├── tests/
│   ├── integration/        CLI-driven golden-file tests
│   ├── threat-model/       Doxxing / EXIF / PII fixtures
│   └── a11y/               Playwright + @axe-core/playwright
└── docs/                   User and operator docs
```

## Tech Stack

| Layer | Technology |
|-------|------------|
| Backend (core) | Rust — transport-agnostic library crate |
| HTTP transport | Axum 0.7 + utoipa (OpenAPI 3.0) |
| CLI transport | clap + insta golden tests |
| Frontend | React 19 + TypeScript 5.9 strict |
| Styling | Tailwind v4 with design tokens; `data-theme="dark"` |
| Build | Vite 7 + Turbo + Cargo workspace |
| State | Zustand 5 (UI state only) |
| i18n | i18next, 11 locales |
| Shells | Tauri 2.11.x (desktop today, mobile targets wired) |
| Type sync | ts-rs (Rust → TS) + @hey-api/openapi-ts (OpenAPI → typed client) |
| Encryption | AES-256-GCM-SIV (RFC 8452) under Argon2id (m=64 MiB, t=3, p=4) |
| Secrets | `keyring-core` + platform-native stores OR encrypted-file (selected once at startup) |
| Audit log | HMAC-SHA256 chain with per-day HKDF-derived keys |

**Tauri 2 is the only desktop/mobile shell.** No Electron, no React Native, no Capacitor. Mobile uses Tauri 2's iOS/Android targets with the same React app.

## Build Commands

```bash
pnpm dev                       # All workspaces (Turbo)
pnpm dev:web                   # Frontend only (localhost:5173)
pnpm dev:desktop               # Desktop app (Tauri 2 + server sidecar)

pnpm build                     # All workspaces (Turbo)
pnpm build:web                 # Frontend only
pnpm build:desktop             # Desktop app with sidecar

pnpm typecheck                 # TypeScript checking (Turbo)
cargo check --workspace        # All Rust crates
cargo test --workspace         # All tests (ts-rs export runs here)

cargo run -p spiritstream-cli -- profile list
cargo run -p spiritstream-cli -- stream status --pretty
bash tests/integration/run.sh  # CLI-driven golden-file integration suite
```

Mobile (requires Xcode 16+ / Android NDK r27+):

```bash
pnpm --filter @spiritstream/tauri tauri ios init       # one-shot
pnpm --filter @spiritstream/tauri tauri ios dev
pnpm --filter @spiritstream/tauri tauri android init
pnpm --filter @spiritstream/tauri tauri android dev
```

## Environment Variables

```bash
# Frontend (Vite)
VITE_BACKEND_URL=http://host:8008   # Backend URL
VITE_BACKEND_TOKEN=secret           # Auth token

# Backend server
SPIRITSTREAM_HOST=127.0.0.1         # Bind address
SPIRITSTREAM_PORT=8008              # HTTP port — must be 1024..=65535 (privileged ports + 0 rejected at startup)
SPIRITSTREAM_API_TOKEN=secret       # Auth token (≥32 chars required in cloud mode)
SPIRITSTREAM_DEV_TOKEN=secret       # Alias for API_TOKEN — read when API_TOKEN is unset (dev convenience)
SPIRITSTREAM_UI_ENABLED=1           # Serve static UI bundle from the backend
SPIRITSTREAM_DEPLOY_MODE=desktop    # desktop | cloud — cloud refuses startup without TLS + strong token
SPIRITSTREAM_BEHIND_TLS_PROXY=1     # Required when DEPLOY_MODE=cloud (reverse-proxy attestation)
SPIRITSTREAM_CORS_ORIGINS=https://… # Comma-separated allow-list (no wildcards in cloud)
SPIRITSTREAM_COOKIE_MODE=same-origin # same-origin | cross-origin | localhost-dev — override the auto-probe
SPIRITSTREAM_SECRET_STORE=keyring   # keyring | file — overrides the one-shot platform probe
SPIRITSTREAM_DATA_DIR=./data        # Where profiles, audit log, secrets-on-disk live
SPIRITSTREAM_LOG_DIR=./data/logs    # Log directory; defaults to {DATA_DIR}/logs when unset
SPIRITSTREAM_THEMES_DIR=./themes    # Theme catalog directory (read for the View → Theme menu)
SPIRITSTREAM_UI_DIR=./dist          # Static UI bundle directory when UI_ENABLED=1
SPIRITSTREAM_LOG_FORMAT=json        # json for log shippers; text for human reading
```

## Design Theme

Purple & pink palette. WCAG 2.2 AA across the UI; AAA-7:1 high-contrast theme variant available. Full light/dark mode via `data-theme="dark"`. Primary: Violet, Secondary: Fuchsia, Accent: Pink. Tokens at `apps/web/src/styles/tokens.css`; design rationale at `.claude/claudedocs/research/spiritstream-complete-design-system.md`.

## Coding Standards

- **Rust**: structured `CoreError` enum (no `Result<T, String>` in new code), `Arc<ServiceRegistry>` for service wiring, `mask_sensitive()` for any log payload, traits in `crates/core/src/traits/` for cross-cutting concerns. No `#[allow(dead_code)]`; no adapters/fields without a live caller.
- **TypeScript**: strict mode, explicit return types, `interface` for objects, `type` for unions. Import domain types only from `@spiritstream/types`; never re-declare.
- **React**: functional components, Zustand for UI state only, all API calls via the `api` client from `@spiritstream/api-client`. No `fetch()` directly.
- **CSS**: Tailwind v4 with design tokens, dark mode via `data-theme="dark"`. Hardcoded hex colors, magic z-indices, and `style={{}}` attributes are smells — use CSS custom properties.
- **DTOs**: write transport DTOs with `#[serde(rename_all = "camelCase")]` from the first draft. Don't reactively add it after test failures.
- **No business logic in the frontend.** If a check decides "should this action happen," it belongs in `crates/core`.
- **No TODO markers** in source. Track future work in plans, issues, or `BLOCKERS.md` — never in code comments.

See `.claude/rules/coding-standards.md` for full details.

## Per-crate documentation

- `crates/core/README.md` — service catalog, architectural rules, test info.
- `crates/transport-http/README.md` — middleware stack, cloud-mode guard, OpenAPI exposure.
- `crates/transport-cli/README.md` — subcommand catalog, exit codes.
- `crates/transport-veilid/README.md` — pointer to `BLOCKERS.md` (contract-gap enumeration).

## Where to read more

- Architecture rules: `.claude/rules/architecture.md`
- Coding standards: `.claude/rules/coding-standards.md`
- Git workflow: `.claude/rules/git-workflow.md`
- Documentation rules: `.claude/rules/documentation.md`
- Self-hosting (cloud / compose / Helm): `docs/07-deployment/self-hosting.md`
- Threat model & population-specific defenses: `.claude/claudedocs/`

@.claude/claudedocs/web-app-split-master-plan.md
