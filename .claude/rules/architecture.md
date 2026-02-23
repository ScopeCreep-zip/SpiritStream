# SpiritStream Architecture

## Monorepo Layout

```
spiritstream/
├── apps/web/          # React frontend (standalone)
├── apps/desktop/      # Tauri 2.x launcher (minimal)
├── server/            # Standalone Rust HTTP server (Axum 0.7)
├── docker/            # Docker distribution
├── themes/            # Theme files
├── scripts/           # Build & utility scripts
└── packages/          # Shared packages (future)
```

## Server (`server/`)

Rust + Axum 0.7 HTTP server. Single binary, no Tauri dependency.

**Entry**: `server/src/main.rs` — route definitions, middleware, server startup

**Routes**:
- `POST /api/invoke/:command` — command dispatch (all business logic)
- `GET /ws` — WebSocket for real-time events
- `GET /health` / `GET /ready` — health checks
- `POST /auth/login` / `POST /auth/logout` / `GET /auth/check` — cookie-based auth
- `GET /api/files/browse` / `GET /api/files/home` / `POST /api/files/open` — file browser

**Services** (`server/src/services/`):
- `profile_manager` — Profile CRUD, encryption
- `ffmpeg_handler` — Stream process management
- `ffmpeg_downloader` — FFmpeg binary download/update
- `encryption` — AES-256-GCM, Argon2id key derivation
- `settings_manager` — App settings persistence
- `theme_manager` / `embedded_themes` — Theme management
- `log_manager` — Logging, log rotation
- `chat_manager` / `chat` — Multi-platform chat integration
- `obs_websocket` — OBS WebSocket integration
- `discord_webhook` — Discord notifications
- `oauth` — OAuth 2.0 for multiple providers
- `events` — Event bus, WebSocket broadcasting
- `path_validator` — Path traversal prevention
- `platform_registry` — Platform-specific utilities

**Models** (`server/src/models/`): Domain types with `#[derive(Serialize, Deserialize, Clone)]`

## Frontend (`apps/web/`)

React 19 + Zustand 5 + Tailwind CSS v4 + Vite 7 + i18next (11 locales)

**Key paths**:
- `src/components/` — React components (ui/, layout/, stream/, modals/)
- `src/stores/` — Zustand stores, one per domain
- `src/lib/backend/` — Transport abstraction layer
- `src/types/` — TypeScript type definitions
- `src/locales/` — i18n translations (af, ar, de, en, es, fr, ja, ko, ru, uk, zh-CN)
- `src/views/` — Page views

**Transport abstraction** (`src/lib/backend/`):
- `api.ts` — selects `httpApi` (default) or `tauriApi` (legacy) based on detected mode
- `httpApi.ts` — `POST /api/invoke/{command}` with `safeFetch()` + retry + cookie auth
- `httpEvents.ts` — WebSocket event handler with auto-reconnect
- `httpDialogs.ts` — File dialog abstraction for HTTP mode
- `env.ts` — Mode detection, URL management, health checks

## Desktop (`apps/desktop/`)

Minimal Tauri 2.x wrapper. **No business logic** — `generate_handler![]` is intentionally empty.

**Entry**: `apps/desktop/src-tauri/src/main.rs`
- Spawns `spiritstream-server` sidecar binary
- Waits for `/health` endpoint
- Opens webview to `http://127.0.0.1:8008`

## Deployment Modes

| Mode | How it runs |
|------|-------------|
| **Desktop** | Tauri launcher spawns server sidecar, UI in embedded webview |
| **Docker** | Server container, UI served or accessed via browser |
| **Web browser** | Remote access to running server instance |
