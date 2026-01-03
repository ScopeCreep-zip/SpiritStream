# MagillaStream

Multi-destination streaming application that allows you to stream to multiple platforms simultaneously at different bitrates.

## Features

- Stream to YouTube, Twitch, Kick, Facebook, and custom RTMP servers
- Multiple output groups with independent encoding settings
- Hardware encoder support (NVENC, QuickSync, AMF, VideoToolbox)
- Profile management with encrypted stream keys
- Real-time stream statistics
- Cross-platform: macOS, Windows, Linux

## Quick Start

### One-Command Setup

Run the setup script to install all prerequisites automatically:

**macOS / Linux:**
```bash
git clone https://github.com/billboyles/magillastream.git
cd magillastream
./setup.sh
```

**Windows (PowerShell):**
```powershell
git clone https://github.com/billboyles/magillastream.git
cd magillastream
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

Download the latest release from [Releases](https://github.com/billboyles/magillastream/releases):

| Platform | File |
|----------|------|
| macOS | `MagillaStream_x.x.x_aarch64.dmg` |
| Windows | `MagillaStream_x.x.x_x64-setup.exe` |
| Linux | `MagillaStream_x.x.x_amd64.AppImage` or `.deb` |

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
| Rust 1.70+ | `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \| sh` |
| FFmpeg | See above |
| Platform tools | [Tauri Prerequisites](https://tauri.app/start/prerequisites/) |

### Build Steps

```bash
git clone https://github.com/billboyles/magillastream.git
cd magillastream
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
