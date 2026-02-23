---
description: Start the development environment
allowed-tools:
  - Bash
---

Start SpiritStream in development mode:

1. `pnpm dev` — runs all workspaces in parallel via Turbo (frontend + server)
2. `pnpm dev:web` — frontend only (Vite on localhost:5173)
3. `pnpm dev:desktop` — desktop app (Tauri launcher + server sidecar)

For backend-only development:
- `cargo run --manifest-path server/Cargo.toml` — starts the Axum HTTP server on localhost:8008

The frontend auto-detects backend mode (HTTP by default). If there are compilation errors, review them and suggest fixes.
