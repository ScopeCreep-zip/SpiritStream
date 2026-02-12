#!/bin/bash
#
# Patch SpiritStream binary for distribution on non-NixOS Linux systems
#
# When building inside a Nix devshell, binaries are linked against /nix/store paths.
# This script patches the binary to use standard FHS paths so it runs on Ubuntu, Fedora, etc.
#

set -euo pipefail

# Get product name from tauri.conf.json (lowercase)
PRODUCT_NAME=$(grep -o '"productName": *"[^"]*"' apps/desktop/src-tauri/tauri.conf.json | cut -d'"' -f4 | tr '[:upper:]' '[:lower:]')
BINARY="apps/desktop/src-tauri/target/release/${PRODUCT_NAME:-spiritstream}"

# Only patch on Linux
if [[ "$(uname -s)" != "Linux" ]]; then
    echo "Skipping binary patching (not Linux)"
    exit 0
fi

# Only patch if patchelf is available (indicates Nix environment)
if ! command -v patchelf &> /dev/null; then
    echo "patchelf not found, skipping binary patching"
    exit 0
fi

# Check if binary exists
if [[ ! -f "$BINARY" ]]; then
    echo "Binary not found: $BINARY"
    exit 1
fi

# Check if binary has Nix interpreter (needs patching)
INTERP=$(patchelf --print-interpreter "$BINARY" 2>/dev/null || true)
if [[ "$INTERP" != *"/nix/store/"* ]]; then
    echo "Binary already has system interpreter, skipping"
    exit 0
fi

echo "Patching binary for distribution..."
echo "  Current interpreter: $INTERP"

# Set standard FHS interpreter
# Set standard FHS interpreter based on architecture
case "$(uname -m)" in
    x86_64)  INTERP="/lib64/ld-linux-x86-64.so.2" ;;
    aarch64) INTERP="/lib/ld-linux-aarch64.so.1" ;;
    *)       echo "Unsupported architecture: $(uname -m)"; exit 1 ;;
esac
patchelf --set-interpreter "$INTERP" "$BINARY"

# Set RPATH to standard system library paths
# These cover Debian/Ubuntu, Fedora/RHEL, and generic Linux
patchelf --set-rpath /usr/lib/x86_64-linux-gnu:/usr/lib64:/usr/lib:/lib/x86_64-linux-gnu:/lib64:/lib "$BINARY"

echo "  New interpreter: $(patchelf --print-interpreter "$BINARY")"
echo "  New RPATH: $(patchelf --print-rpath "$BINARY")"
echo "Binary patched successfully"
