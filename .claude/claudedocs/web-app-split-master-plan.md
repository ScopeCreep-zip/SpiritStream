# SpiritStream Web App Split - Master Implementation Plan

> **Status**: Implementation In Progress
> **Branch**: `web-app-split`
> **Last Updated**: 2026-01-16
> **Primary Coordination Document** for multi-developer work

---

## Table of Contents

1. [Executive Summary](#1-executive-summary)
2. [Business Rationale](#2-business-rationale)
3. [Architecture Overview](#3-architecture-overview)
4. [Implementation Status](#4-implementation-status)
5. [Detailed Implementation Plan](#5-detailed-implementation-plan)
6. [Work Breakdown Structure](#6-work-breakdown-structure)
7. [API Reference](#7-api-reference)
8. [Configuration Reference](#8-configuration-reference)
9. [Security Model](#9-security-model)
10. [Testing Strategy](#10-testing-strategy)
11. [Future Roadmap](#11-future-roadmap)

---

## 1. Executive Summary

### What We're Building

SpiritStream is transitioning from a monolithic Tauri desktop app to a **host process + client architecture**. The core streaming workload (FFmpeg, RTMP relay, encoding) runs in an installed host binary, while the UI can run as:

- Embedded Tauri webview (desktop)
- Standalone web browser (remote access)
- Docker container (self-hosted)
- Cloud service (future SaaS)

### Why This Architecture

1. **Best Performance**: Local GPU/Metal acceleration for streaming
2. **Minimal Novel Code**: Keep FFmpeg integration unchanged
3. **Decoupled Workloads**: Frontend and backend can evolve independently
4. **Future-Proof**: Ready for Veilid network protocol integration at client level
5. **Distribution Flexibility**: One codebase, multiple deployment modes

### Key Decisions Made

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Host binding default | `localhost:8008` | Security-first, opt-in remote access |
| Remote access | User-configurable | Allow binding to `0.0.0.0` via settings |
| Auth (short-term) | Single Bearer token | Simple, env var or settings, someone else handling full auth later |
| UI serving | Toggle in settings | Disabled by default for security |
| FFmpeg changes | None | Preserve working streaming logic |

---

## 2. Business Rationale

### Three-Tier Distribution Model

```
+----------------------+--------------------+----------------------+
| Desktop (Tauri)      | Docker             | Cloud                |
+----------------------+--------------------+----------------------+
| Local GPU            | Your GPU / CPU     | Managed GPU pool     |
| Local profiles       | Volume-mounted     | Multi-tenant store   |
| Local HTTP/WS        | Public HTTP/WS     | Public HTTP/WS       |
| Free                 | Free (OSS)         | Paid (SaaS)          |
+----------------------+--------------------+----------------------+
```

### Target Users by Tier

**Desktop (Free)**
- Streamers, content creators, gamers
- Value: Best GPU performance, simple install
- Friction: None - download and run

**Docker (Free OSS)**
- Homelabbers, developers, privacy-focused users
- Value: Full control, GPU passthrough, self-hosted
- Friction: Moderate - requires Docker knowledge

**Cloud (Paid SaaS)**
- Teams, businesses, agencies, enterprise
- Value: No-ops, SLAs, managed updates, SSO, compliance
- Friction: None - sign up and stream

### Upgrade Path

```
Creator downloads Desktop → Power user self-hosts Docker → Enterprise upgrades to Cloud
```

All three tiers use the **same HTTP/WS API surface**, enabling seamless migration.

---

## 3. Architecture Overview

### System Diagram

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              CLIENT LAYER                                    │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│   ┌─────────────────────┐    ┌─────────────────────┐                       │
│   │  Tauri Desktop      │    │  Web Browser        │                       │
│   │  (Embedded Webview) │    │  (Remote Access)    │                       │
│   │                     │    │                     │                       │
│   │  React + Vite       │    │  React + Vite       │                       │
│   │  Auto-detects Tauri │    │  HTTP mode          │                       │
│   └─────────┬───────────┘    └─────────┬───────────┘                       │
│             │                          │                                   │
│             │    HTTP/WS API           │                                   │
│             └──────────┬───────────────┘                                   │
│                        │                                                   │
├────────────────────────┼───────────────────────────────────────────────────┤
│                        ▼                                                   │
│   ┌─────────────────────────────────────────────────────────────────────┐ │
│   │                     HOST SERVER (Rust + Axum)                        │ │
│   │                                                                      │ │
│   │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐              │ │
│   │  │  HTTP Router │  │  WebSocket   │  │  Static UI   │              │ │
│   │  │  /api/invoke │  │  /ws events  │  │  (optional)  │              │ │
│   │  └──────┬───────┘  └──────┬───────┘  └──────────────┘              │ │
│   │         │                  │                                        │ │
│   │         ▼                  ▼                                        │ │
│   │  ┌─────────────────────────────────────────────────────────────┐   │ │
│   │  │                    SERVICE LAYER                             │   │ │
│   │  │  ProfileManager │ FFmpegHandler │ SettingsManager │ Themes   │   │ │
│   │  └─────────────────────────────────────────────────────────────┘   │ │
│   │                              │                                      │ │
│   │                              ▼                                      │ │
│   │  ┌─────────────────────────────────────────────────────────────┐   │ │
│   │  │                    FFmpeg LAYER                              │   │ │
│   │  │  RTMP Relay │ Encoding Processes │ Stream Stats              │   │ │
│   │  └─────────────────────────────────────────────────────────────┘   │ │
│   └─────────────────────────────────────────────────────────────────────┘ │
│                              │                                             │
│                              ▼                                             │
│   ┌─────────────────────────────────────────────────────────────────────┐ │
│   │                    STORAGE LAYER                                     │ │
│   │  Profiles (JSON) │ Settings (JSON) │ Logs │ Themes                  │ │
│   └─────────────────────────────────────────────────────────────────────┘ │
│                                                                             │
│                              HOST LAYER                                     │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Component Responsibilities

| Component | Responsibility | Location |
|-----------|----------------|----------|
| **Tauri Launcher** | Spawns host server, health check, opens UI | `src-tauri/src/launcher.rs` |
| **Host Server** | HTTP/WS API, service orchestration | `src-tauri/src/bin/server.rs` |
| **FFmpegHandler** | RTMP relay, encoding, process management | `src-tauri/src/services/ffmpeg_handler.rs` |
| **ProfileManager** | Profile CRUD, encryption | `src-tauri/src/services/profile_manager.rs` |
| **Frontend** | UI, state management, API abstraction | `src-frontend/` |
| **Backend Abstraction** | Transport-agnostic API calls | `src-frontend/lib/backend/` |

### Data Flow

```
UI Action → Backend Abstraction → HTTP POST /api/invoke/{command} → Host Server
                                                                         │
                                                                         ▼
UI Update ← WebSocket Event ← Event Bus ← Service Layer ← FFmpeg Process
```

---

## 4. Implementation Status

### Completed (as of 2026-01-16)

| Component | Status | Notes |
|-----------|--------|-------|
| HTTP Server (`server.rs`) | ✅ Complete | 570 lines, Axum-based, all commands mapped |
| Launcher (`launcher.rs`) | ✅ Complete | 208 lines, sidecar spawning, health check |
| Token Auth | ✅ Complete | Bearer header + WS query param |
| Settings Model | ✅ Complete | New fields for remote access |
| Settings UI | ✅ Complete | Remote Access card in Settings view |
| Backend Abstraction | ✅ Complete | `env.ts`, `httpApi.ts`, `httpEvents.ts` |
| Mode Auto-Detection | ✅ Complete | Tauri vs HTTP detection |
| WebSocket Events | ✅ Complete | Event broadcasting to all clients |
| CORS | ✅ Complete | Permissive for development |
| Health Endpoint | ✅ Complete | `GET /health` |

### In Progress

| Component | Status | Owner | Notes |
|-----------|--------|-------|-------|
| Docker Distribution | ✅ Complete | - | Dockerfile, compose, README in `docker/` |
| Full Auth System | 🔄 External | Another developer | SSO, RBAC, token rotation |
| UI Serving Toggle | ✅ Settings exist | - | Server-side implementation complete |
| Sidecar Configuration | ✅ Complete | - | `build-server.ts` script, `tauri.conf.json` updated |

### Not Started

| Component | Priority | Notes |
|-----------|----------|-------|
| Cloud Distribution | Future | Multi-tenant, storage abstraction |
| Veilid Integration | Future | Client-level network protocol |
| Windows Service Mode | Low | Auto-start without UI |

---

## 5. Detailed Implementation Plan

### Phase 0: Documentation (Current)

**Goal**: Complete coordination documentation for multi-developer work.

- [x] Business rationale documented
- [x] Architecture diagrams
- [x] API reference
- [x] Configuration reference
- [x] Work breakdown structure

### Phase 1: Desktop Launcher + Local Host (Mostly Complete)

**Goal**: Desktop app always runs host server internally.

| Task | Status | Details |
|------|--------|---------|
| Host server binary | ✅ Done | `src-tauri/src/bin/server.rs` |
| Launcher spawns server | ✅ Done | `src-tauri/src/launcher.rs` |
| Health check on startup | ✅ Done | Waits for `/health` before proceeding |
| Settings for remote access | ✅ Done | `backend_host`, `backend_port`, `backend_token` |
| Settings UI | ✅ Done | Remote Access card in Settings view |
| CSP updates | 🔄 Verify | May need adjustment for HTTP to localhost |

**Remaining Work**:
1. Test end-to-end desktop flow
2. Verify CSP allows HTTP to configured host
3. Add host status indicator in UI (optional)

### Phase 2: Web Client Mode (Complete)

**Goal**: Frontend works standalone via HTTP without Tauri.

| Task | Status | Details |
|------|--------|---------|
| Backend mode detection | ✅ Done | `VITE_BACKEND_MODE` or auto-detect |
| HTTP API wrapper | ✅ Done | `httpApi.ts` mirrors Tauri commands |
| WebSocket handler | ✅ Done | `httpEvents.ts` with auto-reconnect |
| Dialog abstraction | ✅ Done | `httpDialogs.ts` for file dialogs |
| Token handling | ✅ Done | Header + URL query param + localStorage |

**Remaining Work**:
1. Test remote access flow end-to-end
2. Document browser-based usage

### Phase 3: Docker Distribution (Not Started)

**Goal**: Containerized host server for self-hosted deployment.

| Task | Status | Owner |
|------|--------|-------|
| Dockerfile | ❌ | TBD |
| docker-compose.yml | ❌ | TBD |
| Environment variable docs | ✅ | `.env.example` exists |
| Volume mount docs | ❌ | TBD |
| GPU passthrough docs | ❌ | TBD |
| Health check for orchestrators | ✅ | `/health` endpoint exists |

### Phase 4: UI Serving (Partial)

**Goal**: Host server can optionally serve static UI files.

| Task | Status | Details |
|------|--------|---------|
| Settings toggle | ✅ Done | `backend_ui_enabled` |
| Server static file serving | ✅ Done | `ServeDir` in server.rs |
| Build UI into dist | ✅ Done | Vite build |
| Package UI with Tauri | 🔄 Verify | Resource bundling |

### Phase 5: Cloud Roadmap (Future)

**Goal**: Managed SaaS with multi-tenancy.

| Task | Priority | Notes |
|------|----------|-------|
| Storage abstraction | High | Replace file system with object store/Postgres |
| Multi-tenant auth | High | SSO/OIDC, tenant ID in request context |
| Worker execution | Medium | Break out FFmpeg to worker nodes |
| Observability | Medium | Metrics, tracing, rate limiting |
| Audit logging | Medium | Compliance requirements |

---

## 6. Work Breakdown Structure

### Workstream A: Desktop Integration

**Owner**: TBD
**Focus**: Tauri launcher, desktop UX, packaging

| Task ID | Task | Estimate | Dependencies |
|---------|------|----------|--------------|
| A1 | Verify CSP allows HTTP to localhost | 1h | None |
| A2 | Test launcher end-to-end on Windows | 2h | None |
| A3 | Test launcher end-to-end on macOS | 2h | None |
| A4 | Test launcher end-to-end on Linux | 2h | None |
| A5 | Add host status indicator in UI | 4h | None |
| A6 | Handle launcher errors gracefully | 2h | A2-A4 |
| A7 | Package server binary as sidecar | 2h | None |

### Workstream B: Server + API

**Owner**: TBD
**Focus**: HTTP server, API stability, performance

| Task ID | Task | Estimate | Dependencies |
|---------|------|----------|--------------|
| B1 | Audit all command mappings | 2h | None |
| B2 | Add request logging middleware | 2h | None |
| B3 | Add rate limiting (optional) | 4h | None |
| B4 | Add API versioning (future-proof) | 2h | None |
| B5 | Load testing | 4h | B1 |
| B6 | Document API errors | 2h | B1 |

### Workstream C: Docker Distribution

**Owner**: TBD
**Focus**: Containerization, self-hosted deployment

| Task ID | Task | Estimate | Dependencies |
|---------|------|----------|--------------|
| C1 | Create Dockerfile | 4h | None |
| C2 | Create docker-compose.yml | 2h | C1 |
| C3 | Document volume mounts | 2h | C2 |
| C4 | Document GPU passthrough (NVIDIA) | 4h | C1 |
| C5 | Document GPU passthrough (AMD) | 4h | C1 |
| C6 | Test container on Linux | 2h | C2 |
| C7 | Test container on Windows (WSL) | 2h | C2 |
| C8 | Publish to Docker Hub / GHCR | 2h | C6-C7 |

### Workstream D: Frontend Polish

**Owner**: TBD
**Focus**: UI improvements for remote access experience

| Task ID | Task | Estimate | Dependencies |
|---------|------|----------|--------------|
| D1 | Connection status indicator | 4h | None |
| D2 | Reconnection UX (toast, overlay) | 4h | D1 |
| D3 | Token input flow for remote | 2h | None |
| D4 | Backend URL configuration in UI | 4h | None |
| D5 | Offline mode handling | 4h | D1 |

### Workstream E: Auth (External)

**Owner**: Another developer
**Focus**: Full authentication system

| Task ID | Task | Estimate | Dependencies |
|---------|------|----------|--------------|
| E1 | Define auth requirements | - | External |
| E2 | Implement auth layer | - | E1 |
| E3 | Token rotation | - | E2 |
| E4 | SSO/OIDC integration | - | E2 |
| E5 | RBAC | - | E2 |

---

## 7. API Reference

### HTTP Endpoints

All commands via `POST /api/invoke/{command}`:

```
Authorization: Bearer <token>
Content-Type: application/json

POST /api/invoke/{command}
Body: { ...parameters }
```

### Command Categories

#### Profile Commands
| Command | Parameters | Returns |
|---------|------------|---------|
| `get_all_profiles` | - | `string[]` |
| `load_profile` | `{ name, password? }` | `Profile` |
| `save_profile` | `{ profile, password? }` | `void` |
| `delete_profile` | `{ name }` | `void` |
| `is_profile_encrypted` | `{ name }` | `boolean` |
| `validate_input` | `{ profileId, input }` | `ValidationResult` |

#### Stream Commands
| Command | Parameters | Returns |
|---------|------------|---------|
| `start_stream` | `{ group, incomingUrl }` | `void` |
| `start_all_streams` | `{ groups[], incomingUrl }` | `void` |
| `stop_stream` | `{ groupId }` | `void` |
| `stop_all_streams` | - | `void` |
| `get_active_stream_count` | - | `number` |
| `is_group_streaming` | `{ groupId }` | `boolean` |
| `get_active_group_ids` | - | `string[]` |
| `toggle_stream_target` | `{ targetId, enabled, group, incomingUrl }` | `void` |
| `is_target_disabled` | `{ targetId }` | `boolean` |

#### System Commands
| Command | Parameters | Returns |
|---------|------------|---------|
| `get_encoders` | - | `Encoders` |
| `test_ffmpeg` | - | `TestResult` |
| `validate_ffmpeg_path` | `{ path }` | `ValidationResult` |
| `get_recent_logs` | `{ maxLines? }` | `LogEntry[]` |
| `export_logs` | `{ path, content }` | `void` |

#### Settings Commands
| Command | Parameters | Returns |
|---------|------------|---------|
| `get_settings` | - | `Settings` |
| `save_settings` | `{ settings }` | `void` |
| `get_profiles_path` | - | `string` |
| `export_data` | `{ exportPath }` | `void` |
| `clear_data` | - | `void` |
| `rotate_machine_key` | - | `void` |

#### FFmpeg Commands
| Command | Parameters | Returns |
|---------|------------|---------|
| `download_ffmpeg` | - | `void` |
| `cancel_ffmpeg_download` | - | `void` |
| `get_bundled_ffmpeg_path` | - | `string` |
| `check_ffmpeg_update` | `{ installedVersion? }` | `UpdateInfo` |

#### Theme Commands
| Command | Parameters | Returns |
|---------|------------|---------|
| `list_themes` | - | `Theme[]` |
| `refresh_themes` | - | `void` |
| `get_theme_tokens` | `{ themeId }` | `ThemeTokens` |
| `install_theme` | `{ themePath }` | `void` |

### WebSocket Events

Connect to `GET /ws?token=<token>`:

```typescript
// Event format
{
  "event": "event_name",
  "payload": { ... }
}

// Events
- "stream_stats": { bitrate, fps, uptime, droppedFrames, ... }
- "log://log": { level, message, target }
- "theme_updated": { themeId, tokens }
- "ffmpeg_download_progress": { progress, total, status }
```

### Response Format

```typescript
// Success
{ "ok": true, "data": <result>, "error": null }

// Error
{ "ok": false, "data": null, "error": "Error message" }
```

---

## 8. Configuration Reference

### Environment Variables

#### Frontend (Vite)

| Variable | Default | Description |
|----------|---------|-------------|
| `VITE_BACKEND_MODE` | auto-detect | `tauri` or `http` |
| `VITE_BACKEND_URL` | `http://127.0.0.1:8008` | Base URL for HTTP mode |
| `VITE_BACKEND_WS_URL` | derived from URL | WebSocket URL |
| `VITE_BACKEND_TOKEN` | - | Auth token |

#### Backend Server

| Variable | Default | Description |
|----------|---------|-------------|
| `SPIRITSTREAM_HOST` | `127.0.0.1` | Bind address |
| `SPIRITSTREAM_PORT` | `8008` | HTTP port |
| `SPIRITSTREAM_DATA_DIR` | platform-specific | Profile storage |
| `SPIRITSTREAM_LOG_DIR` | `$DATA_DIR/logs` | Log directory |
| `SPIRITSTREAM_THEMES_DIR` | `./themes` | Theme directory |
| `SPIRITSTREAM_UI_DIR` | `./dist` | Static UI directory |
| `SPIRITSTREAM_UI_ENABLED` | `0` | Enable UI serving |
| `SPIRITSTREAM_API_TOKEN` | - | Auth token |

#### Launcher

| Variable | Default | Description |
|----------|---------|-------------|
| `SPIRITSTREAM_UI_URL` | derived | UI URL to open |
| `SPIRITSTREAM_SERVER_PATH` | sidecar | Server binary path |
| `SPIRITSTREAM_LAUNCHER_HIDE_WINDOW` | `0` | Hide launcher window |
| `SPIRITSTREAM_LAUNCHER_OPEN_EXTERNAL` | `0` | Open in browser |

### Settings File Fields

```typescript
interface Settings {
  // General
  language: string;
  start_minimized: boolean;
  show_notifications: boolean;

  // FFmpeg
  ffmpeg_path: string;
  auto_download_ffmpeg: boolean;

  // Data & Privacy
  encrypt_stream_keys: boolean;
  log_retention_days: number;
  theme_id: string;

  // Remote Access (NEW)
  backend_remote_enabled: boolean;  // false = localhost only
  backend_ui_enabled: boolean;      // false = no static UI
  backend_host: string;             // default: 127.0.0.1
  backend_port: number;             // default: 8008
  backend_token: string;            // optional auth token

  last_profile: string | null;
}
```

### Precedence Order

1. Environment variables (highest)
2. Settings file
3. Hardcoded defaults (lowest)

---

## 9. Security Model

### Current (Short-Term) Auth

**Mechanism**: Optional single Bearer token

**Configuration**:
- Environment: `SPIRITSTREAM_API_TOKEN`
- Settings: `backend_token` field
- Enforced only when token is set

**Usage**:
```
# HTTP
Authorization: Bearer <token>

# WebSocket
ws://host:port/ws?token=<token>
```

**Behavior**:
- No token configured = open API (local use case)
- Token configured = all endpoints require valid token
- Invalid token = 401 Unauthorized

### Remote Access Controls

| Setting | Default | Effect |
|---------|---------|--------|
| `backend_remote_enabled` | `false` | When false, binds to `127.0.0.1` only |
| `backend_ui_enabled` | `false` | When false, no static file serving |
| `backend_token` | empty | When set, requires auth |

### Encryption

- Profile encryption: AES-256-GCM
- Key derivation: Argon2id
- Stream keys encrypted at rest when `encrypt_stream_keys` is true
- Machine key for key encryption (rotatable)

### Sanitization

- Platform-aware stream key redaction in logs
- Generic pattern matching fallback
- Prevents accidental credential exposure

### Future Auth (External Work)

- Full SSO/OIDC integration
- RBAC for team access
- Token rotation
- Audit logging

---

## 10. Testing Strategy

### Unit Tests

| Area | Location | Coverage |
|------|----------|----------|
| Backend services | `src-tauri/src/services/` | TBD |
| Frontend stores | `src-frontend/stores/` | TBD |
| API abstraction | `src-frontend/lib/backend/` | TBD |

### Integration Tests

| Scenario | Method | Priority |
|----------|--------|----------|
| Desktop startup flow | Manual | High |
| Remote access flow | Manual | High |
| Docker deployment | Manual | Medium |
| Profile encryption | Automated | High |
| Stream start/stop | Manual | High |

### End-to-End Tests

| Scenario | Method |
|----------|--------|
| Complete streaming workflow | Manual |
| Multi-platform output | Manual |
| Remote client connection | Manual |

### Test Environments

| Environment | Configuration |
|-------------|---------------|
| Local Desktop | Default settings |
| Local HTTP | `VITE_BACKEND_MODE=http` |
| Docker | Container with volumes |
| Remote | Separate host + client |

---

## 11. Future Roadmap

### Near-Term (Current Sprint)

1. Complete documentation
2. Test desktop flow on all platforms
3. Docker distribution basics

### Medium-Term (Next Sprint)

1. Full Docker with GPU passthrough docs
2. UI polish for remote access
3. Connection status indicators

### Long-Term

1. **Veilid Integration**: Client-level network protocol for decentralized access
2. **Cloud Distribution**: Multi-tenant SaaS with managed infrastructure
3. **FFmpeg Alternatives**: Potential re-implementation for specific use cases
4. **Mobile Clients**: iOS/Android remote control apps

---

## Related Documents

- [Distribution Strategy](../docs/07-deployment/03-distribution-strategy.md)
- [API Reference](../docs/05-api-reference/01-commands-api.md)
- [Architecture Overview](./architecture-new.md)
- [Migration Status](./migration-status.md)

---

*This document is the primary coordination point for the web-app-split implementation. Keep it updated as work progresses.*
