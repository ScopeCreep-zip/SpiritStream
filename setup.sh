#!/bin/bash
#
# SpiritStream Setup Script
# Installs all prerequisites for building and running SpiritStream
#
# Usage:
#   macOS/Linux: ./setup.sh
#   Windows (Git Bash): ./setup.sh
#   Windows (PowerShell): Run setup.ps1 instead
#

set -euo pipefail  # Strict mode: exit on error, unset vars, pipeline failures
shopt -s inherit_errexit  # Subshells inherit errexit

# Colors for output
readonly RED='\033[0;31m'
readonly GREEN='\033[0;32m'
readonly YELLOW='\033[1;33m'
readonly BLUE='\033[0;34m'
readonly CYAN='\033[0;36m'
readonly BOLD='\033[1m'
readonly NC='\033[0m' # No Color

# Minimum version requirements
readonly MIN_NODE_MAJOR_20=20
readonly MIN_NODE_20_MINOR=19
readonly MIN_NODE_LTS_MAJOR=22
readonly MIN_NODE_22_MINOR=12
readonly MIN_NODE_REQUIREMENT="20.19+ or 22.12+"
readonly MIN_RUST_VERSION="1.70"

# Track verification results
VERIFY_PASSED=true

# Detected OS (set in main)
OS=""

print_header() {
    echo ""
    echo -e "${BLUE}══════════════════════════════════════════════════════════════${NC}"
    echo -e "${BLUE}  SpiritStream Setup${NC}"
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

print_fail() {
    echo -e "${RED}✗ FAIL:${NC} $1"
    VERIFY_PASSED=false
}

print_pass() {
    echo -e "${GREEN}✓ PASS:${NC} $1"
}

# Extract major version number (pure bash, no grep)
get_major_version() {
    local version="$1"
    # Extract leading digits using parameter expansion
    local major="${version%%[!0-9]*}"
    if [[ -z "${major}" ]]; then
        echo "0"
    else
        echo "${major}"
    fi
}

# Extract minor version number (pure bash, no grep)
get_minor_version() {
    local version="${1#v}"
    local remainder="${version#*.}"
    local minor="${remainder%%[!0-9]*}"
    if [[ -z "${minor}" ]]; then
        echo "0"
    else
        echo "${minor}"
    fi
}

is_supported_node_version() {
    local version="$1"
    local major minor
    major=$(get_major_version "${version}")
    minor=$(get_minor_version "${version}")

    if (( major == MIN_NODE_MAJOR_20 && minor >= MIN_NODE_20_MINOR )); then
        return 0
    fi

    if (( ( major == MIN_NODE_LTS_MAJOR && minor >= MIN_NODE_22_MINOR ) || ( major > MIN_NODE_LTS_MAJOR ) )); then
        return 0
    fi

    return 1
}
# Detect OS
detect_os() {
    local uname_out
    uname_out=$(uname -s || true)
    case "${uname_out}" in
        Linux*) echo "linux" ;;
        Darwin*) echo "macos" ;;
        CYGWIN* | MINGW* | MSYS*) echo "windows" ;;
        *) echo "unknown" ;;
    esac
}

# Detect architecture and warn if not x86_64
detect_architecture() {
    local arch
    arch=$(uname -m || true)
    echo "${arch}"
}

# Check if sudo is available and working
check_sudo_access() {
    if ! command -v sudo > /dev/null 2>&1; then
        print_error "sudo command not found. Please install sudo or run as root."
        exit 1
    fi

    if ! sudo -v 2>/dev/null; then
        print_error "This script requires sudo access. Please ensure you have sudo privileges."
        exit 1
    fi

    print_success "sudo access verified"
}

# Install Rust
install_rust() {
    local has_rustc=false
    if command -v rustc > /dev/null 2>&1; then
        has_rustc=true
    fi

    if [[ "${has_rustc}" = true ]]; then
        local rust_version
        rust_version=$(rustc --version || true)
        print_success "Rust already installed: ${rust_version}"
    else
        print_step "Installing Rust..."
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y || {
            print_error "Failed to install Rust"
            return 1
        }
        # shellcheck source=/dev/null
        source "${HOME}/.cargo/env" 2> /dev/null || true
        print_success "Rust installed"
    fi
}

