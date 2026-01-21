# SpiritStream

Multi-destination streaming application that allows you to stream to multiple platforms simultaneously at different bitrates.

## Features

- Stream to YouTube, Twitch, Kick, Facebook, and custom RTMP servers
- Multiple output groups with independent encoding settings
- Hardware encoder support (NVENC, QuickSync, AMF, VideoToolbox)
- Profile management with encrypted stream keys
- Real-time stream statistics
- Cross-platform: macOS, Windows, Linux

## Documentation

Comprehensive technical documentation is available in the [`docs/`](./docs/) directory.

### Quick Links

| Document | Description |
|----------|-------------|
| [Getting Started](./docs/06-tutorials/01-getting-started.md) | Installation and first run guide |
| [System Overview](./docs/01-architecture/01-system-overview.md) | Architecture and component diagrams |
| [FFmpeg Integration](./docs/04-streaming/01-ffmpeg-integration.md) | Relay architecture and process management |
| [State Management](./docs/03-frontend/02-state-management.md) | Zustand stores and data flow |
| [Services Layer](./docs/02-backend/02-services-layer.md) | Rust backend services |
| [Glossary](./docs/GLOSSARY.md) | Technical terms and definitions |

### Reading Paths

- **Beginners**: Start with [Getting Started](./docs/06-tutorials/01-getting-started.md) → [First Stream](./docs/06-tutorials/02-first-stream.md)
- **Developers**: [System Overview](./docs/01-architecture/01-system-overview.md) → [Services Layer](./docs/02-backend/02-services-layer.md) → [State Management](./docs/03-frontend/02-state-management.md)
- **Complete Documentation**: [docs/README.md](./docs/README.md)

## Quick Start

### One-Command Setup

Run the setup script to install all prerequisites automatically:

**macOS / Linux:**
```bash
git clone https://github.com/ScopeCreep-zip/SpiritStream.git
cd spiritstream
./setup.sh
```

**Windows (PowerShell):**
```powershell
git clone https://github.com/ScopeCreep-zip/SpiritStream.git
cd spiritstream
.\setup.ps1
```

The setup script installs: Rust, FFmpeg, platform build tools, and pnpm dependencies.

After setup completes, restart your terminal and run:
```bash
pnpm dev       # Development mode (all workspaces)
pnpm build     # Production build
```

---

## Download Pre-built Binaries

Download the latest release from [Releases](https://github.com/ScopeCreep-zip/SpiritStream/releases):

| Platform | File |
|----------|------|
| macOS | `SpiritStream_x.x.x_aarch64.dmg` |
| Windows | `SpiritStream_x.x.x_x64-setup.exe` |
| Linux | `SpiritStream_x.x.x_amd64.AppImage` or `.deb` |

**Note:** FFmpeg must be installed separately:
- macOS: `brew install ffmpeg`
- Windows: `winget install ffmpeg`
- Linux: `sudo apt install ffmpeg`

---

## Manual Build from Source

### Prerequisites

| Requirement | Installation |
|-------------|--------------|
| Node.js 18+ | [nodejs.org](https://nodejs.org/) |
| pnpm 8+ | `npm install -g pnpm` |
| Rust 1.70+ | `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \| sh` |
| FFmpeg | See above |
| Platform tools | [Tauri Prerequisites](https://tauri.app/start/prerequisites/) |

### Build Steps

```bash
git clone https://github.com/ScopeCreep-zip/SpiritStream.git
cd spiritstream
pnpm install
pnpm build
```

Build output: `apps/desktop/src-tauri/target/release/bundle/`

## Usage

1. **Create a Profile**: Set up your incoming RTMP URL and configure output groups
2. **Add Stream Targets**: Add your streaming platforms with server URLs and stream keys
3. **Configure Encoding**: Choose video encoder, resolution, bitrate, and audio settings
4. **Start Streaming**: Click "Start Stream" to begin broadcasting to all targets

## Development

This is a pnpm monorepo with multiple workspaces:
- `apps/web` - React frontend (@spiritstream/web)
- `apps/desktop` - Tauri desktop wrapper (@spiritstream/desktop)
- `server` - Standalone Rust HTTP server

```bash
pnpm dev             # Start all workspaces in parallel
pnpm dev:web         # Frontend only (localhost:5173)
pnpm dev:desktop     # Desktop app with server sidecar
pnpm build           # Production build (all workspaces)
pnpm typecheck       # Check TypeScript types
cargo check --manifest-path server/Cargo.toml  # Check Rust server
```

### Backend Server (HTTP/WebSocket)

Run the standalone backend server for browser-based or Docker usage:

```bash
pnpm backend:dev
```

Environment variables:

- `SPIRITSTREAM_HOST` (default: `127.0.0.1`)
- `SPIRITSTREAM_PORT` (default: `8008`)
- `SPIRITSTREAM_DATA_DIR` (default: `./data`)
- `SPIRITSTREAM_LOG_DIR` (default: `./data/logs`)
- `SPIRITSTREAM_THEMES_DIR` (default: `./themes`)
- `SPIRITSTREAM_UI_DIR` (default: `./dist`)
- `SPIRITSTREAM_API_TOKEN` (optional; single shared token for HTTP auth)
- `SPIRITSTREAM_UI_ENABLED` (optional; `1` to serve the web UI from the host)
- `SPIRITSTREAM_UI_URL` (launcher-only; default: `http://localhost:1420` in dev, `http://HOST:PORT` in release)
- `SPIRITSTREAM_SERVER_PATH` (launcher-only; absolute path to the host binary)
- `SPIRITSTREAM_LAUNCHER_HIDE_WINDOW` (launcher-only; `1` to hide launcher window)
- `SPIRITSTREAM_LAUNCHER_OPEN_EXTERNAL` (launcher-only; `1` to open the UI in your browser)

Sample values live in `.env.example`.

Frontend configuration for the web UI:

```bash
VITE_BACKEND_MODE=http
VITE_BACKEND_URL=http://127.0.0.1:8008
VITE_BACKEND_WS_URL=ws://127.0.0.1:8008/ws
VITE_BACKEND_TOKEN=
```

Then start the frontend with:

```bash
VITE_BACKEND_MODE=http pnpm dev:web
```

### Launcher (Desktop Host)

The Tauri desktop binary now starts the host server and can open the UI URL in your default browser.
Use `SPIRITSTREAM_UI_URL` to point it at a local Vite dev server or a cloud UI, and `SPIRITSTREAM_SERVER_PATH`
to override the host binary location.

## License

ISC License - See [LICENSE](LICENSE) for details.
