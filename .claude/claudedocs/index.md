# SpiritStream Documentation Index

Welcome to the SpiritStream documentation. This index provides quick access to all available documentation.

## Quick Start

- [CLAUDE.md](../../CLAUDE.md) - Main project context and overview

## Current Status

**Branch**: `web-app-split`
**Migration**: ✅ **COMPLETE** (Electron fully removed)
**Current Work**: 🔄 **Host Process + Web Client Architecture**
**Production Status**: Tauri 2.x production-ready

## Active Development

| Document | Description |
|----------|-------------|
| [web-app-split-master-plan.md](./web-app-split-master-plan.md) | 📋 **PRIMARY COORDINATION DOC** - Host/client split architecture |
| [Distribution Strategy](../../docs/07-deployment/03-distribution-strategy.md) | Desktop, Docker, Cloud distribution model |

## New Architecture (Target)

| Layer | Technology |
|-------|------------|
| Desktop Framework | Tauri 2.x |
| Backend | Rust |
| Frontend | React 18+ |
| Styling | Tailwind CSS v4 |
| Build Tool | Vite |

## Documentation

### Working Documents

| Directory | Purpose |
|-----------|---------|
| [scratch/](./scratch/) | Temporary working documents, drafts, session notes |
| [research/](./research/) | Reference materials, mockups, external research |

### Architecture & Planning

| Document | Description |
|----------|-------------|
| [roadmap.md](./roadmap.md) | 🚀 **Complete development roadmap** (v0.1 → v3.0) |
| [migration-status.md](./migration-status.md) | ✅ **Migration complete status** |
| [passthrough-architecture.md](./passthrough-architecture.md) | Passthrough-first design with immutable default group |
| [architecture-new.md](./architecture-new.md) | Current Tauri + React architecture |
| [scratch/2026-01-05-critical-fixes.md](./scratch/2026-01-05-critical-fixes.md) | FFmpeg handler race condition fixes |
| [scratch/2026-01-05-css-validation.md](./scratch/2026-01-05-css-validation.md) | CSS value validation implementation |
| [scratch/2026-01-06-theme-production-build.md](./scratch/2026-01-06-theme-production-build.md) | Theme system production build support |
| [scratch/theme-system-review.md](./scratch/theme-system-review.md) | ⭐ **Theme system review** - Grade A (95/100) |
| [themes/README.md](../themes/README.md) | Theme installation and creation guide |
| [themes/dracula.jsonc](../themes/dracula.jsonc) | Dracula theme (purple/pink/cyan) |
| [themes/nord.jsonc](../themes/nord.jsonc) | Nord theme (arctic blue tones) |
| [themes/catppuccin-mocha.jsonc](../themes/catppuccin-mocha.jsonc) | Catppuccin Mocha theme (soothing pastels) |
| [scratch/immutable-default-group.md](./scratch/immutable-default-group.md) | Default group implementation notes |
| [scratch/passthrough-mode-changes.md](./scratch/passthrough-mode-changes.md) | Copy mode implementation |
| [scratch/profile-encoding-removal.md](./scratch/profile-encoding-removal.md) | Profile modal simplification |

### Design System

| Document | Description |
|----------|-------------|
| [design-system.md](./design-system.md) | Design token reference guide |
| [component-library.md](./component-library.md) | React component documentation |
| [ui-specification.md](./ui-specification.md) | Complete UI/UX specification from mockup |
| [pages-and-views.md](./pages-and-views.md) | All 8 views with state and Tauri commands |
| [research/magillastream-complete-design-system.md](./research/magillastream-complete-design-system.md) | Full design system specification |
| [research/magillastream-mockup.html](./research/magillastream-mockup.html) | Interactive HTML mockup |

### Key Features

| Feature | Status |
|---------|--------|
| Profile Management | ✅ Complete (encrypted, password-protected) |
| Output Groups | ✅ Complete (immutable default + custom) |
| Passthrough Mode | ✅ Default (FFmpeg copy mode) |
| Hardware Encoders | ✅ Auto-detection (NVENC, QuickSync, AMF, VideoToolbox) |
| Stream Targets | ✅ Complete (YouTube, Twitch, Kick, Facebook, Custom) |
| FFmpeg Auto-Download | ✅ Complete (with version checking) |
| i18n Support | ✅ Complete (en, de, es, fr, ja) |
| Theme System | ✅ Complete (CSS validation, 3 example themes, light/dark) |
| **HTTP Server** | ✅ Complete (Axum-based, all commands mapped) |
| **WebSocket Events** | ✅ Complete (real-time streaming stats) |
| **Token Auth** | ✅ Complete (Bearer header + WS query param) |
| **Remote Access Settings** | ✅ Complete (host, port, token config in UI) |
| **Backend Abstraction** | ✅ Complete (Tauri/HTTP auto-detection) |
| **Launcher** | ✅ Complete (spawns host server, health check) |
| Docker Distribution | ✅ Complete (Dockerfile, compose, docs) |
| Cloud Distribution | 📋 Planned |