# Install FFmpeg
install_ffmpeg() {
    local target_os="$1"

    local has_ffmpeg=false
    if command -v ffmpeg > /dev/null 2>&1; then
        has_ffmpeg=true
    fi

    if [[ "${has_ffmpeg}" = true ]]; then
        local ffmpeg_version
        ffmpeg_version=$(ffmpeg -version 2>&1 | head -n1 || true)
        print_success "FFmpeg already installed: ${ffmpeg_version}"
        return 0
    fi

    print_step "Installing FFmpeg..."

    case "${target_os}" in
        macos)
            local has_brew=false
            if command -v brew > /dev/null 2>&1; then
                has_brew=true
            fi

            if [[ "${has_brew}" = true ]]; then
                brew install ffmpeg
            else
                print_warning "Homebrew not found. Installing Homebrew first..."
                # SC2312: Invoke curl separately to avoid masking its return value
                local homebrew_install_script
                homebrew_install_script=$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)
                local curl_result=$?
                if [[ "${curl_result}" -ne 0 ]]; then
                    print_error "Failed to download Homebrew installer"
                    return 1
                fi
                /bin/bash -c "${homebrew_install_script}" || {
                    print_error "Failed to install Homebrew"
                    return 1
                }
                brew install ffmpeg
            fi
            ;;
        linux)
            # Note: apt-get update already called in install_linux_deps
            local has_apt=false has_dnf=false has_pacman=false
            if command -v apt-get > /dev/null 2>&1; then has_apt=true; fi
            if command -v dnf > /dev/null 2>&1; then has_dnf=true; fi
            if command -v pacman > /dev/null 2>&1; then has_pacman=true; fi

            if [[ "${has_apt}" = true ]]; then
                sudo apt-get install -y ffmpeg
            elif [[ "${has_dnf}" = true ]]; then
                sudo dnf install -y ffmpeg
            elif [[ "${has_pacman}" = true ]]; then
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
        *)
            print_error "Unknown OS for FFmpeg installation: ${target_os}"
            return 1
            ;;
    esac

    print_success "FFmpeg installed"
}

# Install macOS dependencies
install_macos_deps() {
    print_step "Checking macOS dependencies..."

    # Check for Xcode Command Line Tools
    if ! xcode-select -p &> /dev/null; then
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

    local has_apt=false has_dnf=false has_pacman=false
    if command -v apt-get > /dev/null 2>&1; then has_apt=true; fi
    if command -v dnf > /dev/null 2>&1; then has_dnf=true; fi
    if command -v pacman > /dev/null 2>&1; then has_pacman=true; fi

    if [[ "${has_apt}" = true ]]; then
        sudo apt-get update

        # Check if WebKitGTK 4.1 is available (requires Ubuntu 22.04+ or Debian 12+)
        if ! apt-cache show libwebkit2gtk-4.1-dev &>/dev/null; then
            print_error "libwebkit2gtk-4.1-dev not available in your package repositories."
            print_error "Tauri 2.x requires Ubuntu 22.04+, Debian 12+, or equivalent."
            print_error "Ubuntu 20.04 LTS and older are NOT supported."
            exit 1
        fi
        print_success "WebKitGTK 4.1 availability confirmed"

        sudo apt-get install -y \
            build-essential \
            curl \
            wget \
            file \
            git \
            pkg-config \
            libssl-dev \
            libgtk-3-dev \
            libglib2.0-dev \
            libcairo2-dev \
            libayatana-appindicator3-dev \
            librsvg2-dev \
            libwebkit2gtk-4.1-dev \
            libjavascriptcoregtk-4.1-dev \
            libsoup-3.0-dev \
            pciutils \
            libfuse2 \
            liblzma-dev \
            xz-utils
    elif [[ "${has_dnf}" = true ]]; then
        sudo dnf install -y \
            gcc \
            gcc-c++ \
            make \
            curl \
            wget \
            git \
            pkg-config \
            openssl-devel \
            gtk3-devel \
            glib2-devel \
            cairo-devel \
            webkit2gtk4.1-devel \
            libayatana-appindicator-gtk3-devel \
            librsvg2-devel \
            libsoup3-devel \
            pciutils \
            fuse-libs \
            xz-devel \
            xz
    elif [[ "${has_pacman}" = true ]]; then
        sudo pacman -S --noconfirm \
            base-devel \
            curl \
            wget \
            git \
            pkgconf \
            openssl \
            gtk3 \
            glib2 \
            cairo \
            webkit2gtk-4.1 \
            libayatana-appindicator \
            librsvg \
            libsoup3 \
            pciutils \
            fuse2 \
            xz
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
    local has_winget=false
    if command -v winget > /dev/null 2>&1; then
        has_winget=true
    fi

    if [[ "${has_winget}" = true ]]; then
        read -p "Would you like to install Visual Studio Build Tools via winget? (y/n) " -n 1 -r
        echo
        if [[ ${REPLY} =~ ^[Yy]$ ]]; then
            winget install Microsoft.VisualStudio.2022.BuildTools --override "--wait --passive --add Microsoft.VisualStudio.Workload.VCTools"
        fi
    fi
}

