# MagillaStream

Multi-destination streaming application that allows you to stream to multiple platforms simultaneously at different bitrates.

## Features

- Stream to YouTube, Twitch, Kick, Facebook, and custom RTMP servers
- Multiple output groups with independent encoding settings
- Hardware encoder support (NVENC, QuickSync, AMF, VideoToolbox)
- Profile management with encrypted stream keys
- Real-time stream statistics
- Cross-platform: macOS, Windows, Linux

## Requirements

- **FFmpeg** must be installed on your system
  - macOS: `brew install ffmpeg`
  - Windows: Download from [ffmpeg.org](https://ffmpeg.org/download.html) or use `winget install ffmpeg`
  - Linux: `sudo apt install ffmpeg` (Debian/Ubuntu) or `sudo dnf install ffmpeg` (Fedora)

## Installation

### Download Pre-built Binaries

Download the latest release for your platform from the [Releases](https://github.com/billboyles/magillastream/releases) page:

| Platform | File |
|----------|------|
| macOS | `MagillaStream_x.x.x_aarch64.dmg` or `MagillaStream.app` |
| Windows | `MagillaStream_x.x.x_x64-setup.exe` |
| Linux | `MagillaStream_x.x.x_amd64.AppImage` or `.deb` |

### Build from Source

#### Prerequisites

- [Node.js](https://nodejs.org/) 18+
- [Rust](https://rustup.rs/) 1.70+ (required - run `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`)
- Platform-specific dependencies: See [Tauri Prerequisites](https://tauri.app/start/prerequisites/)

#### Steps

```bash
# Clone the repository
git clone https://github.com/billboyles/magillastream.git
cd magillastream

# Install dependencies
npm install

# Development mode
npm run dev

# Production build
npm run build
```

Build output will be in `src-tauri/target/release/bundle/`.

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
