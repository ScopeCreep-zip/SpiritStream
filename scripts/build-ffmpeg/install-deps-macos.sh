#!/usr/bin/env bash
#
# Install FFmpeg build dependencies on macOS via Homebrew. Mirrors
# Homebrew's own ffmpeg.rb `depends_on` list (the gold-standard set)
# minus the runtime-only / shared-build deps we don't need for a
# static binary. nasm is required even on arm64 because some codec
# assembly paths (libx264 / libx265) use it across architectures.
#
# Usage:
#   bash scripts/build-ffmpeg/install-deps-macos.sh
#
# Idempotent: brew install skips already-installed formulae.

set -euo pipefail

BREW_PACKAGES=(
    pkgconf
    nasm
    # gnupg is required by scripts/build-ffmpeg/verify-source.sh — the
    # GitHub-hosted macOS runners ship gpg in the base image, but the
    # maintainer's local laptop might not have it (Apple stopped
    # shipping system gpg around 10.10). Install explicitly so the
    # local-build path matches CI.
    gnupg
    dav1d
    lame
    libvmaf
    libvpx
    openssl@3
    opus
    svt-av1
    x264
    x265
)

echo "==> brew install ${BREW_PACKAGES[*]}"
brew install "${BREW_PACKAGES[@]}"

# openssl@3 is keg-only on Homebrew. Surface the prefix so the build
# orchestrator can wire pkg-config to find it.
OPENSSL_PREFIX="$(brew --prefix openssl@3)"
echo "openssl_prefix:$OPENSSL_PREFIX"
echo "==> Done."