# Install pnpm dependencies
install_pnpm_deps() {
    if [[ -f "package.json" ]]; then
        if ! command -v pnpm > /dev/null 2>&1; then
            print_error "pnpm not found. Install with: corepack prepare pnpm@9.15.0 --activate (or npm install -g pnpm@9.15.0)"
            return 1
        fi
        print_step "Installing pnpm dependencies..."
        pnpm install
        print_success "pnpm dependencies installed"
    else
        print_error "package.json not found. Run this script from the project root."
        return 1
    fi
}

# Install Tauri CLI
install_tauri_cli() {
    # Check if cargo tauri actually works (not just exists)
    local tauri_version
    if tauri_version=$(cargo tauri --version 2>&1) && [[ "${tauri_version}" == *"tauri-cli"* ]]; then
        print_success "Tauri CLI already installed: ${tauri_version}"
        return 0
    fi

    # Binary missing or broken - install/reinstall
    print_step "Installing Tauri CLI..."
    cargo install tauri-cli --force

    # Verify installation succeeded
    if tauri_version=$(cargo tauri --version 2>&1) && [[ "${tauri_version}" == *"tauri-cli"* ]]; then
        print_success "Tauri CLI installed: ${tauri_version}"
    else
        print_error "Tauri CLI installation failed. Error: ${tauri_version}"
        print_warning "Try manually: cargo install tauri-cli"
        return 1
    fi
}

