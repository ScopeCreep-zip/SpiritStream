# SpiritStream - Claude Code Context

> This file provides persistent context for Claude Code sessions. It is automatically loaded at session start.

## Project Overview

**SpiritStream** is a desktop streaming application undergoing a complete architectural overhaul. The application manages RTMP stream configurations, handles FFmpeg-based stream processing, and provides a modern UI for multi-output streaming with profile management.

**Repository**: https://github.com/ScopeCreep-zip/SpiritStream
**Current Branch**: web-app-split
**Migration Status**: ✅ **COMPLETE** — Electron fully removed, Tauri 2.x production-ready
**Current Work**: 🔄 **Host Process + Web Client Architecture** — See [web-app-split-master-plan.md](.claude/claudedocs/web-app-split-master-plan.md)

## New Architecture (Target)

### Technology Stack

| Layer | Technology |
|-------|------------|
| Desktop Framework | **Tauri 2.x** |
| Backend Language | **Rust** |
| Frontend Framework | **React 18+** |
| Styling | **Tailwind CSS v4** |
| Build Tool | **Vite + Tauri** |
| State Management | **Zustand** |
| Internationalization | **i18next** (5 languages) |
| Type Safety | **TypeScript + Rust** |

### Design System

The application uses a **Purple & Pink theme** with full light/dark mode support:

