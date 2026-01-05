# SpiritStream Documentation Index

Welcome to the SpiritStream documentation. This index provides quick access to all available documentation.

## Quick Start

- [CLAUDE.md](../../CLAUDE.md) - Main project context and overview

## Current Status

**Branch**: `cleanup-release-cand`
**Migration**: ✅ **COMPLETE** (Electron fully removed)
**Production Status**: Tauri 2.x production-ready

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
| [migration-status.md](./migration-status.md) | ✅ **Migration complete status** |
| [passthrough-architecture.md](./passthrough-architecture.md) | Passthrough-first design with immutable default group |
| [architecture-new.md](./architecture-new.md) | Current Tauri + React architecture |
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
| Theme System | ✅ Complete (light/dark, purple/pink) |

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

## Recent Changes (2026-01-04)

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
│   ├── hooks/             # Custom React hooks
│   ├── stores/            # Zustand state management
│   ├── lib/               # Utilities and Tauri API
│   ├── types/             # TypeScript definitions
│   ├── styles/            # Tailwind CSS
│   ├── locales/           # i18n translations (5 languages)
│   └── views/             # Page views
├── src-tauri/             # Rust backend
│   ├── src/
│   │   ├── commands/      # Tauri IPC commands
│   │   ├── services/      # Business logic
│   │   └── models/        # Domain models
│   └── Cargo.toml
├── .claude/               # Claude Code config
├── setup.sh               # Unix setup script
└── setup.ps1              # Windows setup script
```

## Common Tasks

### Development

```bash
npm run dev          # Start development server
npm run build        # Production build
npm run typecheck    # Check TypeScript
npm run check        # Check Rust
npm run lint         # Run ESLint
npm run format       # Format with Prettier
```

### Adding Features

1. **Frontend Component**: Read [component-library.md](./component-library.md)
2. **Tauri Command**: Define in `src-tauri/src/commands/`, register in `main.rs`
3. **State Management**: Add to Zustand stores in `src-frontend/stores/`
4. **i18n**: Add translations to `src-frontend/locales/*.json`

### Understanding Architecture

1. **Passthrough Mode**: Read [passthrough-architecture.md](./passthrough-architecture.md)
2. **Output Groups**: See [scratch/immutable-default-group.md](./scratch/immutable-default-group.md)
3. **Design System**: Reference [design-system.md](./design-system.md)
