#!/usr/bin/env bash
#
# Fetch hardware-encoder SDK headers for FFmpeg's --enable-nvenc /
# --enable-libvpl / --enable-amf configure options. Headers only — these
# don't ship runtime libraries, just the C headers FFmpeg needs at compile
# time to emit calls into the vendor-provided drivers.
#
# All three header sets are MIT-licensed and redistributable. The
# resulting FFmpeg binary contains the FFmpeg implementation of the
# encoder wrappers; the runtime call into the actual NVENC / QSV / AMF
# implementation is satisfied by the user's GPU driver at exec time.
#
# Supported on:
#   - Linux x86_64
#   - Windows x86_64
# Skipped (`--disable-nvenc --disable-libvpl --disable-amf` set by build.sh) on:
#   - macOS (uses VideoToolbox from the system SDK)
#   - Linux aarch64 (no working NVENC/QSV/AMF on Linux ARM)
#
# Usage:
#   bash scripts/build-ffmpeg/fetch-encoder-headers.sh <install_prefix>
#
# Installs headers under $install_prefix/include/{ffnvcodec,vpl,AMF}.
# pkg-config files land in $install_prefix/lib/pkgconfig so FFmpeg's
# configure picks them up via PKG_CONFIG_PATH.

set -euo pipefail

INSTALL_PREFIX="${1:?usage: fetch-encoder-headers.sh <install_prefix>}"

# Pinned tags for reproducibility. Bump these in tandem with FFmpeg
# version bumps after checking each project's release notes.
NVCODEC_TAG='n13.0.19.0'
LIBVPL_TAG='v2.15.0'
AMF_TAG='v1.4.36'

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

mkdir -p "$INSTALL_PREFIX/include" "$INSTALL_PREFIX/lib/pkgconfig"

echo "==> Fetching nv-codec-headers ${NVCODEC_TAG}"
git clone --depth 1 --branch "$NVCODEC_TAG" \
    https://github.com/FFmpeg/nv-codec-headers \
    "$TMP/nv-codec-headers"
make -C "$TMP/nv-codec-headers" PREFIX="$INSTALL_PREFIX" install

echo "==> Fetching Intel libvpl ${LIBVPL_TAG}"
git clone --depth 1 --branch "$LIBVPL_TAG" \
    https://github.com/intel/libvpl \
    "$TMP/libvpl"
# libvpl uses CMake. We only need the headers + a stub pkg-config; the
# runtime dispatcher lives in the user's Intel driver. Building just the
# API stub keeps the install footprint minimal.
cmake -S "$TMP/libvpl" -B "$TMP/libvpl/build" \
    -DCMAKE_INSTALL_PREFIX="$INSTALL_PREFIX" \
    -DCMAKE_BUILD_TYPE=Release \
    -DBUILD_TESTS=OFF \
    -DBUILD_EXAMPLES=OFF \
    -DBUILD_TOOLS=OFF \
    -DBUILD_DISPATCHER=ON \
    -DBUILD_DISPATCHER_ONEVPL_EXPERIMENTAL=OFF
cmake --build "$TMP/libvpl/build" --target install --parallel "$(getconf _NPROCESSORS_ONLN 2>/dev/null || echo 2)"

echo "==> Fetching AMD AMF ${AMF_TAG}"
git clone --depth 1 --branch "$AMF_TAG" \
    https://github.com/GPUOpen-LibrariesAndSDKs/AMF \
    "$TMP/AMF"
# AMF ships headers under amf/public/include — copy them under our install
# prefix where FFmpeg's configure looks (./include/AMF).
mkdir -p "$INSTALL_PREFIX/include/AMF"
cp -r "$TMP/AMF/amf/public/include/." "$INSTALL_PREFIX/include/AMF/"

echo "==> Done. Encoder headers installed under $INSTALL_PREFIX"
