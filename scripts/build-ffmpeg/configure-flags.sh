#!/usr/bin/env bash
#
# Print the FFmpeg ./configure flag list for a given target triple to
# stdout, one flag per line. Sourced by build.sh; not invoked directly.
#
# The base flag set mirrors Homebrew's ffmpeg.rb formula install args
# (https://github.com/Homebrew/homebrew-core/blob/master/Formula/f/ffmpeg.rb)
# with three deliberate differences:
#   1. --enable-static --disable-shared --pkg-config-flags="--static"
#      (Homebrew ships a shared build; we want a single self-contained
#      sidecar binary that matches the existing Command::new(path)
#      spawn pattern in ffmpeg_handler.rs).
#   2. --disable-ffplay --disable-doc --disable-debug
#      (size reduction — we don't ship ffplay, manpages, or symbols).
#   3. Hardware encoders enabled per-platform
#      (Homebrew defers these to the separate ffmpeg-full formula;
#      we want hardware acceleration for vulnerable users on older
#      hardware where software encode is unwatchable).
#
# Codec set follows Homebrew's `--enable-*` list verbatim except
# libfdk_aac is intentionally absent (requires --enable-nonfree,
# blocks redistribution). FFmpeg's native AAC encoder is used instead.
#
# Usage:
#   bash scripts/build-ffmpeg/configure-flags.sh <target-triple>

set -euo pipefail

TARGET="${1:?usage: configure-flags.sh <target-triple>}"

# Cross-platform base flags — same on every runner.
print_flag() { printf '%s\n' "$1"; }

print_flag '--enable-static'
print_flag '--disable-shared'
print_flag '--pkg-config-flags=--static'
print_flag '--enable-pthreads'
print_flag '--enable-version3'
print_flag '--enable-gpl'

# Codec libraries (matches Homebrew's --enable-* list).
print_flag '--enable-libx264'
print_flag '--enable-libx265'
print_flag '--enable-libdav1d'
print_flag '--enable-libsvtav1'
print_flag '--enable-libvpx'
print_flag '--enable-libvmaf'
print_flag '--enable-libmp3lame'
print_flag '--enable-libopus'
print_flag '--enable-openssl'

# Size reduction.
print_flag '--disable-ffplay'
print_flag '--disable-doc'
print_flag '--disable-debug'

# Per-platform additions.
case "$TARGET" in
    aarch64-apple-darwin)
        print_flag '--enable-videotoolbox'
        print_flag '--enable-audiotoolbox'
        print_flag '--enable-neon'
        ;;
    x86_64-apple-darwin)
        print_flag '--enable-videotoolbox'
        print_flag '--enable-audiotoolbox'
        ;;
    x86_64-pc-windows-msvc)
        print_flag '--enable-nvenc'
        print_flag '--enable-libvpl'
        print_flag '--enable-amf'
        ;;
    x86_64-unknown-linux-gnu)
        print_flag '--enable-nvenc'
        print_flag '--enable-libvpl'
        print_flag '--enable-amf'
        ;;
    aarch64-unknown-linux-gnu)
        # No working hardware encoders on Linux ARM as of 2026.
        # NVIDIA's Tegra path doesn't expose NVENC the same way;
        # Intel QSV is x86-only; AMF is x86-only. Explicit disable
        # makes the runtime behavior unambiguous.
        print_flag '--disable-nvenc'
        ;;
    *)
        echo "configure-flags: unsupported target $TARGET" >&2
        exit 64
        ;;
esac
