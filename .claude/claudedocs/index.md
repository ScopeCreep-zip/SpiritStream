# MagillaStream Documentation Index

Welcome to the MagillaStream documentation. This index provides quick access to all available documentation.

## Quick Start

- [CLAUDE.md](../../CLAUDE.md) - Main project context and overview

## Current Status

**Branch**: `tauri-shift`
**Migration**: Full lift-and-shift in progress (Electron → Tauri)
**Compatibility**: NO backwards compatibility

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
| [architecture-new.md](./architecture-new.md) | Target architecture with Tauri + React |
| [tauri-migration.md](./tauri-migration.md) | Complete migration plan and checklist |

### Design System

| Document | Description |
|----------|-------------|
| [design-system.md](./design-system.md) | Design token reference guide |
| [component-library.md](./component-library.md) | React component documentation |
| [ui-specification.md](./ui-specification.md) | Complete UI/UX specification from mockup |
| [pages-and-views.md](./pages-and-views.md) | All 8 views with state and Tauri commands |
| [research/magillastream-complete-design-system.md](./research/magillastream-complete-design-system.md) | Full design system specification |
| [research/magillastream-mockup.html](./research/magillastream-mockup.html) | Interactive HTML mockup |

### Legacy (Electron)

| Document | Description |
|----------|-------------|
| [architecture.md](./architecture.md) | Current Electron architecture |
| [electron-ipc.md](./electron-ipc.md) | Electron IPC patterns |
| [models.md](./models.md) | Domain model documentation |
| [services.md](./services.md) | Service layer documentation |
| [frontend.md](./frontend.md) | Vanilla JS frontend |
| [security.md](./security.md) | Security implementation |
| [build-system.md](./build-system.md) | Electron build pipeline |
| [ffmpeg-integration.md](./ffmpeg-integration.md) | FFmpeg process management |
| [development-workflow.md](./development-workflow.md) | Development setup |

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
│                     Frontend (React + Tailwind)                      │
│                        src-frontend/                                 │
│  Components │ Hooks │ Stores │ Styles (Design Tokens)               │
├─────────────────────────────────────────────────────────────────────┤
│                     Tauri IPC Bridge                                 │
│                   @tauri-apps/api                                    │
├─────────────────────────────────────────────────────────────────────┤
│                     Tauri Commands (Rust)                            │
│                      src-tauri/src/                                  │
│  commands/ │ services/ │ models/ │ utils/                           │
├─────────────────────────────────────────────────────────────────────┤
│                     System Integration                               │
│              FFmpeg │ File System │ Encryption                       │
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

## Migration Phases

1. **Setup** - Initialize Tauri, Vite, React
2. **Frontend** - Build component library with Tailwind
3. **Backend** - Port services to Rust
4. **Integration** - Wire frontend to Tauri commands
5. **Build** - Configure packaging
6. **Cleanup** - Remove Electron code

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

### Current Structure (Legacy)

```
magillastream/
├── src/
│   ├── electron/           # Main process
│   ├── models/             # Domain models
│   ├── utils/              # Services
│   └── frontend/           # Vanilla JS UI
└── .claude/                # Claude Code config
```

## Common Tasks

### Start Migration

1. Read [tauri-migration.md](./tauri-migration.md)
2. Install Rust and Tauri CLI
3. Initialize Tauri project
4. Follow phase-by-phase checklist

### Build Components

1. Read [component-library.md](./component-library.md)
2. Reference [design-system.md](./design-system.md) for tokens
3. Use Radix UI primitives for accessibility
4. Apply Tailwind classes with design tokens

### Implement Tauri Commands

1. Define Rust models matching TypeScript types
2. Implement service logic
3. Create command function with `#[tauri::command]`
4. Register in `main.rs`
5. Call from frontend via `invoke()`
