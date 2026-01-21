# Building SpiritStream

[Documentation](../README.md) > [Deployment](./README.md) > Building

---

This document covers building SpiritStream for development and production across all supported platforms and deployment modes.

---

## Project Structure

SpiritStream uses a **monorepo architecture** with pnpm workspaces and Turbo for build orchestration:

```
spiritstream/
├── apps/
│   ├── web/                    # React frontend (standalone)
│   │   ├── package.json        # @spiritstream/web
│   │   ├── vite.config.ts
│   │   └── src/
│   └── desktop/                # Tauri wrapper (minimal)
│       ├── package.json        # @spiritstream/desktop
│       └── src-tauri/
│           ├── Cargo.toml
│           ├── tauri.conf.json
│           └── binaries/       # Server sidecar
├── server/                     # Standalone Rust backend
│   ├── Cargo.toml
│   └── src/
├── docker/                     # Docker configuration
│   ├── Dockerfile
│   └── docker-compose.yml
├── pnpm-workspace.yaml
├── turbo.json
└── package.json
```

---

## Prerequisites

### Required Tools

| Tool | Version | Purpose |
|------|---------|---------|
| Node.js | 18+ | Frontend tooling |
| pnpm | 8+ | Package management |
| Rust | 1.70+ | Backend compilation |
| Tauri CLI | 2.x | Desktop build orchestration |

### Platform-Specific Requirements

**macOS:**
- Xcode Command Line Tools
- macOS 10.15+ (Catalina)

**Windows:**
- Visual Studio Build Tools 2019+
- WebView2 Runtime
- Windows 10/11

**Linux:**
- `libwebkit2gtk-4.1-dev`
- `libappindicator3-dev`
- `librsvg2-dev`

---

## Development Setup

### Clone and Install

```bash
# Clone repository
git clone https://github.com/ScopeCreep-zip/SpiritStream.git
cd SpiritStream

# Install dependencies (uses pnpm workspaces)
pnpm install

# Install Tauri CLI (if not global)
cargo install tauri-cli
```

### Development Modes

SpiritStream supports multiple development modes:

```bash
# Full desktop app (Tauri + server sidecar + UI)
pnpm dev

# Frontend only (localhost:5173)
pnpm dev:web

# Backend server only (localhost:8008)
pnpm backend:dev

# Frontend with remote backend (HTTP mode)
VITE_BACKEND_MODE=http VITE_BACKEND_URL=http://localhost:8008 pnpm dev:web
```

### Dev Mode Details

| Mode | Command | Use Case |
|------|---------|----------|
| Desktop | `pnpm dev` | Full desktop app development |
| Web Only | `pnpm dev:web` | Frontend development (no backend) |
| Server Only | `pnpm backend:dev` | Backend API development |
| HTTP Client | `VITE_BACKEND_MODE=http pnpm dev:web` | Test browser-based remote access |

### Dev Server Options

```bash
# Specify Vite port
VITE_PORT=3000 pnpm dev:web

# Enable verbose Rust logging
RUST_LOG=debug pnpm dev

# Backend on different port
SPIRITSTREAM_PORT=9000 pnpm backend:dev
```

---

## Production Build

### Build All Workspaces

```bash
# Build all packages (Turbo orchestrated)
pnpm build

# Build specific workspaces
pnpm build:web       # Frontend only
pnpm build:desktop   # Desktop app with server sidecar
```

### Desktop Build Output

```
apps/desktop/src-tauri/target/release/
├── spiritstream              # Linux binary
├── spiritstream.exe          # Windows binary
└── bundle/
    ├── msi/                  # Windows installer
    ├── dmg/                  # macOS disk image
    ├── macos/                # macOS application
    ├── appimage/             # Linux AppImage
    └── deb/                  # Linux Debian package
```

### Standalone Server Build

```bash
# Build the standalone HTTP server
pnpm backend:build

# Or directly with Cargo
cargo build --release --manifest-path server/Cargo.toml
```

Server output: `server/target/release/spiritstream-server`

---

## Platform-Specific Builds

### Windows

```bash
# Build MSI installer
pnpm tauri build --target x86_64-pc-windows-msvc

# Build NSIS installer (alternative)
pnpm tauri build --bundles nsis
```

