# SpiritStream - Claude Code Context

Professional streaming studio aiming for **full OBS Studio feature parity**. React UI + Rust/Axum backend, Tauri desktop wrapper.

**Repo**: https://github.com/ScopeCreep-zip/SpiritStream

## Current Work

**Branch**: `multi-input-v2`
**Focus**: Audio and video source capture with OBS parity — cameras, screens, windows, audio devices, previews, device discovery.

## Tech Stack

Tauri 2.x | Rust/Axum backend | React 18 | TypeScript | Tailwind CSS v4 | Zustand | Vite | FFmpeg + go2rtc | i18next (5 langs)

## Build Commands

```bash
# Development
pnpm dev                  # All workspaces (Turbo)
pnpm dev:web              # Frontend only (localhost:5173)
pnpm backend:dev          # Rust server only (localhost:8008)
pnpm dev:desktop          # Desktop app (Tauri + server sidecar)

# Build
pnpm build                # All workspaces (Turbo)
pnpm backend:build        # Rust server release build

# Type checking
pnpm typecheck            # TypeScript (Turbo)
cargo check --manifest-path server/Cargo.toml

# Linting
pnpm lint                 # ESLint (Turbo)
pnpm format               # Prettier
```

## Research Guidelines

**Prioritize DeepWiki** for external library/framework questions:
```
mcp__deepwiki__ask_question("obsproject/obs-studio", "How does OBS handle X?")
mcp__deepwiki__read_wiki_contents("obsproject/obs-studio")
```
Fall back to **WebSearch** when DeepWiki doesn't have the answer.

## Key Gotchas

### Preview Architecture (Critical)
- **One capture per source, multiple consumers** — OBS pattern
- Edit mode: `useJpegPreview` (capture → JPEG → canvas). NO FFmpeg/H264/go2rtc
- Preview mode: MSE for scene composite (capture → GPU compositor → H264 → go2rtc → MSE)
- **NEVER spawn per-source H264 encoders for canvas** — exhausts Mac VideoToolbox slots (~3-4 max), freezes machine

### WKWebView/Safari Rendering (Critical)
- **NEVER blob URL → img.src at high fps** — WKWebView IPC freeze
- MJPEG is broken in Safari/WKWebView (15+ years unfixed)
- Use decoupled frame buffer: `WS onmessage → ArrayBuffer ref → rAF → createImageBitmap → canvas.drawImage → bitmap.close()`
- `will-change: transform` on canvas (forces GPU layer), NEVER `transition-opacity` on canvas

### JPEG Encoding Performance
- **NEVER use `image` crate JPEG** in debug builds (~500ms/frame). Use `turbojpeg` (~30ms debug, ~2ms release)
- Build needs: `PKG_CONFIG_PATH="/usr/local/Cellar/jpeg-turbo/3.1.3/lib/pkgconfig:$PKG_CONFIG_PATH"`

### macOS Permissions
- Screen: `scap::has_permission()` — call at capture start, not cached
- Camera: `objc` crate `AVCaptureDevice authorizationStatusForMediaType:` — returns 0-3
- FFmpeg avfoundation has NO permission code — hangs if `.notDetermined`

### MPEG-TS Alignment
- `read_mpegts_output()` must send 188-byte-aligned packets starting with sync byte 0x47
- go2rtc rejects misaligned data with "wrong sync byte"

## Coding Standards

See `.claude/rules/` for detailed patterns:
- `coding-standards.md` — TypeScript/Rust conventions
- `rust-patterns.md` — Async, error handling, module organization
- `ffmpeg-patterns.md` — FFmpeg process management
- `tauri-patterns.md` — Tauri sidecar and security patterns
- `architecture.md` — System architecture and directory structure
- `domain-models.md` — Data models and source types
- `security.md` — CORS, CSP, encryption, auth
- `environment.md` — Environment variables
- `feature-status.md` — Implemented/in-progress/planned features

## JSON Interop

Rust: `#[serde(rename_all = "camelCase")]`. TypeScript interfaces match camelCase fields.

## Documentation Rules

All Claude-generated docs go in `.claude/claudedocs/` — never in project root.
- `claudedocs/index.md` — master index (update when adding)
- `claudedocs/scratch/` — temporary work
- `claudedocs/research/` — reference materials

## OBS Reference

Key OBS source files: `libobs/obs-source.c`, `plugins/mac-capture/`, `plugins/win-capture/`, `libobs/audio-monitoring.c`
