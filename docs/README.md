# SpiritStream Documentation

[Back to Project](../README.md)

---

SpiritStream is a multi-destination streaming application that lets you stream to YouTube, Twitch, Kick, Facebook, and custom RTMP servers simultaneously. Built with Tauri 2.x, Rust, and React.

---

## Quick Navigation

| I Need To... | Start Here |
|--------------|------------|
| Install and run SpiritStream | [Getting Started](./06-tutorials/01-getting-started.md) |
| Understand the architecture | [System Overview](./01-architecture/01-system-overview.md) |
| Learn about backend services | [`crates/core/README.md`](../crates/core/README.md) |
| Understand frontend state | [State Management](./03-frontend/02-state-management.md) |
| Configure streaming | [FFmpeg Integration](./04-streaming/01-ffmpeg-integration.md) |
| Reference the API | OpenAPI spec at `GET /api/v1/openapi.json` (served by the backend); typed client at [`@spiritstream/api-client`](../packages/api-client/) |
| Deploy with Docker | [Building](./07-deployment/01-building.md#docker-build) |
| Look up a term | [Glossary](./GLOSSARY.md) |

---

## Project Statistics

| Metric | Value |
|--------|-------|
| **Framework** | Tauri 2.x + Axum |
| **Backend** | Rust workspace — `spiritstream-core` + transport adapters |
| **Frontend** | React 19 + TypeScript 5.9 strict |
| **REST endpoints** | `/api/v1/*` (Axum + utoipa, OpenAPI 3.0) |
| **Supported Platforms** | macOS, Windows, Linux (desktop); iOS, Android (mobile via Tauri 2) |
| **Deployment Modes** | Desktop, Mobile, Docker / self-hosted cloud, headless CLI |
| **Supported Languages** | 11 (af, ar, de, en, es, fr, ja, ko, ru, uk, zh-CN) |
| **Encryption** | AES-256-GCM-SIV under Argon2id; HMAC-SHA256 audit chain |

---

## Reading Paths

### Beginner (2-4 hours)
New to desktop apps or streaming? Start here:

1. [Glossary](./GLOSSARY.md) — Learn key terminology
2. [Getting Started](./06-tutorials/01-getting-started.md) — Install and first run
3. [First Stream](./06-tutorials/02-first-stream.md) — Set up your first stream
4. [System Overview](./01-architecture/01-system-overview.md) — High-level concepts

### Intermediate (4-8 hours)
Comfortable with React and TypeScript? Go deeper:

5. [React Architecture](./03-frontend/01-react-architecture.md)
6. [State Management](./03-frontend/02-state-management.md)
7. [FFmpeg Integration](./04-streaming/01-ffmpeg-integration.md)
8. [Multi-Platform Tutorial](./06-tutorials/03-multi-platform.md)

### Advanced (8-16 hours)
Ready for implementation details and security?

9. [Security Architecture](./01-architecture/04-security-architecture.md)
10. [`crates/core/README.md`](../crates/core/README.md) — service catalog and architectural rules
11. [`crates/transport-http/README.md`](../crates/transport-http/README.md) — HTTP transport, middleware, OpenAPI
12. OpenAPI spec at `/api/v1/openapi.json` — generated from `utoipa` annotations, consumed by [`@spiritstream/api-client`](../packages/api-client/)

---

## Table of Contents

### Glossary
- [Technical Glossary](./GLOSSARY.md) — 50+ terms and definitions

### Architecture
- [Section Overview](./01-architecture/README.md)
- [System Overview](./01-architecture/01-system-overview.md) — High-level architecture with diagrams
- [Component Architecture](./01-architecture/02-component-architecture.md) — Detailed component breakdown
- [Data Flow](./01-architecture/03-data-flow.md) — Data flow and sequence diagrams
- [Security Architecture](./01-architecture/04-security-architecture.md) — Security model, encryption, Tauri permissions

### Backend (Rust)

The backend is a Cargo workspace with per-crate READMEs. Read those, plus the architecture rules, instead of section docs:

- [Architecture rules](../.claude/rules/architecture.md) — layered architecture, service catalog, deployment modes
- [`crates/core/README.md`](../crates/core/README.md) — transport-agnostic library: services, models, traits, errors
- [`crates/transport-http/README.md`](../crates/transport-http/README.md) — Axum + utoipa adapter, middleware, cloud-mode guard
- [`crates/transport-cli/README.md`](../crates/transport-cli/README.md) — `spiritstream-cli` subcommand catalog, exit codes
- [`crates/transport-veilid/README.md`](../crates/transport-veilid/README.md) — contract-validation spike (see `BLOCKERS.md`)

### Frontend (React)
- [Section Overview](./03-frontend/README.md)
- [React Architecture](./03-frontend/01-react-architecture.md) — Component hierarchy and patterns
- [State Management](./03-frontend/02-state-management.md) — Zustand stores (profile, stream, theme)
- [Component Library](./03-frontend/03-component-library.md) — UI components with props and usage
- [Tauri Integration](./03-frontend/04-tauri-integration.md) — IPC patterns and api wrapper
- [Theming and i18n](./03-frontend/05-theming-i18n.md) — Theme system and internationalization

### Streaming
- [Section Overview](./04-streaming/README.md)
- [FFmpeg Integration](./04-streaming/01-ffmpeg-integration.md) — Process management, relay architecture
- [RTMP Fundamentals](./04-streaming/02-rtmp-fundamentals.md) — Protocol basics for streaming
- [Multi-Destination](./04-streaming/03-multi-destination.md) — Output groups and target management
- [Encoding Reference](./04-streaming/04-encoding-reference.md) — Codecs, presets, hardware acceleration

### API Reference

The REST surface is auto-documented from `utoipa` annotations on every handler; the typed TypeScript client is generated from that spec by `@hey-api/openapi-ts`. There is no hand-written API reference to keep in sync.

- **OpenAPI spec**: `GET /api/v1/openapi.json` (served by the backend at runtime)
- **Typed client**: [`@spiritstream/api-client`](../packages/api-client/)
- **Domain types**: [`@spiritstream/types`](../packages/types/) — generated from Rust via `ts-rs`
- **Error model**: `CoreError` enum in [`crates/core/src/errors.rs`](../crates/core/src/errors.rs); HTTP mapping in [`crates/transport-http/src/lib.rs`](../crates/transport-http/src/lib.rs)
- **CLI surface**: [`crates/transport-cli/README.md`](../crates/transport-cli/README.md) — subcommand catalog and exit codes

### Tutorials
- [Section Overview](./06-tutorials/README.md)
- [Getting Started](./06-tutorials/01-getting-started.md) — Installation on all platforms
- [First Stream](./06-tutorials/02-first-stream.md) — Basic streaming setup
- [Multi-Platform](./06-tutorials/03-multi-platform.md) — Streaming to multiple services
- [Custom Encoding](./06-tutorials/04-custom-encoding.md) — Advanced encoding configuration
- [Contributing](./06-tutorials/05-contributing.md) — Development setup and code style

### Deployment
- [Section Overview](./07-deployment/README.md)
- [Building](./07-deployment/01-building.md) — Build process documentation
- [Platform Guides](./07-deployment/02-platform-guides.md) — Windows, macOS, Linux specifics
- [Distribution Strategy](./07-deployment/03-distribution-strategy.md) — Desktop, Docker, Cloud
- [Release Process](./07-deployment/04-release-process.md) — Versioning and distribution

---

## Technology Stack

```text
┌─────────────────────────────────────────────────────────────────────┐
│  CLIENT LAYER                                                       │
│    Tauri 2 desktop  │  Tauri 2 mobile  │  Web browser  │  CLI       │
│    (sidecar HTTP)   │  (in-process)    │  (HTTP)       │  (in-proc) │
├─────────────────────────────────────────────────────────────────────┤
│  TRANSPORT LAYER                                                    │
│    transport-http  (Axum + utoipa, REST /api/v1/*, WS /api/v1/events)│
│    transport-cli   (in-process dispatch, JSON / --pretty / --quiet) │
│    transport-veilid (contract spike — see crates/.../BLOCKERS.md)   │
├─────────────────────────────────────────────────────────────────────┤
│  CORE LAYER — spiritstream-core (transport-agnostic library)        │
│    services/   ProfileService, StreamService, ChatService,          │
│                ObsService, OAuthService, SettingsService,           │
│                SafetyService, AuditLogService, …                    │
│    traits/     Transport, SecretStore, EventSink, MediaProcessor,   │
│                IdentityProvider, Clock                              │
│    models/     ts-rs-derived domain types → @spiritstream/types     │
├─────────────────────────────────────────────────────────────────────┤
│  INFRASTRUCTURE                                                     │
│    FFmpeg processes (desktop)  │  Native encoders (mobile follow-up)│
│    Keyring OR encrypted-file secret store (chosen once at startup)  │
│    AES-256-GCM-SIV envelope  │  HMAC-SHA256 audit chain             │
└─────────────────────────────────────────────────────────────────────┘
```

---

## Deployment Modes

| Mode | Use Case | Setup |
|------|----------|-------|
| **Desktop** | Local streaming with GPU acceleration | Download installer |
| **Mobile** | iOS / Android via Tauri 2 | AltStore PAL (EU/JP) · F-Droid · direct APK |
| **Docker / self-hosted cloud** | Single-tenant on your own VPS or home server | `docker compose` + Caddy + Let's Encrypt; see [self-hosting](./07-deployment/self-hosting.md) |
| **CLI** | Scriptable, headless management | `cargo run -p spiritstream-cli` |

See [self-hosting guide](./07-deployment/self-hosting.md) for cloud deploys; mobile distribution notes live in the rewrite plan and `apps/tauri/`.

---

## Diagrams

All Mermaid diagrams use a dark theme:

- **Background:** `#0F0A14` (deep purple-black)
- **Primary:** `#7C3AED` / `#A78BFA` (violet)
- **Text:** `#F4F2F7` (off-white)

---

## Code References

Source code links use the format: [`filename.rs:line`](../path/to/file.rs#L123)

---

## Contributing

See [Contributing Guide](./06-tutorials/05-contributing.md) for documentation style and review process.