# Verify all installations
verify_installation() {
    echo ""
    echo -e "${BLUE}══════════════════════════════════════════════════════════════${NC}"
    echo -e "${BLUE}  Verification${NC}"
    echo -e "${BLUE}══════════════════════════════════════════════════════════════${NC}"
    echo ""

    # Verify Rust
    local has_rustc=false
    if command -v rustc > /dev/null 2>&1; then
        has_rustc=true
    fi

    if [[ "${has_rustc}" = true ]]; then
        local rust_ver
        rust_ver=$(rustc --version | awk '{print $2}' || true)
        # SC2310: Inline version comparison to avoid function in conditional context
        local sorted_version
        sorted_version=$(printf '%s\n' "${MIN_RUST_VERSION}" "${rust_ver}" | sort -V | head -n1 || true)
        if [[ "${sorted_version}" = "${MIN_RUST_VERSION}" ]]; then
            print_pass "Rust ${rust_ver} (>= ${MIN_RUST_VERSION} required)"
        else
            print_fail "Rust ${rust_ver} is below minimum ${MIN_RUST_VERSION}"
        fi
    else
        print_fail "Rust not found in PATH"
    fi

    # Verify Cargo
    local has_cargo=false
    if command -v cargo > /dev/null 2>&1; then
        has_cargo=true
    fi

    if [[ "${has_cargo}" = true ]]; then
        local cargo_ver
        cargo_ver=$(cargo --version | awk '{print $2}' || true)
        print_pass "Cargo ${cargo_ver}"
    else
        print_fail "Cargo not found in PATH"
    fi

    # Verify Node.js
    local has_node=false
    if command -v node > /dev/null 2>&1; then
        has_node=true
    fi

    if [[ "${has_node}" = true ]]; then
        local node_ver node_major node_minor
        node_ver=$(node --version | tr -d 'v' || true)
        node_major=$(get_major_version "${node_ver}")
        node_minor=$(get_minor_version "${node_ver}")
        if (( node_major % 2 == 1 )); then
            print_fail "Node.js ${node_ver} is odd-numbered (non-LTS); install ${MIN_NODE_REQUIREMENT}"
        elif is_supported_node_version "${node_ver}"; then
            print_pass "Node.js ${node_ver} (Vite requires ${MIN_NODE_REQUIREMENT})"
        else
            print_fail "Node.js ${node_ver} is too old; Vite requires ${MIN_NODE_REQUIREMENT}"
        fi
    else
        print_fail "Node.js not found in PATH"
    fi

    # Verify pnpm
    local has_pnpm=false
    if command -v pnpm > /dev/null 2>&1; then
        has_pnpm=true
    fi

    if [[ "${has_pnpm}" = true ]]; then
        local pnpm_ver
        pnpm_ver=$(pnpm --version || true)
        print_pass "pnpm ${pnpm_ver}"
    else
        print_fail "pnpm not found in PATH"
    fi

    # Verify FFmpeg
    local has_ffmpeg=false
    if command -v ffmpeg > /dev/null 2>&1; then
        has_ffmpeg=true
    fi

    if [[ "${has_ffmpeg}" = true ]]; then
        local ffmpeg_ver
        ffmpeg_ver=$(ffmpeg -version 2>&1 | head -n1 | awk '{print $3}' || true)
        print_pass "FFmpeg ${ffmpeg_ver}"
    else
        print_fail "FFmpeg not found in PATH"
    fi

    # Verify Tauri CLI
    local cargo_tauri_works=false
    if cargo tauri --version > /dev/null 2>&1; then
        cargo_tauri_works=true
    fi

    if [[ "${cargo_tauri_works}" = true ]]; then
        local tauri_ver
        tauri_ver=$(cargo tauri --version | awk '{print $2}' || true)
        print_pass "Tauri CLI ${tauri_ver}"
    else
        print_fail "Tauri CLI not found (cargo tauri)"
    fi

    # Verify pkg-config (Linux only)
    if [[ "${OS}" = "linux" ]]; then
        local has_pkgconfig=false
        if command -v pkg-config > /dev/null 2>&1; then
            has_pkgconfig=true
        fi

        if [[ "${has_pkgconfig}" = true ]]; then
            local pkg_ver
            pkg_ver=$(pkg-config --version || true)
            print_pass "pkg-config ${pkg_ver}"
        else
            print_fail "pkg-config not found (required for Rust builds)"
        fi
    fi

    # Verify node_modules exists
    if [[ -d "node_modules" ]]; then
        local node_modules_count
        node_modules_count=$(find node_modules -maxdepth 1 -type d 2> /dev/null | wc -l || true)
        print_pass "node_modules installed (${node_modules_count} packages)"
    else
        print_fail "node_modules not found"
    fi

    # Verify Cargo.toml exists (sanity check we're in right directory)
    if [[ -f "src-tauri/Cargo.toml" ]]; then
        print_pass "src-tauri/Cargo.toml found"
    else
        print_fail "src-tauri/Cargo.toml not found (wrong directory?)"
    fi

    echo ""
}

