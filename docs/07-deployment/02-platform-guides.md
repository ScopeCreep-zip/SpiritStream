# Platform Guides

[Documentation](../README.md) > [Deployment](./README.md) > Platform Guides

---

This document provides platform-specific guidance for deploying SpiritStream on Windows, macOS, and Linux.

---

## Windows

### System Requirements

| Requirement | Minimum | Recommended |
|-------------|---------|-------------|
| OS | Windows 10 (1809+) | Windows 11 |
| RAM | 4 GB | 8 GB+ |
| Disk | 200 MB | 500 MB |
| Runtime | WebView2 | (included) |

### Installation

**MSI Installer:**
1. Download `SpiritStream_x.x.x_x64_en-US.msi`
2. Double-click to run installer
3. Follow installation wizard
4. Launch from Start Menu

**NSIS Installer:**
1. Download `SpiritStream_x.x.x_x64-setup.exe`
2. Run installer with admin rights
3. Choose installation directory
4. Complete installation

### WebView2 Runtime

SpiritStream requires Microsoft Edge WebView2:

- Windows 11: Pre-installed
- Windows 10: Auto-installed with app or download from [Microsoft](https://developer.microsoft.com/en-us/microsoft-edge/webview2/)

### File Locations

```
%APPDATA%\SpiritStream\
├── profiles\           # Profile JSON files
├── settings.json       # App settings
└── logs\              # Application logs

%LOCALAPPDATA%\SpiritStream\
└── ffmpeg\            # Downloaded FFmpeg
```

### Firewall Configuration

SpiritStream requires network access:

1. Open Windows Defender Firewall
2. Allow SpiritStream through firewall
3. Enable both private and public networks

### Known Issues

| Issue | Solution |
|-------|----------|
| App won't start | Install WebView2 Runtime |
| FFmpeg not found | Download via Settings or install manually |
| Stream fails | Check Windows Firewall settings |

---

## macOS

### System Requirements

| Requirement | Minimum | Recommended |
|-------------|---------|-------------|
| OS | macOS 10.15 (Catalina) | macOS 12+ |
| RAM | 4 GB | 8 GB+ |
| Disk | 150 MB | 500 MB |
| Architecture | Intel or Apple Silicon | Apple Silicon |

### Installation

**DMG Installer:**
1. Download `SpiritStream_x.x.x_x64.dmg` or `_aarch64.dmg`
2. Open DMG file
3. Drag SpiritStream to Applications
4. Eject DMG

**First Launch:**
1. Right-click app -> Open (bypasses Gatekeeper first time)
2. Or: System Settings -> Privacy & Security -> Open Anyway

### Code Signing

For unsigned builds:

```bash
# Remove quarantine attribute
xattr -dr com.apple.quarantine /Applications/SpiritStream.app
```

### File Locations

```
~/Library/Application Support/SpiritStream/
├── profiles/           # Profile JSON files
├── settings.json       # App settings
└── logs/              # Application logs

~/Library/Caches/SpiritStream/
└── ffmpeg/            # Downloaded FFmpeg
```

### Permissions

SpiritStream may request:
- **Files and Folders**: For profile storage
- **Network**: For streaming

Grant permissions in System Settings -> Privacy & Security.

### Apple Silicon

Native Apple Silicon build provides:
- Better performance
- Lower power consumption
- No Rosetta 2 required

Universal binary works on both architectures.

### Known Issues

| Issue | Solution |
|-------|----------|
| "App is damaged" | Run `xattr -dr com.apple.quarantine` |
| Won't open | Check Gatekeeper settings |
| FFmpeg issues | Install via Homebrew: `brew install ffmpeg` |

---

## Linux

### System Requirements

| Requirement | Minimum | Recommended |
|-------------|---------|-------------|
| OS | Ubuntu 20.04+ | Ubuntu 22.04+ |
| RAM | 4 GB | 8 GB+ |
| Disk | 100 MB | 500 MB |
| Desktop | GTK3 environment | GNOME/KDE |

### Dependencies

**Ubuntu/Debian:**

```bash
sudo apt-get update
sudo apt-get install -y \
  libwebkit2gtk-4.1-0 \
  libappindicator3-1 \
  librsvg2-2 \
  libgtk-3-0
```

**Fedora:**

```bash
sudo dnf install -y \
  webkit2gtk4.1 \
  libappindicator-gtk3 \
  librsvg2
```

**Arch Linux:**

```bash
sudo pacman -S \
  webkit2gtk-4.1 \
  libappindicator-gtk3 \
  librsvg
```

### Installation

**AppImage:**

```bash
# Download AppImage
chmod +x SpiritStream_x.x.x_amd64.AppImage

# Run directly
./SpiritStream_x.x.x_amd64.AppImage

# Or integrate with system
./SpiritStream_x.x.x_amd64.AppImage --appimage-extract
mv squashfs-root ~/.local/share/SpiritStream
```

**Debian Package:**

```bash
sudo dpkg -i spiritstream_x.x.x_amd64.deb
sudo apt-get install -f  # Fix dependencies
```

### File Locations

```
~/.config/spiritstream/
├── profiles/           # Profile JSON files
├── settings.json       # App settings
└── logs/              # Application logs

~/.local/share/spiritstream/
└── ffmpeg/            # Downloaded FFmpeg
```

### Desktop Integration

Create `.desktop` file for AppImage:

```ini
[Desktop Entry]
Type=Application
Name=SpiritStream
Exec=/path/to/SpiritStream.AppImage
Icon=spiritstream
Categories=AudioVideo;Video;
```

### FFmpeg Installation

```bash
# Ubuntu/Debian
sudo apt-get install ffmpeg

# Fedora
sudo dnf install ffmpeg

# Arch
sudo pacman -S ffmpeg
```

### Wayland Support

SpiritStream works on both X11 and Wayland:

```bash
# Force X11 backend if issues on Wayland
GDK_BACKEND=x11 ./SpiritStream.AppImage
```

### Known Issues

| Issue | Solution |
|-------|----------|
| Missing libraries | Install webkit2gtk dependencies |
| Tray icon missing | Install libappindicator3 |
| AppImage won't run | Make executable with `chmod +x` |
| Wayland glitches | Set `GDK_BACKEND=x11` |

---

## FFmpeg Setup

### Automatic Download

SpiritStream can download FFmpeg automatically:
1. Go to Settings -> FFmpeg
2. Click "Download FFmpeg"
3. Wait for download to complete

### Manual Installation

**Windows:**
1. Download a release from [BtbN FFmpeg Builds](https://github.com/BtbN/FFmpeg-Builds/releases) (choose a `ffmpeg-n*-win64-lgpl` or similar archive).
2. Extract the archive:
   - Create `C:\ffmpeg` (for example) and extract the downloaded archive there.
   - You should end up with `C:\ffmpeg\bin\ffmpeg.exe`.
3. Make FFmpeg available to SpiritStream:
   - **Option A – Add to `PATH`:**
     1. Press `Win + R`, type `sysdm.cpl`, and press Enter.
     2. Open **Advanced** → **Environment Variables…**.
     3. Under **System variables**, select **Path** → **Edit** → **New** and add `C:\ffmpeg\bin`.
     4. Click **OK** to save and restart SpiritStream.
   - **Option B – Configure in SpiritStream Settings:**
     1. Open SpiritStream and go to **Settings → FFmpeg**.
     2. Set the **FFmpeg path** to `C:\ffmpeg\bin\ffmpeg.exe`.

**macOS:**
```bash
brew install ffmpeg
```

**Linux:**
```bash
# Package manager (recommended)
sudo apt install ffmpeg
```
Or download a static build from https://github.com/BtbN/FFmpeg-Builds/releases, extract it, and copy the `ffmpeg` binary to `/usr/local/bin` (or set the path in Settings).

### Verify Installation

```bash
ffmpeg -version
```

---

## Auto-Updates

### Windows

- MSI: Manual update (download new installer)
- Future: Windows Update integration planned

### macOS

- DMG: Manual update (download new DMG)
- Future: Sparkle framework integration planned

### Linux

- AppImage: Manual update (download new AppImage)
- DEB: Use package manager for updates
- Future: AppImageUpdate integration planned

---

## Troubleshooting

### General

| Symptom | Check |
|---------|-------|
| Won't start | Dependencies installed? |
| Crashes on launch | Log files for errors |
| Streams fail | FFmpeg installation |
| UI issues | Graphics drivers |

### Logs Location

- Windows: `%APPDATA%\SpiritStream\logs\`
- macOS: `~/Library/Application Support/SpiritStream/logs/`
- Linux: `~/.config/spiritstream/logs/`

### Debug Mode

```bash
# Run with debug logging
RUST_LOG=debug spiritstream
```

---

**Related:** [Building](./01-building.md) | [Release Process](./03-release-process.md)