## Custom Commands

| Command | Description |
|---------|-------------|
| `/build` | Run full build process |
| `/dev` | Start development environment |
| `/check-types` | Run TypeScript type checking |
| `/add-ipc-handler` | Add new IPC handler |
| `/add-model` | Create new domain model |
| `/review-security` | Security audit |
| `/troubleshoot` | Diagnose issues |
| `/analyze` | Deep code analysis |

## Rules

Coding standards in `.claude/rules/`:

- **coding-standards.md** - TypeScript and general conventions
- **electron-patterns.md** - Electron patterns (legacy)
- **git-workflow.md** - Git branch and commit conventions

## New Stack Overview

```
┌─────────────────────────────────────────────────────────────────────┐
│                     CLIENT LAYER                                     │
│  ┌─────────────────────┐    ┌─────────────────────┐                 │
│  │  Tauri Desktop      │    │  Web Browser        │                 │
│  │  (Embedded Webview) │    │  (Remote Access)    │                 │
│  └──────────┬──────────┘    └──────────┬──────────┘                 │
│             │       HTTP/WS API        │                            │
│             └───────────┬──────────────┘                            │
├─────────────────────────┼───────────────────────────────────────────┤
│                         ▼                                           │
│                  HOST SERVER (Rust + Axum)                          │
│       POST /api/invoke/* │ WS /ws │ Static UI (optional)           │
├─────────────────────────────────────────────────────────────────────┤
│                     SERVICE LAYER                                    │
│  ProfileManager │ FFmpegHandler │ SettingsManager │ ThemeManager    │
├─────────────────────────────────────────────────────────────────────┤
│                     FFMPEG LAYER                                     │
│           RTMP Relay │ Encoding Processes │ Stream Stats            │
├─────────────────────────────────────────────────────────────────────┤
│                     STORAGE LAYER                                    │
│              Profiles │ Settings │ Logs │ Themes                    │
└─────────────────────────────────────────────────────────────────────┘
```

## Design System Colors

| Role | Light | Dark |
|------|-------|------|
| Primary | `#7C3AED` (Violet) | `#A78BFA` |
| Secondary | `#C026D3` (Fuchsia) | `#E879F9` |
| Accent | `#DB2777` (Pink) | `#F472B6` |
| Background | `#FAFAFA` | `#0F0A14` |
| Text | `#1F1A29` | `#F4F2F7` |

## Recent Changes

### 2026-01-17
1. ✅ **Docker Distribution** - Complete Dockerfile, docker-compose, and documentation in `docker/`
2. ✅ **Sidecar Configuration** - Fixed Tauri sidecar config with `build-server.ts` script
3. ✅ **Desktop Dev Flow** - Verified end-to-end `npm run dev` works correctly

### 2026-01-16
1. ✅ **HTTP Server Implementation** - Complete Axum-based server with all 30+ commands mapped
2. ✅ **Remote Access Settings** - New UI for configuring host, port, and token
3. ✅ **Backend Abstraction Layer** - Frontend works in Tauri or HTTP mode transparently
4. ✅ **WebSocket Event Broadcasting** - Real-time events to all connected clients
5. ✅ **Launcher Implementation** - Tauri spawns host server as sidecar with health checks
6. ✅ **Token Authentication** - Bearer header + WebSocket query param support
7. 📋 **Master Implementation Plan** - Comprehensive documentation for multi-developer coordination

### 2026-01-06
1. ✅ **Theme Production Build Support** - Fixed theme loading for production builds using Tauri resource API
2. ✅ **Cross-Platform Theme Sync** - Theme sync now works in dev and production on all platforms

### 2026-01-05
1. ✅ **FFmpeg Handler Fixes** - Fixed relay race condition with atomic refcounting
2. ✅ **Robustness Improvements** - Added poisoned mutex recovery
3. ✅ **Theme System Review** - Comprehensive analysis (Grade A, 95/100)
4. ✅ **CSS Value Validation** - Added comprehensive validation for colors, sizes, shadows, gradients
5. ✅ **Example Themes** - Created Dracula, Nord, and Catppuccin Mocha themes
6. ✅ **Theme Documentation** - Created comprehensive README with installation guide
7. ✅ **Development Roadmap** - Created comprehensive v0.1 → v3.0 roadmap

