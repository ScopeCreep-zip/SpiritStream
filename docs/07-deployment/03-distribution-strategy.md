# Distribution Strategy: Desktop Launcher, Docker, Cloud

This document captures the business plan and a detailed implementation plan for a three-mode distribution model:

1) Desktop (Tauri launcher + local host server + local GPU)
2) Docker (self-hosted)
3) Cloud (managed SaaS)

This is the primary coordination doc for multi-developer work. Keep it updated as the implementation evolves.

---

## Business Plan Summary

We ship one codebase, three deployment paths, and a clear upgrade path:

- Desktop: free, local GPU performance, simplest for creators.
- Docker: free, open-source self-hosted for power users and teams.
- Cloud: paid managed service for businesses and enterprise.

This creates a funnel: creators adopt the free desktop build, teams and power users move to self-hosted, and enterprises upgrade to managed SaaS for SLAs, SSO, and compliance.

---

## Product Tiers and Target Users

Desktop (Tauri launcher)
- Target: creators, streamers, gamers.
- Value: best GPU performance, low friction install.
- Pricing: free.

Docker (Self-hosted)
- Target: homelabbers, devs, privacy-focused, cost-conscious.
- Value: control + GPU passthrough + easy deployment.
- Pricing: free (OSS).

Cloud (Managed)
- Target: creators with teams, businesses, agencies, enterprise.
- Value: no-ops, SLAs, managed updates, enterprise security.
- Pricing: paid.

---

## Guiding Principles

- Keep the FFmpeg and streaming logic unchanged.
- Decouple frontend and backend via HTTP/WS everywhere.
- Avoid multiple codepaths for local vs remote behavior.
- Design for future auth and multi-tenant data stores.
- Keep the desktop build fast and reliable.

---

## System Architecture Overview

ASCII diagram:

    +--------------------+        HTTP/WS         +-----------------------+
    |  UI (React/Vite)   | <--------------------> | Host Server (Rust)    |
    |  Tauri Webview     |                         | FFmpeg + Profiles     |
    +--------------------+                         +-----------------------+
             |                                                    |
             | local GPU / FFmpeg                                |
             v                                                    v
    +--------------------+                         +-----------------------+
    | Desktop Launcher   |                         | Profiles/Logs/Themes  |
    | (Tauri shell)      |                         | Local or Remote Store |
    +--------------------+                         +-----------------------+

---

## Deployment Modes

### 1) Desktop (Launcher)

Behavior:
- Always include the HTTP/WS host server.
- Tauri UI continues to use native commands; HTTP/WS exists for remote access.
- Host server is local and uses local GPU/FFmpeg.
- Optional toggle enables remote access (host bind + token).
- Optional toggle enables serving the web UI assets from the host.

Default config:
- Base URL: http://127.0.0.1:8008
- WS URL: ws://127.0.0.1:8008/ws

### 2) Docker (Self-hosted)

Behavior:
- Same host server, runs in container.
- Serves static UI or external UI.
- Volume mounts for data/logs/themes.
- Optional GPU passthrough for encoding.

### 3) Cloud (Managed)

Behavior:
- Same API surface.
- Auth/SSO and multi-tenant store.
- Workers for FFmpeg jobs (GPU pool).
- UI served from the cloud.

---

## Implementation Plan (Detailed)

### Phase 0: Documentation and Coordination

- Maintain this doc and update with changes.
- Document settings, env vars, and default behavior.
- Document API contract and event channels.

### Phase 1: Desktop Launcher + Local Host

1) Always run the HTTP/WS host server in the desktop build.
2) Keep Tauri UI on the native command path; HTTP/WS is for remote web clients.
3) Add UI + settings for:
   - Remote access toggle (binds to localhost when off).
   - Serve web UI toggle (disabled by default for security).
   - Host bind address (default 127.0.0.1).
   - Host port (default 8008).
   - Optional dev token (env var or config).
4) Update Tauri CSP to allow HTTP/WS to configured host.
5) Add health checks and status indicator in the UI.

### Phase 2: Docker Distribution

1) Dockerfile for host server + static UI.
2) Support environment configuration:
   - SPIRITSTREAM_HOST
   - SPIRITSTREAM_PORT
   - SPIRITSTREAM_DATA_DIR
   - SPIRITSTREAM_LOG_DIR
   - SPIRITSTREAM_THEMES_DIR
   - SPIRITSTREAM_UI_DIR
   - SPIRITSTREAM_API_TOKEN (optional)
3) Document GPU passthrough.
4) Provide docker-compose example with volumes.

### Phase 3: Cloud Roadmap

1) Replace local file store with a storage abstraction (e.g., object store or Postgres).
2) Add auth + multi-tenant IDs in request context.
3) Break out streaming job execution to worker nodes.
4) Add observability, rate limiting, and audit logging.

---

## Auth (Short-Term vs Long-Term)

Short-term:
- Optional shared token in env var or settings for HTTP server.
- Header-based: `Authorization: Bearer <token>`.
- WebSocket uses `?token=` query parameter.
- Enforced only when token is configured.

Long-term:
- Full auth layer (SSO/OIDC) and RBAC.
- Token rotation and audit logging.

---

## Configuration Sources

Order of precedence:
1) Environment variables (highest priority)
2) Settings file (for desktop/launcher)
3) Defaults

Recommended defaults:
- Host: 127.0.0.1
- Port: 8008
- UI dir: ./dist (for server)

---

## Workstreams and Ownership

Workstream A: Desktop + Launcher
- Host process lifecycle and status
- Tauri CSP updates
- Settings toggle UI

Workstream B: Server + API
- Token auth and CORS policy
- Health and status endpoints
- Logging and telemetry hooks

Workstream C: Docker
- Dockerfile and compose
- GPU passthrough docs
- Container environment variables

Workstream D: Cloud Roadmap
- Multi-tenant design
- Storage abstraction
- Worker execution model

---

## Decisions Made

These questions have been resolved:

| Question | Decision | Rationale |
|----------|----------|-----------|
| Remote access default binding | `localhost:8008` (opt-in for 0.0.0.0) | Security-first, user must explicitly enable remote access |
| Default bind address | User-configurable, default `127.0.0.1` | Allow users to set binding in settings |
| UI in desktop mode | Tauri webview (embedded) | Best performance, native feel |
| Host server disable-able? | No, always runs | Simplifies architecture, enables remote access |
| Token sharing flow | Simple: copy token from settings | Full pairing flow handled by external auth work |
| Host status in UI | Connection indicator planned | Shows connected/disconnected state |

## Open Questions

- Exact CORS policy for production (currently permissive)
- Whether to expose host logs separately from app logs
- Container orchestration health check intervals

---

## Notes for Multi-Developer Coordination

- Keep frontend backend-agnostic and driven by `VITE_BACKEND_*`.
- Keep host server API surface stable across deployments.
- Avoid any backend-only features that cannot be reached over HTTP.
- Keep FFmpeg integration unchanged.

---

## Diagrams

Deployment matrix:

    +----------------------+--------------------+----------------------+
    | Desktop (Tauri)      | Docker             | Cloud                |
    +----------------------+--------------------+----------------------+
    | Local GPU            | Your GPU / CPU     | Managed GPU pool     |
    | Local profiles       | Volume-mounted     | Multi-tenant store   |
    | Local HTTP/WS        | Public HTTP/WS     | Public HTTP/WS       |
    | Free                | Free (OSS)         | Paid (SaaS)          |
    +----------------------+--------------------+----------------------+

Request flow:

    UI -> HTTP POST /api/invoke/* -> Host Server -> Services -> Storage
    UI <- WS events (stream_stats, log://log, themes_updated)
