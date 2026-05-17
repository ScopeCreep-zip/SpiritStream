#!/usr/bin/env bash
#
# Install FFmpeg build dependencies on Debian/Ubuntu via apt. Mirrors
# Homebrew's depends_on list plus the on_linux additions (alsa-lib,
# libxcb, xz, zlib) plus the uses_from_macos shims that Linux needs
# from apt instead of the macOS system SDK (bzip2, libxml2).
#
# Hardware encoder headers (nv-codec-headers / libvpl / AMF) are
# fetched separately by scripts/build-ffmpeg/fetch-encoder-headers.sh
# because they're git-clones, not apt packages.
#
# Usage:
#   bash scripts/build-ffmpeg/install-deps-linux.sh
#
# Idempotent: apt-get install skips already-installed packages.

set -euo pipefail

APT_PACKAGES=(
    # Build toolchain
    build-essential
    pkg-config
    nasm
    yasm
    git
    autoconf
    automake
    libtool
    # gpg is required by scripts/build-ffmpeg/verify-source.sh.
    gnupg

    # Codec libs (matches Homebrew depends_on)
    libdav1d-dev
    libmp3lame-dev
    libvmaf-dev
    libvpx-dev
    libopus-dev
    libsvtav1-dev
    libx264-dev
    libx265-dev

    # Cryptographic + IO
    libssl-dev
    libbz2-dev
    libxml2-dev
    zlib1g-dev
    liblzma-dev

    # System libs (Homebrew on_linux block)
    libasound2-dev
    libxcb1-dev
    libxcb-shm0-dev
    libxcb-xfixes0-dev
)

# Match the runner's package cache state once, then install in a single
# apt call to keep the package-manager cost flat.
sudo apt-get update
sudo apt-get install -y --no-install-recommends "${APT_PACKAGES[@]}"

echo "==> Done."