- **Primary**: Violet (#7C3AED light / #A78BFA dark)
- **Secondary**: Fuchsia (#C026D3 light / #E879F9 dark)
- **Accent**: Pink (#DB2777 light / #F472B6 dark)
- **Neutrals**: Purple-tinted gray scale

All colors are WCAG 2.2 AA compliant. See `.claude/claudedocs/research/spiritstream-complete-design-system.md` for complete design tokens.

## Target Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                         CLIENT LAYER                                 │
│  ┌─────────────────────────┐    ┌─────────────────────────┐         │
│  │  Tauri Desktop          │    │  Web Browser            │         │
│  │  (Embedded Webview)     │    │  (Remote Access)        │         │
│  └───────────┬─────────────┘    └───────────┬─────────────┘         │
│              │         HTTP/WS API          │                       │
│              └──────────────┬───────────────┘                       │
├─────────────────────────────┼───────────────────────────────────────┤
│                             ▼                                       │
│                    HOST SERVER (Rust + Axum)                        │
│         POST /api/invoke/* │ WS /ws │ Static UI (optional)         │
├─────────────────────────────────────────────────────────────────────┤
│                       SERVICE LAYER                                  │
│    ProfileManager │ FFmpegHandler │ SettingsManager │ ThemeManager  │
├─────────────────────────────────────────────────────────────────────┤
│                       FFMPEG LAYER                                   │
│             RTMP Relay │ Encoding Processes │ Stream Stats          │
├─────────────────────────────────────────────────────────────────────┤
│                       STORAGE LAYER                                  │
│                Profiles │ Settings │ Logs │ Themes                  │
└─────────────────────────────────────────────────────────────────────┘
```

**Deployment Modes:**
- **Desktop**: Tauri launcher spawns host server, UI in embedded webview
- **Docker**: Host server in container, UI served or separate
- **Cloud**: Managed host servers with multi-tenant storage (future)

## Directory Structure (Target)

```
spiritstream/
├── src-frontend/              # React frontend
│   ├── components/            # React components
│   │   ├── ui/               # Base UI components
│   │   ├── layout/           # Layout components
│   │   ├── profile/          # Profile management
│   │   ├── stream/           # Streaming controls
│   │   └── settings/         # Settings panels
│   ├── hooks/                # Custom React hooks
│   ├── stores/               # State management
│   ├── lib/                  # Utilities
│   ├── types/                # TypeScript types
│   ├── styles/               # Global styles + Tailwind
│   │   └── tokens.css        # Design system tokens
│   ├── App.tsx
│   └── main.tsx
├── src-tauri/                 # Rust backend
│   ├── src/
│   │   ├── main.rs           # Tauri entry point
│   │   ├── commands/         # Tauri commands
│   │   │   ├── mod.rs
│   │   │   ├── profile.rs
│   │   │   ├── stream.rs
│   │   │   └── system.rs
│   │   ├── services/         # Business logic
│   │   │   ├── mod.rs
│   │   │   ├── profile_manager.rs
│   │   │   ├── ffmpeg_handler.rs
│   │   │   └── encryption.rs
│   │   ├── models/           # Data structures
│   │   └── utils/            # Utilities
│   ├── Cargo.toml
│   └── tauri.conf.json
├── .claude/                   # Claude Code config
│   ├── claudedocs/           # Documentation
│   ├── commands/             # Custom commands
│   └── rules/                # Coding standards
├── tailwind.config.js
├── vite.config.ts
├── package.json
└── tsconfig.json
```

## Core Domain Models

### Profile
Top-level configuration entity:
- `id: string` - UUID
- `name: string` - User-friendly name
- `incomingUrl: string` - RTMP source URL
- `outputGroups: OutputGroup[]` - Encoding configurations
- `theme?: Theme` - Optional UI customization

### OutputGroup
Encoding profile for stream targets:
- `videoEncoder: string` - FFmpeg video codec
- `resolution: string` - Output resolution
- `videoBitrate: number` - Video bitrate (kbps)
- `fps: number` - Frame rate
- `audioCodec: string` - FFmpeg audio codec
- `audioBitrate: number` - Audio bitrate (kbps)
- `generatePts: boolean` - PTS timestamp generation
- `streamTargets: StreamTarget[]` - Output destinations

### StreamTarget
RTMP destination:
- `url: string` - RTMP server URL
- `streamKey: string` - Authentication key
- `port: number` - RTMP port (default: 1935)

## Tauri Commands (Target API)

### Profile Commands
```rust
#[tauri::command]
async fn get_all_profiles() -> Result<Vec<String>, String>;

#[tauri::command]
async fn load_profile(name: String, password: Option<String>) -> Result<Profile, String>;

#[tauri::command]
async fn save_profile(profile: Profile, password: Option<String>) -> Result<(), String>;

#[tauri::command]
async fn delete_profile(name: String) -> Result<(), String>;
```

### Stream Commands
```rust
#[tauri::command]
async fn start_stream(group: OutputGroup, incoming_url: String) -> Result<ProcessInfo, String>;

#[tauri::command]
async fn stop_stream(group_id: String) -> Result<(), String>;

#[tauri::command]
async fn stop_all_streams() -> Result<(), String>;

#[tauri::command]
async fn get_available_encoders() -> Result<Encoders, String>;
```

## Frontend Component Strategy

### UI Component Library
Build from scratch using:
- Tailwind CSS v4 with design tokens
- CSS custom properties for theming
- Radix UI primitives for accessibility
- Framer Motion for animations

### Core Components
| Component | Purpose |
|-----------|---------|
| `Button` | All button variants (primary, secondary, ghost, destructive) |
| `Card` | Container component with header/body/footer |
| `Input` | Text input with labels and validation |
| `Select` | Dropdown selection |
| `Switch` | Toggle switches |
| `Modal` | Dialog overlays |
| `StreamStatus` | Live/connecting/offline/error indicator |
| `ThemeToggle` | Light/dark mode switch |

## Design Tokens

Theme tokens are defined as CSS custom properties:

```css
/* Primary Colors */
--primary: #7C3AED;           /* Light mode */
--primary: #A78BFA;           /* Dark mode */

/* Backgrounds */
--bg-base: #FAFAFA;           /* Light */
--bg-base: #0F0A14;           /* Dark */

/* Text */
--text-primary: #1F1A29;      /* Light */
--text-primary: #F4F2F7;      /* Dark */

/* Status */
--status-live: #10B981;
--status-connecting: #F59E0B;
--status-offline: #9489A8;
--status-error: #EF4444;
```

See full token list in design system research document.

## Build Commands

```bash
# Development Modes
npm run dev              # Desktop (Tauri + embedded host server)
npm run backend:dev      # Standalone HTTP server only (no Tauri)
npm run vite:dev         # Frontend only (use with VITE_BACKEND_MODE=http)

# Build
npm run build            # Production build
npm run tauri build      # Package for distribution

# Type checking
npm run typecheck        # Check TypeScript
cargo check              # Check Rust

# Linting
npm run lint             # ESLint + Prettier
cargo clippy             # Rust linting
```

## Environment Variables

```bash
# Frontend (Vite)
VITE_BACKEND_MODE=http              # Force HTTP mode (auto-detects if not set)
VITE_BACKEND_URL=http://host:8008   # Backend URL for HTTP mode
VITE_BACKEND_TOKEN=secret           # Auth token

# Backend Server
SPIRITSTREAM_HOST=127.0.0.1         # Bind address (default localhost)
SPIRITSTREAM_PORT=8008              # HTTP port
SPIRITSTREAM_API_TOKEN=secret       # Auth token (optional)
SPIRITSTREAM_UI_ENABLED=1           # Serve static UI files
```

## Security Model

### Remote Access Security
- Default binding: `localhost:8008` (remote access opt-in)
- Token authentication: Bearer header + WebSocket query param
- Enforced only when token is configured
- UI serving disabled by default

### Tauri Security
- Capability-based permissions
- CSP headers enforced
- IPC allowlist configuration
- No Node.js in renderer

### Profile Encryption
- AES-256-GCM encryption
- Argon2id key derivation (Rust)
- Random salt and nonce per encryption
- Stream keys always encrypted at rest

## Coding Standards

### TypeScript (Frontend)
- Strict mode enabled
- Explicit return types for functions
- Interface over type for object shapes
- Functional components with hooks

### Rust (Backend)
- Use `Result<T, E>` for error handling
- Prefer `&str` over `String` for parameters
- Use `#[derive]` macros appropriately
- Document public APIs with `///`

### React Components
- One component per file
- Props interface defined above component
- Use `forwardRef` when exposing refs
- Memoize expensive computations

### CSS/Tailwind
- Use design tokens via `var(--token)`
- Semantic class names for custom CSS
- Mobile-first responsive design
- Dark mode via `data-theme="dark"`

## Documentation Guidelines

**All Claude-generated documentation MUST be placed in `.claude/claudedocs/`**

### Directory Structure

```
.claude/claudedocs/
├── index.md                    # Master index (update when adding docs)
├── architecture-new.md         # System architecture
├── component-library.md        # React components
├── design-system.md            # Design tokens reference
├── tauri-migration.md          # Migration plan
├── ui-specification.md         # UI/UX specification
├── pages-and-views.md          # View documentation
├── research/                   # Research & reference materials
│   ├── *.md                    # Analysis documents
│   └── *.html                  # Mockups, prototypes
└── scratch/                    # Temporary working documents
    └── *.md                    # Draft docs, notes, explorations
```

### Documentation Rules

1. **Never create docs in project root** - Use `.claude/claudedocs/` exclusively
2. **Update index.md** - Add new documents to the index with descriptions
3. **Use `scratch/` for temporary work** - Draft analysis, exploration notes, temporary plans
4. **Use `research/` for reference materials** - Mockups, external research, design specs
5. **Promote scratch to root when finalized** - Move completed docs from `scratch/` to `claudedocs/`

### When to Create Documentation

| Scenario | Location | Filename Pattern |
|----------|----------|------------------|
| Planning a feature | `scratch/` | `feature-name-plan.md` |
| Analyzing code | `scratch/` | `analysis-topic.md` |
| API documentation | `claudedocs/` | `api-name.md` |
| Component specs | `claudedocs/` | `component-name.md` |
| Research/mockups | `research/` | Descriptive name |
| Architecture decisions | `claudedocs/` | `adr-NNN-title.md` |

### Document Template

```markdown
# Document Title

> Brief description of document purpose

## Overview
[What this document covers]

## Content
[Main content]

## Related Documents
- [Link to related doc](./related.md)

---
*Last Updated: YYYY-MM-DD*
```

## Extended Documentation

@.claude/claudedocs/index.md
@.claude/claudedocs/web-app-split-master-plan.md
@.claude/claudedocs/migration-status.md
@.claude/claudedocs/passthrough-architecture.md
@.claude/claudedocs/architecture-new.md
@.claude/claudedocs/component-library.md
@.claude/claudedocs/design-system.md
@.claude/claudedocs/ui-specification.md
@.claude/claudedocs/pages-and-views.md
@.claude/claudedocs/research/spiritstream-complete-design-system.md
