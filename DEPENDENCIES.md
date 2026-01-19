# SpiritStream Dependencies

> Comprehensive dependency matrix for building and running SpiritStream across all platforms.

## Table of Contents

- [Dependency Categories](#dependency-categories)
- [End User Dependencies (Install & Run)](#end-user-dependencies-install--run)
- [Developer Dependencies (Build & Develop)](#developer-dependencies-build--develop)
- [Platform-Specific Dependencies](#platform-specific-dependencies)
- [Nix Coverage Matrix](#nix-coverage-matrix)
- [Package Name Cross-Reference](#package-name-cross-reference)

---

## Dependency Categories

### Installation Scope

| Scope | Description |
|-------|-------------|
| **End User** | Required to install and run pre-built SpiritStream binaries |
| **Developer** | Required to build SpiritStream from source |

### Installation Method

| Method | Description |
|--------|-------------|
| **Native Required** | MUST be installed on host system (kernel, drivers, display server) |
| **Native Recommended** | Should be native for best desktop integration (theming, tray icons) |
| **Nix Compatible** | Can be provided by Nix flake/devshell |
| **Runtime** | Needed at application runtime |
| **Build-time** | Only needed during compilation |

---

## End User Dependencies (Install & Run)

These are required for users who download pre-built binaries.

### All Platforms

| Dependency | Purpose | Native | Nix | Status |
|------------|---------|--------|-----|--------|
| - [ ] FFmpeg | Stream encoding/decoding | Recommended | Yes | |

### Linux Native Requirements

| Dependency | Purpose | Native | Nix | Status |
|------------|---------|--------|-----|--------|
| - [ ] Display Server (Wayland/X11) | GUI rendering | Required | No | |
| - [ ] GPU Drivers | Hardware acceleration | Required | No | |
| - [ ] Audio System (PipeWire/PulseAudio) | Audio passthrough | Required | No | |
| - [ ] FUSE kernel module | AppImage support | Required | No | |
| - [ ] D-Bus | IPC for desktop integration | Required | No | |
| - [ ] GTK3 Runtime | GUI toolkit | Recommended | Partial | |
| - [ ] WebKitGTK 4.1 Runtime | Tauri webview | Recommended | Partial | |
| - [ ] libayatana-appindicator | System tray | Recommended | Yes | |

### macOS Native Requirements

| Dependency | Purpose | Native | Nix | Status |
|------------|---------|--------|-----|--------|
| - [ ] macOS 10.15+ (Catalina) | OS requirement | Required | No | |
| - [ ] WebKit (system) | Tauri webview | Required | No | |

### Windows Native Requirements

| Dependency | Purpose | Native | Nix | Status |
|------------|---------|--------|-----|--------|
| - [ ] Windows 10/11 | OS requirement | Required | No | |
| - [ ] WebView2 Runtime | Tauri webview | Required | No | |
| - [ ] Visual C++ Redistributable | Runtime libraries | Required | No | |

---

## Developer Dependencies (Build & Develop)

### Core Toolchain (All Platforms)

| Dependency | Version | Purpose | Native | Nix | Status |
|------------|---------|---------|--------|-----|--------|
| - [ ] Rust | >= 1.70 | Backend compilation | No | Yes | |
| - [ ] Cargo | (with Rust) | Rust package manager | No | Yes | |
| - [ ] Node.js | >= 18 | Frontend build | No | Yes | |
| - [ ] npm | (with Node) | JS package manager | No | Yes | |
| - [ ] pnpm | (optional) | Alternative JS pkg mgr | No | Yes | |
| - [ ] Git | any | Version control | No | Yes | |
| - [ ] cargo-tauri | >= 2.0 | Tauri CLI | No | Yes | |

### Build Tools (All Platforms)

| Dependency | Purpose | Native | Nix | Status |
|------------|---------|--------|-----|--------|
| - [ ] C Compiler (gcc/clang) | Native code compilation | No | Yes | |
| - [ ] C++ Compiler (g++/clang++) | Native code compilation | No | Yes | |
| - [ ] make | Build automation | No | Yes | |
| - [ ] pkg-config / pkgconf | Library discovery | No | Yes | |
| - [ ] curl | Downloads | No | Yes | |
| - [ ] wget | Downloads | No | Yes | |

### Runtime Dependencies (Development)

| Dependency | Purpose | Native | Nix | Status |
|------------|---------|--------|-----|--------|
| - [ ] FFmpeg | Stream testing | Recommended | Yes | |

---

## Platform-Specific Dependencies

### Linux - Tauri 2.x Build Dependencies

> **IMPORTANT**: Tauri 2.x requires WebKitGTK 4.1 which needs Ubuntu 22.04+, Debian 12+, Fedora 36+, or equivalent.

#### System Libraries (Headers + Runtime)

| Dependency | Purpose | Native | Nix | Status |
|------------|---------|--------|-----|--------|
| - [ ] libssl | TLS/crypto | Recommended | Yes | |
| - [ ] libgtk-3 | GUI toolkit | Recommended | Yes | |
| - [ ] libglib-2.0 | GLib utilities | Recommended | Yes | |
| - [ ] libcairo | 2D graphics | Recommended | Yes | |
| - [ ] librsvg | SVG rendering | Recommended | Yes | |
| - [ ] libwebkit2gtk-4.1 | Tauri webview | Recommended | Yes | |
| - [ ] libjavascriptcoregtk-4.1 | JS engine | Recommended | Yes | |
| - [ ] libsoup-3.0 | HTTP client | Recommended | Yes | |
| - [ ] libayatana-appindicator3 | System tray | Recommended | Yes | |

#### Compression Libraries

| Dependency | Purpose | Native | Nix | Status |
|------------|---------|--------|-----|--------|
| - [ ] liblzma / xz | LZMA compression | No | Yes | |
| - [ ] libzstd | Zstandard compression | No | Yes | |

#### Utility Libraries

| Dependency | Purpose | Native | Nix | Status |
|------------|---------|--------|-----|--------|
| - [ ] libfuse2 | AppImage support | Recommended | Partial | |
| - [ ] pciutils | Hardware detection | No | Yes | |
| - [ ] file | File type detection | No | Yes | |

### macOS - Build Dependencies

| Dependency | Purpose | Native | Nix | Status |
|------------|---------|--------|-----|--------|
| - [ ] Xcode Command Line Tools | Compiler, linker, SDKs | Required | No | |
| - [ ] Homebrew | Package manager (optional) | Recommended | No | |

### Windows - Build Dependencies

| Dependency | Purpose | Native | Nix | Status |
|------------|---------|--------|-----|--------|
| - [ ] Visual Studio Build Tools | MSVC compiler | Required | No | |
| - [ ] Windows SDK | Windows headers/libs | Required | No | |
| - [ ] WebView2 SDK | Webview development | Required | No | |

---

## Nix Coverage Matrix

### Fully Nix-Compatible (100% Coverage)

These can be entirely provided by a Nix devshell:

| Category | Dependencies |
|----------|--------------|
| **Languages** | - [ ] Rust, - [ ] Node.js, - [ ] npm/pnpm |
| **Build Tools** | - [ ] gcc/clang, - [ ] g++/clang++, - [ ] make, - [ ] pkg-config, - [ ] cmake |
| **CLI Tools** | - [ ] git, - [ ] curl, - [ ] wget, - [ ] cargo-tauri, - [ ] file |
| **Compression** | - [ ] xz/liblzma, - [ ] zstd |
| **Media** | - [ ] FFmpeg |
| **Tauri Libs** | - [ ] openssl, - [ ] gtk3, - [ ] glib, - [ ] cairo, - [ ] librsvg, - [ ] webkit2gtk-4.1, - [ ] javascriptcoregtk-4.1, - [ ] libsoup3, - [ ] libayatana-appindicator |

### Partially Nix-Compatible (Desktop Integration Issues)

These work in Nix but may have theming/integration issues:

| Dependency | Issue | Workaround |
|------------|-------|------------|
| - [ ] GTK3 | Theme mismatch with host | Use `gsettings-desktop-schemas` |
| - [ ] WebKitGTK | Font/theme issues | May need host fonts |
| - [ ] libfuse2 | Requires kernel module | FUSE must be enabled on host |

### Native-Only (Cannot Use Nix)

| Dependency | Reason |
|------------|--------|
| - [ ] Linux kernel | OS level |
| - [ ] Wayland/X11 | Display server |
| - [ ] GPU drivers | Hardware |
| - [ ] PipeWire/PulseAudio | Audio system |
| - [ ] D-Bus daemon | System service |
| - [ ] FUSE kernel module | Kernel feature |
| - [ ] macOS (system) | Apple platform |
| - [ ] Xcode CLT | Apple toolchain |
| - [ ] Windows (system) | Microsoft platform |
| - [ ] Visual Studio Build Tools | Microsoft toolchain |
| - [ ] WebView2 | Windows component |

---

## Package Name Cross-Reference

### Core Toolchain

| Dependency | apt (Debian/Ubuntu) | dnf (Fedora/RHEL) | pacman (Arch) | brew (macOS) | winget (Windows) | nixpkgs |
|------------|---------------------|-------------------|---------------|--------------|------------------|---------|
| Rust | - [ ] | - [ ] | - [ ] | - [ ] | - [ ] | - [ ] rust-bin.stable.latest.default |
| Node.js | - [ ] nodejs | - [ ] nodejs | - [ ] nodejs | - [ ] node | - [ ] | - [ ] nodejs_22 |
| npm | - [ ] npm | - [ ] npm | - [ ] npm | - [ ] (with node) | - [ ] | - [ ] (with nodejs) |
| Git | - [ ] git | - [ ] git | - [ ] git | - [ ] git | - [ ] Git.Git | - [ ] git |
| cargo-tauri | - [ ] | - [ ] | - [ ] | - [ ] | - [ ] | - [ ] cargo-tauri |

### Build Tools

| Dependency | apt (Debian/Ubuntu) | dnf (Fedora/RHEL) | pacman (Arch) | brew (macOS) | nixpkgs |
|------------|---------------------|-------------------|---------------|--------------|---------|
| C Compiler | - [ ] gcc | - [ ] gcc | - [ ] gcc | - [ ] (Xcode) | - [ ] gcc |
| C++ Compiler | - [ ] g++ | - [ ] gcc-c++ | - [ ] gcc | - [ ] (Xcode) | - [ ] gcc |
| make | - [ ] make | - [ ] make | - [ ] make | - [ ] (Xcode) | - [ ] gnumake |
| pkg-config | - [ ] pkg-config | - [ ] pkg-config | - [ ] pkgconf | - [ ] pkg-config | - [ ] pkg-config |
| Build Essentials | - [ ] build-essential | - [ ] @development-tools | - [ ] base-devel | - [ ] | - [ ] stdenv |
| curl | - [ ] curl | - [ ] curl | - [ ] curl | - [ ] curl | - [ ] curl |
| wget | - [ ] wget | - [ ] wget | - [ ] wget | - [ ] wget | - [ ] wget |
| file | - [ ] file | - [ ] file | - [ ] file | - [ ] (system) | - [ ] file |

### SSL/TLS

| Dependency | apt (Debian/Ubuntu) | dnf (Fedora/RHEL) | pacman (Arch) | brew (macOS) | nixpkgs |
|------------|---------------------|-------------------|---------------|--------------|---------|
| OpenSSL (dev) | - [ ] libssl-dev | - [ ] openssl-devel | - [ ] openssl | - [ ] openssl | - [ ] openssl |

### GTK/GUI Stack

| Dependency | apt (Debian/Ubuntu) | dnf (Fedora/RHEL) | pacman (Arch) | nixpkgs |
|------------|---------------------|-------------------|---------------|---------|
| GTK3 (dev) | - [ ] libgtk-3-dev | - [ ] gtk3-devel | - [ ] gtk3 | - [ ] gtk3 |
| GLib (dev) | - [ ] libglib2.0-dev | - [ ] glib2-devel | - [ ] glib2 | - [ ] glib |
| Cairo (dev) | - [ ] libcairo2-dev | - [ ] cairo-devel | - [ ] cairo | - [ ] cairo |
| librsvg (dev) | - [ ] librsvg2-dev | - [ ] librsvg2-devel | - [ ] librsvg | - [ ] librsvg |

### WebKitGTK (Tauri 2.x)

| Dependency | apt (Debian/Ubuntu) | dnf (Fedora/RHEL) | pacman (Arch) | nixpkgs |
|------------|---------------------|-------------------|---------------|---------|
| WebKitGTK 4.1 (dev) | - [ ] libwebkit2gtk-4.1-dev | - [ ] webkit2gtk4.1-devel | - [ ] webkit2gtk-4.1 | - [ ] webkitgtk_4_1 |
| JavaScriptCore 4.1 (dev) | - [ ] libjavascriptcoregtk-4.1-dev | - [ ] (with webkit) | - [ ] (with webkit) | - [ ] (with webkitgtk) |
| libsoup 3.0 (dev) | - [ ] libsoup-3.0-dev | - [ ] libsoup3-devel | - [ ] libsoup3 | - [ ] libsoup_3 |

### System Tray

| Dependency | apt (Debian/Ubuntu) | dnf (Fedora/RHEL) | pacman (Arch) | nixpkgs |
|------------|---------------------|-------------------|---------------|---------|
| AppIndicator | - [ ] libayatana-appindicator3-dev | - [ ] libayatana-appindicator-gtk3-devel | - [ ] libayatana-appindicator | - [ ] libayatana-appindicator |

### Compression

| Dependency | apt (Debian/Ubuntu) | dnf (Fedora/RHEL) | pacman (Arch) | brew (macOS) | nixpkgs |
|------------|---------------------|-------------------|---------------|--------------|---------|
| xz/liblzma (dev) | - [ ] liblzma-dev | - [ ] xz-devel | - [ ] xz | - [ ] xz | - [ ] xz |
| xz (runtime) | - [ ] xz-utils | - [ ] xz | - [ ] xz | - [ ] xz | - [ ] xz |
| zstd (dev) | - [ ] libzstd-dev | - [ ] libzstd-devel | - [ ] zstd | - [ ] zstd | - [ ] zstd |

### Utilities

| Dependency | apt (Debian/Ubuntu) | dnf (Fedora/RHEL) | pacman (Arch) | nixpkgs |
|------------|---------------------|-------------------|---------------|---------|
| pciutils | - [ ] pciutils | - [ ] pciutils | - [ ] pciutils | - [ ] pciutils |
| FUSE2 | - [ ] libfuse2 | - [ ] fuse-libs | - [ ] fuse2 | - [ ] fuse |

### Media

| Dependency | apt (Debian/Ubuntu) | dnf (Fedora/RHEL) | pacman (Arch) | brew (macOS) | winget (Windows) | nixpkgs |
|------------|---------------------|-------------------|---------------|--------------|------------------|---------|
| FFmpeg | - [ ] ffmpeg | - [ ] ffmpeg | - [ ] ffmpeg | - [ ] ffmpeg | - [ ] ffmpeg | - [ ] ffmpeg |

---

## Minimum OS Versions

### Linux

| Distribution | Minimum Version | Reason |
|--------------|-----------------|--------|
| Ubuntu | 22.04 LTS | WebKitGTK 4.1 |
| Debian | 12 (Bookworm) | WebKitGTK 4.1 |
| Fedora | 36+ | WebKitGTK 4.1 |
| Arch Linux | Rolling | Always current |
| Pop!_OS | 22.04+ | Based on Ubuntu |
| Linux Mint | 21+ | Based on Ubuntu 22.04 |

### macOS

| Version | Minimum | Reason |
|---------|---------|--------|
| macOS | 10.15 (Catalina) | Tauri requirement |

### Windows

| Version | Minimum | Reason |
|---------|---------|--------|
| Windows | 10 (1803+) | WebView2 support |
| Windows | 11 | Full support |

---

## Verification Commands

### Check Installed Versions

```bash
# Rust
rustc --version
cargo --version

# Node.js
node --version
npm --version

# FFmpeg
ffmpeg -version

# Tauri CLI
cargo tauri --version

# pkg-config
pkg-config --version

# GTK (Linux)
pkg-config --modversion gtk+-3.0

# WebKitGTK (Linux)
pkg-config --modversion webkit2gtk-4.1
```

---

## Notes

1. **Nix Users**: Most dependencies can be provided via `github:braincraftio/konductor#frontend` devshell
2. **Non-Nix Users**: Use `setup.sh` or install dependencies manually per platform
3. **Windows**: Nix is not supported; use native tools only
4. **macOS**: Xcode CLT is always required regardless of Nix usage

---

*Last Updated: 2025-01-18*
*SpiritStream Version: 0.1.0*
*Tauri Version: 2.x*
