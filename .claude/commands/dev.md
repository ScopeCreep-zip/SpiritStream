---
description: Start the development environment
allowed-tools:
  - Bash
---

Start SpiritStream in development mode. Choose the appropriate mode:

## Frontend only (React + Vite)
```bash
pnpm dev:web
```
Starts the frontend dev server at localhost:5173.

## Backend only (Rust + Axum)
```bash
pnpm backend:dev
```
Starts the Rust server at localhost:8008.

## Both (recommended for full dev)
Run in separate terminals or use:
```bash
pnpm dev
```
This starts all workspaces in parallel via Turbo.

## Desktop app (Tauri)
```bash
pnpm dev:desktop
```
Launches Tauri with the server sidecar and embedded webview.

Note: If there are compilation errors, they will appear in the output. Review them and suggest fixes.