# Main setup function
main() {
    print_header

    OS=$(detect_os)
    echo -e "Detected OS: ${BLUE}${OS}${NC}"

    # Detect and display architecture with warning for non-x86_64
    local ARCH
    ARCH=$(detect_architecture)
    echo -e "Detected Architecture: ${BLUE}${ARCH}${NC}"

    if [[ "${ARCH}" != "x86_64" ]]; then
        print_warning "Architecture ${ARCH} detected."
        print_warning "FFmpeg auto-download only supports x86_64 (amd64)."
        print_warning "You will need to install FFmpeg manually via your package manager."
    fi
    echo ""

    if [[ "${OS}" = "unknown" ]]; then
        print_error "Unknown operating system. Please install dependencies manually."
        exit 1
    fi

    # Verify sudo access for Linux before proceeding
    if [[ "${OS}" = "linux" ]]; then
        check_sudo_access
    fi

    # Step 1: Platform-specific dependencies
    echo -e "\n${BLUE}[1/7]${NC} Platform Dependencies"
    case "${OS}" in
        macos) install_macos_deps ;;
        linux) install_linux_deps ;;
        windows) install_windows_deps ;;
        *)
            print_error "Unsupported OS: ${OS}"
            exit 1
            ;;
    esac

    # Step 2: Install Rust
    echo -e "\n${BLUE}[2/7]${NC} Rust"
    install_rust

    # Step 3: Check Node.js
    echo -e "\n${BLUE}[3/7]${NC} Node.js"
    # SC2310: Invoke function separately to preserve set -e behavior
    local node_found=true
    if ! command -v node > /dev/null 2>&1; then
        node_found=false
    fi
    if [[ "${node_found}" = false ]]; then
        print_warning "Node.js not found"
        print_error "Node.js is required but not installed."
        echo "  Please install Node.js ${MIN_NODE_REQUIREMENT} from https://nodejs.org/ (LTS recommended)."
        echo "  Or use a version manager like nvm, fnm, or mise."
        echo "  Then run this script again."
        exit 1
    fi
    local node_version_str node_version node_major node_minor
    node_version_str=$(node --version || true)
    node_version="${node_version_str#v}"
    node_major=$(get_major_version "${node_version}")
    node_minor=$(get_minor_version "${node_version}")

    if (( node_major % 2 == 1 )); then
        print_warning "Node.js ${node_major}.x is an odd-numbered (non-LTS) release which may cause issues."
        print_error "Node.js ${MIN_NODE_REQUIREMENT} is required. Please install from https://nodejs.org/ (LTS recommended)."
        exit 1
    fi

    if ! is_supported_node_version "${node_version}"; then
        print_warning "Node.js version ${node_major}.${node_minor} is too old. Vite requires ${MIN_NODE_REQUIREMENT}."
        print_error "Please upgrade Node.js from https://nodejs.org/ and rerun this script."
        exit 1
    fi

    print_success "Node.js already installed: ${node_version_str}"

    # Step 4: Install FFmpeg
    echo -e "\n${BLUE}[4/7]${NC} FFmpeg"
    install_ffmpeg "${OS}"

    # Step 5: Install Tauri CLI
    echo -e "\n${BLUE}[5/7]${NC} Tauri CLI"
    install_tauri_cli

    # Step 6: Install pnpm dependencies
    echo -e "\n${BLUE}[6/7]${NC} pnpm Dependencies"
    install_pnpm_deps

    # Step 7: Verify installation
    echo -e "\n${BLUE}[7/7]${NC} Verification"
    verify_installation

    # Final status
    echo -e "${BOLD}══════════════════════════════════════════════════════════════${NC}"
    if [[ "${VERIFY_PASSED}" = true ]]; then
        echo -e "${GREEN}${BOLD}  Setup Complete - All Checks Passed!${NC}"
        echo -e "${BOLD}══════════════════════════════════════════════════════════════${NC}"
        echo ""
        echo "Ready to build! Next steps:"
        echo -e "  ${CYAN}pnpm run dev${NC}      Start development server"
        echo -e "  ${CYAN}pnpm run build${NC}    Build for production"
        echo ""
    else
        echo -e "${RED}${BOLD}  Setup Complete - Some Checks Failed!${NC}"
        echo -e "${BOLD}══════════════════════════════════════════════════════════════${NC}"
        echo ""
        echo -e "${YELLOW}Please review the failed checks above and fix them before building.${NC}"
        echo ""
        echo "Common fixes:"
        echo "  - Restart terminal to reload PATH"
        echo "  - Run 'source ~/.cargo/env' to load Rust"
        echo "  - Re-run this script after fixes"
        echo ""
        exit 1
    fi
}

# Run main
main "$@"
