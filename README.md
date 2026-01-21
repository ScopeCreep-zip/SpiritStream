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

The setup script installs: Rust, FFmpeg, platform build tools, and npm dependencies.

After setup completes, restart your terminal and run:
```bash
npm run dev    # Development mode
npm run build  # Production build
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
- Windows: Download from https://github.com/BtbN/FFmpeg-Builds/releases and add to PATH (or set the path in Settings).
- Linux: Use your package manager (apt/dnf/pacman) or download a static build from the same BtbN link.
- macOS: `brew install ffmpeg`

---

## Manual Build from Source

### Prerequisites

| Requirement | Installation |
|-------------|--------------|
| Node.js 18+ | [nodejs.org](https://nodejs.org/) |
| Rust 1.70+ | `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \| sh` |
| FFmpeg | See above |
| Platform tools | [Tauri Prerequisites](https://tauri.app/start/prerequisites/) |

### Build Steps

```bash
git clone https://github.com/ScopeCreep-zip/SpiritStream.git
cd spiritstream
npm install
npm run build
```

Build output: `src-tauri/target/release/bundle/`

## Usage

1. **Create a Profile**: Set up your incoming RTMP URL and configure output groups
2. **Add Stream Targets**: Add your streaming platforms with server URLs and stream keys
3. **Configure Encoding**: Choose video encoder, resolution, bitrate, and audio settings
4. **Start Streaming**: Click "Start Stream" to begin broadcasting to all targets

## Development

```bash
npm run dev          # Start development server with hot reload
npm run build        # Production build
npm run build:debug  # Debug build with symbols
npm run typecheck    # Check TypeScript types
npm run check        # Check Rust code
```

## License

ISC License - See [LICENSE](LICENSE) for details.
