# SpiritStream - Claude Code Context

> This file provides persistent context for Claude Code sessions. It is automatically loaded at session start.

## Project Overview

**SpiritStream** is a professional streaming studio application aiming for **full OBS Studio feature parity**. The application provides scene composition, multi-source input, real-time audio mixing, and multi-platform streaming output through a modern React UI backed by a Rust server.

**Repository**: https://github.com/ScopeCreep-zip/SpiritStream
**Goal**: OBS Studio feature parity in a modern, cross-platform streaming application
**Architecture**: Client-server (Tauri desktop + standalone web browser support)

## Current Work

**Branch**: `multi-input`
**Focus**: Audio and video source capture with OBS parity

We are ensuring all audio and video sources function correctly and match OBS behavior:

- Source capture (cameras, screens, windows, game capture, NDI, capture cards)
- Audio input and monitoring with real-time stereo metering
- Source previews and rendering in the scene canvas
- Device discovery and enumeration across platforms

## Research Guidelines

**Prioritize DeepWiki** for code research and understanding external libraries/frameworks:

```text
Use: mcp__deepwiki__ask_question for questions about repos like obsproject/obs-studio
Use: mcp__deepwiki__read_wiki_contents for documentation lookup
```

**Fall back to WebSearch** when DeepWiki doesn't have the answer or for general web resources.

When researching OBS behavior, query the OBS repository directly:

- `obsproject/obs-studio` - Main OBS source code
- `obsproject/obs-websocket` - WebSocket protocol reference

## Feature Status

### Implemented

| Feature | Status | OBS Parity |
|---------|--------|------------|
| Scene composition | ✅ | Layers, groups, transforms |
| Studio Mode | ✅ | Preview/Program, TAKE, T-Bar |
| Audio Mixer | ✅ | Stereo metering, 20s peak hold, dB scale |
| Transitions | ✅ | 12 types (cut, fade, slide, wipe, stinger, luma) |
| Video Filters | ✅ | Chroma key, color correction, LUT, blur, etc. |
| Audio Filters | ✅ | Compressor, gate, expander, gain, suppression |
| Multiview | ✅ | 2x2, 3x3, 4x4 grids |
| Projectors | ✅ | Scene, source, preview, program, multiview |
| Recording | ✅ | Multi-format output |
| Replay Buffer | ✅ | Configurable duration |

### In Progress (Current Sprint)

| Feature | Status | Notes |
|---------|--------|-------|
| Camera capture | 🔄 | Device enumeration, resolution selection |
| Screen capture | 🔄 | Display selection, cursor capture |
| Window capture | 🔄 | Application window targeting |
| Audio device input | 🔄 | Microphone, line-in capture |
| Game capture | 🔄 | Hardware-accelerated game capture |
| NDI source | 🔄 | Network video input |
| Capture card | 🔄 | HDMI/SDI input devices |

### Planned

| Feature | Priority |
|---------|----------|
| Virtual camera output | High |
| Advanced audio routing | Medium |
| Plugin system | Future |
| Cloud SaaS distribution | Future |

## Technology Stack

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
| Streaming | **FFmpeg + Go2rtc** |

### Design System

The application uses a **Purple & Pink theme** with full light/dark mode support:

