# SpiritStream

Desktop streaming application with RTMP stream management, FFmpeg processing, and multi-output streaming with profile management.

**Repository**: https://github.com/ScopeCreep-zip/SpiritStream

## Architecture

Host server (Rust/Axum) + Client (React) — desktop via Tauri sidecar, also Docker and web browser access.

See `.claude/rules/architecture.md` for full details.

## Tech Stack

| Layer | Technology |
|-------|------------|
| Backend | Rust + Axum 0.7 |
| Frontend | React 19 + TypeScript 5.9 |
| Styling | Tailwind CSS v4 |
| Build | Vite 7 + Turbo |
| State | Zustand 5 |
| i18n | i18next (11 locales) |
| Desktop | Tauri 2.x (launcher only) |

## Build Commands

```bash
pnpm dev                  # All workspaces (Turbo)
pnpm dev:web              # Frontend only (localhost:5173)
pnpm dev:desktop          # Desktop app (Tauri + server sidecar)

pnpm build                # All workspaces (Turbo)
pnpm build:web            # Frontend only
pnpm build:desktop        # Desktop app with sidecar

pnpm typecheck            # TypeScript checking (Turbo)
cargo check --manifest-path server/Cargo.toml               # Server
cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml  # Desktop
```

## Environment Variables

```bash
# Frontend (Vite)
VITE_BACKEND_MODE=http              # Force HTTP mode (auto-detects if unset)
VITE_BACKEND_URL=http://host:8008   # Backend URL for HTTP mode
VITE_BACKEND_TOKEN=secret           # Auth token

# Backend Server
SPIRITSTREAM_HOST=127.0.0.1         # Bind address (default localhost)
SPIRITSTREAM_PORT=8008              # HTTP port
SPIRITSTREAM_API_TOKEN=secret       # Auth token (optional)
SPIRITSTREAM_UI_ENABLED=1           # Serve static UI files
```

## Design Theme

Purple & pink palette, WCAG 2.2 AA compliant, full light/dark mode. Primary: Violet, Secondary: Fuchsia, Accent: Pink. See `.claude/claudedocs/research/spiritstream-complete-design-system.md`.

## Coding Standards

- **TypeScript**: strict mode, explicit return types, `interface` for objects, `type` for unions
- **Rust**: `Result<T, String>` errors, `Arc<ServiceManager>` pattern, `mask_sensitive()` for logs
- **React**: functional components, Zustand stores, all API calls via `api.*` from `lib/backend/`
- **CSS**: Tailwind v4 with design tokens, dark mode via `data-theme="dark"`

See `.claude/rules/coding-standards.md` for full details.

## Extended Documentation

@.claude/claudedocs/web-app-split-master-plan.md
