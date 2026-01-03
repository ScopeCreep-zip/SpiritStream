#!/bin/bash
#
# MagillaStream Setup Script
# Installs all prerequisites for building and running MagillaStream
#
# Usage:
#   macOS/Linux: ./setup.sh
#   Windows (Git Bash): ./setup.sh
#   Windows (PowerShell): Run setup.ps1 instead
#

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

print_header() {
    echo ""
    echo -e "${BLUE}══════════════════════════════════════════════════════════════${NC}"
    echo -e "${BLUE}  MagillaStream Setup${NC}"
    echo -e "${BLUE}══════════════════════════════════════════════════════════════${NC}"
    echo ""
}

print_step() {
    echo -e "${GREEN}▶${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}⚠${NC} $1"
}

print_error() {
    echo -e "${RED}✗${NC} $1"
}

print_success() {
    echo -e "${GREEN}✓${NC} $1"
}

# Detect OS
detect_os() {
    case "$(uname -s)" in
        Linux*)     OS="linux";;
        Darwin*)    OS="macos";;
        CYGWIN*|MINGW*|MSYS*) OS="windows";;
        *)          OS="unknown";;
    esac
    echo $OS
}

# Check if command exists
command_exists() {
    command -v "$1" >/dev/null 2>&1
}

# Install Rust
install_rust() {
    if command_exists rustc; then
        RUST_VERSION=$(rustc --version)
        print_success "Rust already installed: $RUST_VERSION"
    else
        print_step "Installing Rust..."
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
        source "$HOME/.cargo/env" 2>/dev/null || true
        print_success "Rust installed"
    fi
}

# Install Node.js (if not present)
check_node() {
    if command_exists node; then
        NODE_VERSION=$(node --version)
        print_success "Node.js already installed: $NODE_VERSION"
        return 0
    else
        print_warning "Node.js not found"
        return 1
    fi
}

# Install FFmpeg
install_ffmpeg() {
    if command_exists ffmpeg; then
        FFMPEG_VERSION=$(ffmpeg -version 2>&1 | head -n1)
        print_success "FFmpeg already installed: $FFMPEG_VERSION"
        return 0
    fi

    print_step "Installing FFmpeg..."

    case "$1" in
        macos)
            if command_exists brew; then
                brew install ffmpeg
            else
                print_warning "Homebrew not found. Installing Homebrew first..."
                /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
                brew install ffmpeg
            fi
            ;;
        linux)
            if command_exists apt-get; then
                sudo apt-get update
                sudo apt-get install -y ffmpeg
            elif command_exists dnf; then
                sudo dnf install -y ffmpeg
            elif command_exists pacman; then
                sudo pacman -S --noconfirm ffmpeg
            else
                print_error "Could not detect package manager. Please install FFmpeg manually."
                return 1
            fi
            ;;
        windows)
            print_warning "Please install FFmpeg manually on Windows:"
            echo "  Option 1: winget install ffmpeg"
            echo "  Option 2: Download from https://ffmpeg.org/download.html"
            return 1
            ;;
    esac

    print_success "FFmpeg installed"
}

# Install macOS dependencies
install_macos_deps() {
    print_step "Checking macOS dependencies..."

    # Check for Xcode Command Line Tools
    if ! xcode-select -p &>/dev/null; then
        print_step "Installing Xcode Command Line Tools..."
        xcode-select --install
        echo "Please complete the Xcode installation and run this script again."
        exit 1
    fi
    print_success "Xcode Command Line Tools installed"
}