- **Primary**: Violet (#7C3AED light / #A78BFA dark)
- **Secondary**: Fuchsia (#C026D3 light / #E879F9 dark)
- **Accent**: Pink (#DB2777 light / #F472B6 dark)
- **Neutrals**: Purple-tinted gray scale

All colors are WCAG 2.2 AA compliant.

## Architecture

```text
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
│   Compositor │ AudioLevels │ DeviceDiscovery │ PreviewHandler       │
│   ProfileManager │ FFmpegHandler │ ScreenCapture │ CameraCapture    │
├─────────────────────────────────────────────────────────────────────┤
│                       MEDIA LAYER                                    │
│        FFmpeg (encoding) │ Go2rtc (RTMP relay) │ WebRTC (preview)   │
├─────────────────────────────────────────────────────────────────────┤
│                       STORAGE LAYER                                  │
│                Profiles │ Settings │ Logs │ Themes                  │
└─────────────────────────────────────────────────────────────────────┘
```

**Deployment Modes:**

- **Desktop**: Tauri launcher spawns host server, UI in embedded webview
- **Docker**: Host server in container, UI served or separate
- **Cloud**: Managed host servers with multi-tenant storage (future)

## Directory Structure

```text
spiritstream/
├── apps/
│   ├── web/                      # React frontend (standalone)
│   │   ├── package.json          # @spiritstream/web
│   │   ├── vite.config.ts
│   │   ├── index.html
│   │   └── src/
│   │       ├── components/       # React components
│   │       │   ├── ui/          # Base UI components
│   │       │   ├── layout/      # Layout components
│   │       │   ├── stream/      # Streaming controls
│   │       │   └── modals/      # Modal dialogs
│   │       ├── hooks/           # Custom React hooks
│   │       ├── stores/          # Zustand state management
│   │       ├── lib/
│   │       │   └── backend/     # Backend abstraction (Tauri/HTTP)
│   │       ├── types/           # TypeScript types
│   │       ├── styles/          # Global styles + Tailwind
│   │       ├── locales/         # i18n translations
│   │       └── views/           # Page views
│   │
│   └── desktop/                  # Tauri wrapper (minimal)
│       ├── package.json          # @spiritstream/desktop
│       ├── vite.config.ts        # Points to ../web
│       └── src-tauri/
│           ├── Cargo.toml        # Minimal deps (launcher only)
│           ├── tauri.conf.json   # Sidecar config
│           ├── binaries/         # Server sidecar binary
│           └── src/main.rs       # Launcher (spawns server)
│
├── server/                       # Standalone Rust backend
│   ├── Cargo.toml                # No Tauri dependencies
│   └── src/
│       ├── main.rs               # Axum HTTP server
│       ├── lib.rs
│       ├── commands/             # Business logic
│       ├── models/               # Domain models
│       └── services/             # Service layer
│
├── packages/
│   └── shared/                   # Shared TypeScript types (future)
│
├── docker/
│   ├── Dockerfile                # Backend container
│   └── docker-compose.yml
│
├── .claude/                      # Claude Code config
│   ├── claudedocs/              # Documentation
│   ├── commands/                # Custom commands
│   └── rules/                   # Coding standards
│
├── pnpm-workspace.yaml           # Workspace config
├── turbo.json                    # Build orchestration
└── package.json                  # Root workspace
```

## Source Types

SpiritStream supports 13 source types matching OBS:

| Type | Description | Key Properties |
|------|-------------|----------------|
| `rtmp` | Network stream input | `url` |
| `camera` | Webcam/USB camera | `deviceId`, `resolution`, `fps` |
| `screen` | Display capture | `displayId`, `captureCursor` |
| `window` | Application window | `windowId`, `captureCursor` |
| `game` | Game capture | `windowId`, `captureMode`, `allowTransparency` |
| `captureCard` | HDMI/SDI input | `deviceId`, `resolution`, `fps` |
| `ndi` | NDI network source | `sourceName`, `bandwidth`, `lowLatency` |
| `mediaFile` | Video/audio file | `filePath`, `loop`, `restartOnActivate` |
| `mediaPlaylist` | File playlist | `items[]`, `shuffleMode`, `loop` |
| `text` | Text overlay | `text`, `font`, `color`, `outline` |
| `browser` | Web page | `url`, `width`, `height`, `css` |
| `colorFill` | Solid color | `color` |
| `nestedScene` | Scene within scene | `sceneId` |
| `audioDevice` | Audio-only input | `deviceId`, `channels` |

## Core Domain Models

### Profile

Top-level configuration containing all scenes, sources, and settings:

- `id: string` - UUID
- `name: string` - User-friendly name
- `scenes: Scene[]` - Scene compositions
- `globalAudioSources: Source[]` - Audio sources available in mixer
- `outputGroups: OutputGroup[]` - Encoding and streaming targets

### Scene

A composable canvas with positioned sources:

- `id: string` - UUID
- `name: string` - Display name
- `layers: Layer[]` - Positioned source instances
- `layerGroups: LayerGroup[]` - Organizational grouping
- `defaultTransition: TransitionConfig` - Scene-specific transition

### Layer

A source instance positioned on a scene canvas:

- `id: string` - UUID
- `sourceId: string` - Reference to source
- `position: { x, y }` - Canvas position
- `size: { width, height }` - Display size
- `rotation: number` - Degrees
- `crop: { top, right, bottom, left }` - Pixel crop
- `visible: boolean` - Layer visibility
- `locked: boolean` - Prevent editing
- `filters: VideoFilter[]` - Applied video filters

### Source

A reusable input that can be placed in multiple scenes:

- `id: string` - UUID
- `name: string` - Display name
- `type: SourceType` - One of 13 source types
- `config: SourceConfig` - Type-specific configuration
- `audioConfig?: AudioConfig` - Volume, mute, filters
- `videoFilters?: VideoFilter[]` - Default filters

### OutputGroup

Encoding profile for stream targets:

- `videoEncoder: string` - FFmpeg video codec
- `resolution: string` - Output resolution
- `videoBitrate: number` - Video bitrate (kbps)
- `fps: number` - Frame rate
- `audioCodec: string` - FFmpeg audio codec
- `audioBitrate: number` - Audio bitrate (kbps)
- `streamTargets: StreamTarget[]` - Output destinations

### StreamTarget

RTMP destination:

- `url: string` - RTMP server URL
- `streamKey: string` - Authentication key
- `port: number` - RTMP port (default: 1935)

## Key Files for Current Work

### Frontend (Source/Audio)

- `apps/web/src/types/source.ts` - Source type definitions
- `apps/web/src/stores/sourceStore.ts` - Source state management
- `apps/web/src/hooks/useAudioLevels.ts` - Audio metering hook
- `apps/web/src/components/stream/AudioMixerPanel.tsx` - Mixer UI
- `apps/web/src/components/stream/UnifiedChannelStrip.tsx` - Per-track controls
- `apps/web/src/components/modals/AddSourceModal.tsx` - Source creation

### Backend (Capture Services)

- `server/src/models/source.rs` - Source models
- `server/src/services/device_discovery/` - Device enumeration (module)
- `server/src/services/audio_capture.rs` - Audio input
- `server/src/services/audio_levels.rs` - Level metering
- `server/src/services/screen_capture.rs` - Display capture
- `server/src/services/camera_capture.rs` - Webcam capture
- `server/src/services/h264_capture.rs` - Hardware capture

## Build Commands

```bash
# Development Modes
pnpm dev                  # All workspaces in parallel (Turbo)
pnpm dev:web              # Frontend only (localhost:5173)
pnpm dev:desktop          # Desktop app (Tauri + server sidecar)
pnpm backend:dev          # Standalone HTTP server (localhost:8008)

# Build
pnpm build                # All workspaces (Turbo)
pnpm build:web            # Frontend only
pnpm build:desktop        # Desktop app with server sidecar
pnpm backend:build        # Rust server release build

# Type checking
pnpm typecheck            # Check TypeScript (Turbo)
cargo check --manifest-path server/Cargo.toml    # Check server
cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml  # Check desktop

# Linting
pnpm lint                 # ESLint (Turbo)
pnpm format               # Prettier
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

### CORS Configuration

The backend server must allow cross-origin requests from the frontend:

**Server-side (Rust/Axum)** in `server/src/main.rs`:

```rust
use tower_http::cors::{Any, CorsLayer};

let cors = CorsLayer::new()
    .allow_origin(Any)  // Or specific origins for production
    .allow_methods(Any)
    .allow_headers(Any);

app.layer(cors)
```

**Common CORS issues:**

- WebSocket connections blocked: Ensure `/ws` endpoint allows upgrade
- Preflight failures: Check OPTIONS requests are handled
- Credentials: If using cookies/auth, set `allow_credentials(true)` and specific origins

### CSP (Content Security Policy)

**Tauri CSP** in `apps/desktop/src-tauri/tauri.conf.json`:

```json
{
  "app": {
    "security": {
      "csp": "default-src 'self'; connect-src 'self' http://127.0.0.1:8008 ws://127.0.0.1:8008 http://localhost:8008 ws://localhost:8008; img-src 'self' data: blob:; media-src 'self' blob:; style-src 'self' 'unsafe-inline'"
    }
  }
}
```

**CSP directives needed:**

| Directive | Required Values | Purpose |
|-----------|-----------------|---------|
| `default-src` | `'self'` | Base policy |
| `connect-src` | `'self' http://127.0.0.1:8008 ws://127.0.0.1:8008` | HTTP API + WebSocket |
| `img-src` | `'self' data: blob:` | Images, thumbnails, previews |
| `media-src` | `'self' blob:` | Video/audio streams |
| `style-src` | `'self' 'unsafe-inline'` | Tailwind + inline styles |

**Common CSP issues:**

- WebSocket blocked: Add `ws://` URLs to `connect-src`
- Blob URLs blocked: Add `blob:` to `img-src` and `media-src`
- Inline styles blocked: Add `'unsafe-inline'` to `style-src` (required for Tailwind)
- Dynamic backend port: May need to update CSP at runtime or use wildcard

## Coding Standards

See `.claude/rules/` for detailed patterns:

- `coding-standards.md` - General TypeScript/Rust conventions
- `rust-patterns.md` - Rust-specific patterns (async, error handling)
- `ffmpeg-patterns.md` - FFmpeg process management
- `tauri-patterns.md` - Tauri sidecar and security patterns

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
- Use `#[serde(rename_all = "camelCase")]` for JSON interop

### React Components

- One component per file
- Props interface defined above component
- Use `forwardRef` when exposing refs
- Memoize expensive computations

### Security

- Never log stream keys or tokens
- Sanitize all paths to prevent traversal attacks
- Validate inputs on backend, trust nothing from frontend

## Documentation Guidelines

**All Claude-generated documentation MUST be placed in `.claude/claudedocs/`**

### Directory Structure

```text
.claude/claudedocs/
├── index.md                    # Master index (update when adding docs)
├── architecture-new.md         # System architecture
├── component-library.md        # React components
├── design-system.md            # Design tokens reference
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

## OBS Reference

When implementing features, reference OBS behavior:

```text
# Ask DeepWiki about OBS implementation
mcp__deepwiki__ask_question("obsproject/obs-studio", "How does OBS handle audio device capture?")

# Read OBS documentation
mcp__deepwiki__read_wiki_contents("obsproject/obs-studio")
```

Key OBS source files for reference:

- `libobs/obs-source.c` - Source base implementation
- `plugins/win-capture/` - Windows capture plugins
- `plugins/mac-capture/` - macOS capture plugins
- `plugins/linux-capture/` - Linux capture plugins
- `libobs/audio-monitoring.c` - Audio monitoring