**Output:**
- `spiritstream_1.0.0_x64-setup.exe` (NSIS)
- `spiritstream_1.0.0_x64_en-US.msi` (MSI)

### macOS

```bash
# Build for Intel Mac
pnpm tauri build --target x86_64-apple-darwin

# Build for Apple Silicon
pnpm tauri build --target aarch64-apple-darwin

# Build Universal binary (both)
pnpm tauri build --target universal-apple-darwin
```

**Output:**
- `SpiritStream_1.0.0_x64.dmg`
- `SpiritStream.app`

### Linux

```bash
# Build AppImage
pnpm tauri build --target x86_64-unknown-linux-gnu

# Build specific bundle type
pnpm tauri build --bundles appimage,deb
```

**Output:**
- `spiritstream_1.0.0_amd64.AppImage`
- `spiritstream_1.0.0_amd64.deb`

---

## Docker Build

SpiritStream can be deployed as a Docker container for self-hosted streaming.

### Build Docker Image

```bash
# Build from project root
docker build -t spiritstream:latest -f docker/Dockerfile .

# Or use docker-compose
cd docker
docker compose build
```

### Run Container

```bash
# Basic run
docker run -p 8008:8008 -p 1935:1935 spiritstream:latest

# With persistent data
docker run -p 8008:8008 -p 1935:1935 \
  -v spiritstream-data:/app/data \
  spiritstream:latest

# With environment configuration
docker run -p 8008:8008 -p 1935:1935 \
  -e SPIRITSTREAM_HOST=0.0.0.0 \
  -e SPIRITSTREAM_API_TOKEN=your-secret-token \
  spiritstream:latest
```

### Docker Compose

```bash
cd docker
docker compose up -d
```

See [Distribution Strategy](./03-distribution-strategy.md) for complete Docker deployment documentation.

---

## Build Configuration

### tauri.conf.json (apps/desktop/src-tauri/)

```json
{
  "productName": "SpiritStream",
  "version": "1.0.0",
  "identifier": "com.spiritstream.app",
  "build": {
    "beforeDevCommand": "pnpm dev:web",
    "beforeBuildCommand": "pnpm build:web",
    "devUrl": "http://localhost:5173",
    "frontendDist": "../../web/dist"
  },
  "app": {
    "windows": [
      {
        "title": "SpiritStream",
        "width": 1200,
        "height": 800,
        "minWidth": 800,
        "minHeight": 600,
        "resizable": true,
        "fullscreen": false
      }
    ],
    "security": {
      "csp": "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; connect-src 'self' http://127.0.0.1:* ws://127.0.0.1:*"
    }
  },
  "bundle": {
    "active": true,
    "targets": "all",
    "externalBin": ["binaries/spiritstream-server"],
    "icon": [
      "icons/32x32.png",
      "icons/128x128.png",
      "icons/128x128@2x.png",
      "icons/icon.icns",
      "icons/icon.ico"
    ],
    "resources": ["resources/*"]
  }
}
```

### Vite Configuration (apps/web/)

```typescript
// apps/web/vite.config.ts
import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import path from 'path';

export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: {
      '@': path.resolve(__dirname, './src'),
    },
  },
  build: {
    outDir: 'dist',
    emptyOutDir: true,
    sourcemap: false,
    minify: 'terser',
    rollupOptions: {
      output: {
        manualChunks: {
          vendor: ['react', 'react-dom', 'zustand'],
        },
      },
    },
  },
  server: {
    port: 5173,
    strictPort: true,
  },
});
```

### Turbo Configuration

```json
// turbo.json
{
  "$schema": "https://turbo.build/schema.json",
  "tasks": {
    "build": {
      "dependsOn": ["^build"],
      "outputs": ["dist/**", ".next/**", "target/**"]
    },
    "dev": {
      "cache": false,
      "persistent": true
    },
    "typecheck": {
      "dependsOn": ["^typecheck"]
    }
  }
}
```

---

## Cargo Configuration

### Release Profile (server/Cargo.toml)

```toml
[profile.release]
opt-level = 3
lto = true
codegen-units = 1
panic = "abort"
strip = true
```

### Feature Flags

```toml
[features]
default = []
devtools = ["tauri/devtools"]
```

Build with features:

```bash
# Enable devtools in release
pnpm tauri build --features devtools
```

---

## Sidecar Configuration

