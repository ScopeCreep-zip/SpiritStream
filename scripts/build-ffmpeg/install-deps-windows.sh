#!/usr/bin/env bash
#
# Install FFmpeg build dependencies on Windows via MSYS2's pacman.
# The mingw-w64-x86_64-* package family is the standard FFmpeg build
# environment on Windows — BtbN's official Windows builds use the same
# stack (https://github.com/BtbN/FFmpeg-Builds), as does the FFmpeg
# project's MSYS2 compilation guide
# (https://trac.ffmpeg.org/wiki/CompilationGuide/MinGW).
#
# Must run from inside an MSYS2 shell (msys2/setup-msys2@v2 in CI).
#
# Usage:
#   bash scripts/build-ffmpeg/install-deps-windows.sh
#
# Idempotent: pacman -S --needed skips installed packages.

set -euo pipefail

PACMAN_PACKAGES=(
    mingw-w64-x86_64-toolchain
    mingw-w64-x86_64-pkgconf
    mingw-w64-x86_64-nasm
    mingw-w64-x86_64-yasm
    # gpg is required by scripts/build-ffmpeg/verify-source.sh.
    gnupg
    mingw-w64-x86_64-dav1d
    mingw-w64-x86_64-lame
    mingw-w64-x86_64-libvmaf
    mingw-w64-x86_64-libvpx
    mingw-w64-x86_64-openssl
    mingw-w64-x86_64-opus
    mingw-w64-x86_64-svt-av1
    mingw-w64-x86_64-x264
    mingw-w64-x86_64-x265
    git
)

pacman -S --needed --noconfirm "${PACMAN_PACKAGES[@]}"

echo "==> Done."