### 2026-01-04
1. ✅ **Migration Complete** - Electron code fully removed
2. ✅ **Passthrough Architecture** - Default groups use copy mode
3. ✅ **Immutable Default Group** - Cannot be edited/deleted
4. ✅ **Profile Simplification** - Removed encoding settings from profile modal
5. ✅ **FFmpeg Enhancements** - Hardware encoder detection, auto-download
6. ✅ **Repository Update** - Moved to ScopeCreep-zip/SpiritStream
7. ✅ **License Update** - Changed to GPL-3.0
8. ✅ **Windows Setup** - Added PowerShell setup script

## Key Files Reference

### Target Structure

```
magillastream/
├── src-frontend/           # React frontend
│   ├── components/ui/      # Base components
│   ├── stores/             # Zustand stores
│   └── styles/tokens.css   # Design tokens
├── src-tauri/              # Rust backend
│   ├── src/commands/       # Tauri commands
│   └── src/services/       # Business logic
└── .claude/                # Claude Code config
```

### Actual Current Structure

```
spiritstream/
├── src-frontend/           # React frontend
│   ├── components/         # UI components
│   ├── hooks/              # Custom React hooks
│   ├── stores/             # Zustand state management
│   ├── lib/                # Utilities
│   │   └── backend/        # ⭐ NEW: Backend abstraction layer
│   │       ├── env.ts      # Mode detection (Tauri/HTTP)
│   │       ├── api.ts      # Tauri native commands
│   │       ├── httpApi.ts  # HTTP API wrapper
│   │       └── httpEvents.ts # WebSocket handler
│   ├── types/              # TypeScript definitions
│   ├── styles/             # Tailwind CSS
│   ├── locales/            # i18n translations (5 languages)
│   └── views/              # Page views
├── src-tauri/              # Rust backend
│   ├── src/
│   │   ├── bin/
│   │   │   └── server.rs   # ⭐ NEW: HTTP server (570 lines)
│   │   ├── launcher.rs     # ⭐ NEW: Host process launcher
│   │   ├── commands/       # Tauri IPC commands
│   │   ├── services/       # Business logic
│   │   └── models/         # Domain models
│   └── Cargo.toml
├── docs/                   # Documentation
│   └── 07-deployment/
│       └── 03-distribution-strategy.md  # ⭐ Distribution plan
├── .claude/                # Claude Code config
│   └── claudedocs/
│       └── web-app-split-master-plan.md # ⭐ Implementation plan
├── .env.example            # Environment variables reference
├── setup.sh                # Unix setup script
└── setup.ps1               # Windows setup script
```

## Common Tasks

### Development Modes

```bash
# Desktop development (Tauri + embedded server)
npm run dev

# Standalone backend server only (no Tauri UI)
npm run backend:dev

# Frontend with separate backend (HTTP mode)
VITE_BACKEND_MODE=http npm run vite:dev

# Production build
npm run build

# Type checking
npm run typecheck    # TypeScript
npm run check        # Rust
```

### Environment Variables

```bash
# Frontend (Vite)
VITE_BACKEND_MODE=http              # Force HTTP mode
VITE_BACKEND_URL=http://host:8008   # Backend URL
VITE_BACKEND_TOKEN=secret           # Auth token

# Backend Server
SPIRITSTREAM_HOST=127.0.0.1         # Bind address (default)
SPIRITSTREAM_PORT=8008              # Port (default)
SPIRITSTREAM_API_TOKEN=secret       # Auth token (optional)
SPIRITSTREAM_UI_ENABLED=1           # Serve static UI
```

### Adding Features

1. **Frontend Component**: Read [component-library.md](./component-library.md)
2. **Tauri Command**: Define in `src-tauri/src/commands/`, register in `main.rs`
3. **State Management**: Add to Zustand stores in `src-frontend/stores/`
4. **i18n**: Add translations to `src-frontend/locales/*.json`

### Understanding Architecture

1. **Host/Client Split**: Read [web-app-split-master-plan.md](./web-app-split-master-plan.md)
2. **Passthrough Mode**: Read [passthrough-architecture.md](./passthrough-architecture.md)
3. **Output Groups**: See [scratch/immutable-default-group.md](./scratch/immutable-default-group.md)
4. **Design System**: Reference [design-system.md](./design-system.md)

## Workstreams (Multi-Developer)

| Workstream | Focus | Key Files |
|------------|-------|-----------|
| **A: Desktop** | Tauri launcher, packaging | `launcher.rs`, `tauri.conf.json` |
| **B: Server + API** | HTTP server, stability | `bin/server.rs`, commands |
| **C: Docker** | Containerization | Dockerfile, compose |
| **D: Frontend** | Remote access UX | `lib/backend/`, views |
| **E: Auth** | Full auth system | External developer |

See [web-app-split-master-plan.md](./web-app-split-master-plan.md) for detailed task breakdown.