The desktop app spawns the server as a sidecar binary. Build the sidecar:

```bash
# Build server for sidecar (runs automatically via build script)
pnpm build:server

# Manual build for specific platform
cargo build --release --manifest-path server/Cargo.toml
```

The `build-server.ts` script copies the built binary to `apps/desktop/src-tauri/binaries/` with the correct platform triple naming.

---

## Code Signing

### macOS

```bash
# Set signing identity
export APPLE_SIGNING_IDENTITY="Developer ID Application: Your Name (XXXXXXXXXX)"

# Build and sign
pnpm tauri build

# Notarize (requires Apple Developer account)
xcrun notarytool submit apps/desktop/src-tauri/target/release/bundle/dmg/SpiritStream.dmg \
  --apple-id "your@email.com" \
  --password "app-specific-password" \
  --team-id "XXXXXXXXXX" \
  --wait
```

### Windows

```bash
# Using signtool
signtool sign /f certificate.pfx /p password /tr http://timestamp.digicert.com /td sha256 spiritstream.exe
```

---

## Optimization

### Bundle Size Reduction

```toml
# Cargo.toml - Strip symbols
[profile.release]
strip = true
panic = "abort"
```

### Frontend Optimization

```bash
# Analyze bundle size
pnpm build:web -- --sourcemap
npx vite-bundle-visualizer
```

### Typical Bundle Sizes

| Platform | Uncompressed | Compressed |
|----------|--------------|------------|
| Windows (MSI) | ~15 MB | ~8 MB |
| macOS (DMG) | ~12 MB | ~6 MB |
| Linux (AppImage) | ~18 MB | ~10 MB |
| Docker Image | ~50 MB | N/A |

---

## CI/CD Integration

### GitHub Actions Example

```yaml
# .github/workflows/build.yml
name: Build

on:
  push:
    tags:
      - 'v*'

jobs:
  build:
    strategy:
      matrix:
        platform: [macos-latest, ubuntu-latest, windows-latest]

    runs-on: ${{ matrix.platform }}

    steps:
      - uses: actions/checkout@v4

      - name: Setup Node
        uses: actions/setup-node@v4
        with:
          node-version: '20'

      - name: Setup pnpm
        uses: pnpm/action-setup@v2
        with:
          version: 8

      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable

      - name: Install dependencies (Linux)
        if: matrix.platform == 'ubuntu-latest'
        run: |
          sudo apt-get update
          sudo apt-get install -y libwebkit2gtk-4.1-dev libappindicator3-dev

      - name: Install npm dependencies
        run: pnpm install

      - name: Build
        run: pnpm build:desktop

      - name: Upload artifacts
        uses: actions/upload-artifact@v4
        with:
          name: binaries-${{ matrix.platform }}
          path: |
            apps/desktop/src-tauri/target/release/bundle/
```

### Docker CI

```yaml
# .github/workflows/docker.yml
name: Docker

on:
  push:
    tags:
      - 'v*'

jobs:
  docker:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Build and push
        uses: docker/build-push-action@v5
        with:
          context: .
          file: docker/Dockerfile
          push: true
          tags: ghcr.io/scopecreep-zip/spiritstream:latest
```

---

## Troubleshooting

### Common Build Issues

| Issue | Cause | Solution |
|-------|-------|----------|
| `webkit2gtk not found` | Missing Linux dep | Install `libwebkit2gtk-4.1-dev` |
| `MSVC not found` | Missing Windows tools | Install VS Build Tools |
| `codesign failed` | Invalid certificate | Check signing identity |
| `out of memory` | Large bundle | Increase `codegen-units` |
| `workspace not found` | Wrong directory | Run from project root |
| `sidecar not found` | Server not built | Run `pnpm build:server` |

### Debug Build

```bash
# Build with debug symbols
pnpm tauri build --debug

# Check binary size
ls -la apps/desktop/src-tauri/target/release/spiritstream*

# Check server binary
ls -la server/target/release/spiritstream-server*
```

### Type Checking

```bash
# Check all TypeScript
pnpm typecheck

# Check Rust
cargo check --manifest-path server/Cargo.toml
cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml
```

---

**Related:** [Platform Guides](./02-platform-guides.md) | [Distribution Strategy](./03-distribution-strategy.md) | [Release Process](./04-release-process.md)
