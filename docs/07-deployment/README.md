# Deployment Documentation

[Documentation](../README.md) > Deployment

---

## Overview

This section covers building, packaging, and distributing SpiritStream across all supported platforms.

## Documents

| Document | Description | Audience |
|----------|-------------|----------|
| [01. Building](./01-building.md) | Build process documentation | Intermediate+ |
| [02. Platform Guides](./02-platform-guides.md) | Windows, macOS, Linux specifics | All levels |
| [03. Release Process](./03-release-process.md) | Versioning and distribution | Maintainers |

## Build Commands

| Command | Purpose |
|---------|---------|
| `pnpm dev` | Full desktop development with hot reload |
| `pnpm dev:web` | Frontend only development |
| `pnpm backend:dev` | Backend server only |
| `pnpm build` | Production build (all workspaces) |
| `pnpm build:desktop` | Desktop app with server sidecar |
| `pnpm typecheck` | TypeScript type checking |

## Platform Bundles

| Platform | Format | Size (approx) |
|----------|--------|---------------|
| Windows | `.msi` | 8-12 MB |
| macOS | `.dmg` | 10-15 MB |
| Linux | `.AppImage`, `.deb` | 8-12 MB |

## Requirements

| Component | Minimum |
|-----------|---------|
| Rust | 1.77.2 |
| Node.js | 18.0 |
| pnpm | 8.0 |
| Tauri CLI | 2.0 |

---

*Section: 07-deployment*

