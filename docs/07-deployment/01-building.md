# Building SpiritStream

[Documentation](../README.md) > [Deployment](./README.md) > Building

---

This document covers building SpiritStream for development and production across all supported platforms.

---

## Prerequisites

### Required Tools

| Tool | Version | Purpose |
|------|---------|---------|
| Node.js | 18+ | Frontend tooling |
| Rust | 1.70+ | Backend compilation |
| pnpm/npm | Latest | Package management |
| Tauri CLI | 2.x | Build orchestration |

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

# Install dependencies
npm install

# Install Tauri CLI (if not global)
cargo install tauri-cli
```

### Development Server

```bash
# Start development server with hot reload
npm run tauri dev
```

This will:
1. Start Vite dev server on port 5173
2. Compile Rust backend
3. Launch app window with DevTools

### Dev Server Options

```bash
# Specify port
VITE_PORT=3000 npm run tauri dev

# Enable verbose logging
RUST_LOG=debug npm run tauri dev

# Skip frontend (backend only)
npm run tauri dev -- --no-watch
```

---

## Production Build

### Build Command

```bash
# Build for current platform
npm run tauri build
```

### Build Output

```
src-tauri/target/release/
├── spiritstream              # Linux binary
├── spiritstream.exe          # Windows binary
└── bundle/
    ├── msi/                  # Windows installer
    ├── dmg/                  # macOS disk image
    ├── app/                  # macOS application
    ├── appimage/             # Linux AppImage
    └── deb/                  # Linux Debian package
```

---

## Platform-Specific Builds

### Windows

```bash
# Build MSI installer
npm run tauri build -- --target x86_64-pc-windows-msvc

# Build NSIS installer (alternative)
npm run tauri build -- --bundles nsis
```

**Output:**
- `spiritstream_1.0.0_x64-setup.exe` (NSIS)
- `spiritstream_1.0.0_x64_en-US.msi` (MSI)

### macOS

```bash
# Build for Intel Mac
npm run tauri build -- --target x86_64-apple-darwin

# Build for Apple Silicon
npm run tauri build -- --target aarch64-apple-darwin

# Build Universal binary (both)
npm run tauri build -- --target universal-apple-darwin
```

**Output:**
- `SpiritStream_1.0.0_x64.dmg`
- `SpiritStream.app`

### Linux

```bash
# Build AppImage
npm run tauri build -- --target x86_64-unknown-linux-gnu

# Build specific bundle type
npm run tauri build -- --bundles appimage,deb
```

**Output:**
- `spiritstream_1.0.0_amd64.AppImage`
- `spiritstream_1.0.0_amd64.deb`

---

## Build Configuration

### tauri.conf.json

```json
{
  "productName": "SpiritStream",
  "version": "1.0.0",
  "identifier": "com.spiritstream.app",
  "build": {
    "beforeDevCommand": "npm run dev",
    "beforeBuildCommand": "npm run build",
    "devUrl": "http://localhost:5173",
    "frontendDist": "../dist"
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
      "csp": "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'"
    }
  },
  "bundle": {
    "active": true,
    "targets": "all",
    "icon": [
      "icons/32x32.png",
      "icons/128x128.png",
      "icons/128x128@2x.png",
      "icons/icon.icns",
      "icons/icon.ico"
    ],
    "resources": [
      "resources/*"
    ],
    "windows": {
      "wix": {
        "language": "en-US"
      }
    },
    "macOS": {
      "minimumSystemVersion": "10.15"
    },
    "linux": {
      "desktop": {
        "categories": ["Video", "AudioVideo"]
      }
    }
  }
}
```

### Vite Configuration

```typescript
// vite.config.ts
import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import path from 'path';

export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: {
      '@': path.resolve(__dirname, './src-frontend'),
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

---

## Cargo Configuration

### Release Profile

```toml
# Cargo.toml
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
npm run tauri build -- --features devtools
```

---

## Bundle Resources

### Including FFmpeg

```json
// tauri.conf.json
{
  "bundle": {
    "resources": [
      "resources/ffmpeg*"
    ]
  }
}
```

Place binaries in `src-tauri/resources/`:

```
src-tauri/resources/
├── ffmpeg          # Linux
├── ffmpeg.exe      # Windows
└── ffmpeg-macos    # macOS
```

### Sidecar Pattern

For FFmpeg as a sidecar:

```json
// tauri.conf.json
{
  "bundle": {
    "externalBin": [
      "binaries/ffmpeg"
    ]
  }
}
```

---

## Code Signing

### macOS

```bash
# Set signing identity
export APPLE_SIGNING_IDENTITY="Developer ID Application: Your Name (XXXXXXXXXX)"

# Build and sign
npm run tauri build

# Notarize (requires Apple Developer account)
xcrun notarytool submit target/release/bundle/dmg/SpiritStream.dmg \
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

# Use panic=abort
panic = "abort"
```

### Frontend Optimization

```bash
# Analyze bundle size
npm run build -- --sourcemap
npx vite-bundle-visualizer
```

### Typical Bundle Sizes

| Platform | Uncompressed | Compressed |
|----------|--------------|------------|
| Windows (MSI) | ~15 MB | ~8 MB |
| macOS (DMG) | ~12 MB | ~6 MB |
| Linux (AppImage) | ~18 MB | ~10 MB |

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

      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable

      - name: Install dependencies (Linux)
        if: matrix.platform == 'ubuntu-latest'
        run: |
          sudo apt-get update
          sudo apt-get install -y libwebkit2gtk-4.1-dev libappindicator3-dev

      - name: Install npm dependencies
        run: npm ci

      - name: Build
        run: npm run tauri build

      - name: Upload artifacts
        uses: actions/upload-artifact@v4
        with:
          name: binaries-${{ matrix.platform }}
          path: |
            src-tauri/target/release/bundle/
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

### Debug Build

```bash
# Build with debug symbols
npm run tauri build -- --debug

# Check binary size
ls -la src-tauri/target/release/spiritstream*
```

---

**Related:** [Platform Guides](./02-platform-guides.md) | [Release Process](./03-release-process.md)