# Install Linux dependencies
install_linux_deps() {
    print_step "Installing Linux dependencies..."

    if command_exists apt-get; then
        sudo apt-get update
        sudo apt-get install -y \
            build-essential \
            curl \
            wget \
            file \
            libssl-dev \
            libayatana-appindicator3-dev \
            librsvg2-dev \
            libwebkit2gtk-4.1-dev \
            libjavascriptcoregtk-4.1-dev
    elif command_exists dnf; then
        sudo dnf install -y \
            gcc \
            gcc-c++ \
            make \
            curl \
            wget \
            openssl-devel \
            webkit2gtk4.1-devel \
            libappindicator-gtk3-devel \
            librsvg2-devel
    elif command_exists pacman; then
        sudo pacman -S --noconfirm \
            base-devel \
            curl \
            wget \
            openssl \
            webkit2gtk-4.1 \
            libappindicator-gtk3 \
            librsvg
    else
        print_error "Could not detect package manager (apt/dnf/pacman)"
        print_warning "Please install Tauri dependencies manually:"
        echo "  https://tauri.app/start/prerequisites/#linux"
        return 1
    fi

    print_success "Linux dependencies installed"
}

# Install Windows dependencies (Git Bash)
install_windows_deps() {
    print_step "Checking Windows dependencies..."

    print_warning "Windows requires additional setup:"
    echo ""
    echo "  1. Microsoft Visual Studio C++ Build Tools"
    echo "     Download: https://visualstudio.microsoft.com/visual-cpp-build-tools/"
    echo "     Select 'Desktop development with C++'"
    echo ""
    echo "  2. WebView2 Runtime (usually pre-installed on Windows 10/11)"
    echo "     Download if needed: https://developer.microsoft.com/en-us/microsoft-edge/webview2/"
    echo ""

    # Check if running in Git Bash with access to winget
    if command_exists winget; then
        read -p "Would you like to install Visual Studio Build Tools via winget? (y/n) " -n 1 -r
        echo
        if [[ $REPLY =~ ^[Yy]$ ]]; then
            winget install Microsoft.VisualStudio.2022.BuildTools --override "--wait --passive --add Microsoft.VisualStudio.Workload.VCTools"
        fi
    fi
}

# Install npm dependencies
install_npm_deps() {
    if [ -f "package.json" ]; then
        print_step "Installing npm dependencies..."
        npm install
        print_success "npm dependencies installed"
    else
        print_warning "package.json not found. Run this script from the project root."
    fi
}

# Main setup function
main() {
    print_header

    OS=$(detect_os)
    echo -e "Detected OS: ${BLUE}$OS${NC}"
    echo ""

    if [ "$OS" = "unknown" ]; then
        print_error "Unknown operating system. Please install dependencies manually."
        exit 1
    fi

    # Step 1: Platform-specific dependencies
    echo -e "\n${BLUE}[1/5]${NC} Platform Dependencies"
    case "$OS" in
        macos)  install_macos_deps ;;
        linux)  install_linux_deps ;;
        windows) install_windows_deps ;;
    esac

    # Step 2: Install Rust
    echo -e "\n${BLUE}[2/5]${NC} Rust"
    install_rust

    # Step 3: Check Node.js
    echo -e "\n${BLUE}[3/5]${NC} Node.js"
    if ! check_node; then
        print_error "Node.js is required but not installed."
        echo "  Please install Node.js 18+ from https://nodejs.org/"
        echo "  Then run this script again."
        exit 1
    fi

    # Step 4: Install FFmpeg
    echo -e "\n${BLUE}[4/5]${NC} FFmpeg"
    install_ffmpeg "$OS"

    # Step 5: Install npm dependencies
    echo -e "\n${BLUE}[5/5]${NC} npm Dependencies"
    install_npm_deps

    # Done
    echo ""
    echo -e "${GREEN}══════════════════════════════════════════════════════════════${NC}"
    echo -e "${GREEN}  Setup Complete!${NC}"
    echo -e "${GREEN}══════════════════════════════════════════════════════════════${NC}"
    echo ""
    echo "Next steps:"
    echo "  1. Restart your terminal (to load Rust environment)"
    echo "  2. Run: npm run dev    (development mode)"
    echo "  3. Run: npm run build  (production build)"
    echo ""
}

# Run main
main "$@"
