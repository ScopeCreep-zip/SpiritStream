#!/usr/bin/env bash
#
# Orchestrate the full FFmpeg build-from-source pipeline for a given
# target triple. Called by the GitHub Actions `build-ffmpeg` matrix
# job. Also runnable locally for dev (slow — 30+ min).
#
# Steps:
#   1. Resolve OS family from the target triple.
#   2. Install platform build deps (apt / brew / pacman).
#   3. Verify the ffmpeg.org source tarball (GPG + SHA-256).
#   4. Extract the tarball.
#   5. Fetch hardware-encoder SDK headers (Linux/Windows x86_64 only).
#   6. ./configure with the canonical flag set.
#   7. make -j$(nproc).
#   8. make install to a build-local prefix.
#   9. Strip + copy the resulting ffmpeg(.exe) to the runner's output dir.
#
# Output goes to:
#   $REPO_ROOT/ffmpeg-bin-<TARGET>/ffmpeg-<TARGET>(.exe)
# which the upload-artifact step in the workflow consumes verbatim.
#
# Usage:
#   bash scripts/build-ffmpeg/build.sh <target-triple>

set -euo pipefail
# `set -E` propagates ERR traps into functions/subshells/command-subs —
# combined with `trap '...' ERR` it makes silent set -e bypasses
# (process substitution, command substitution) loud. Critical because
# the previous version of this script hit `mapfile: command not found`
# inside `<(...)` and continued exit-0 because set -e doesn't fire
# inside process substitution by default.
set -E
trap 'echo "build.sh: ERR trap fired at line $LINENO (exit $?)" >&2; exit 1' ERR

TARGET="${1:?usage: build.sh <target-triple>}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

# Output dir — matches the path the workflow uploads as an artifact.
OUT_DIR="$REPO_ROOT/ffmpeg-bin-$TARGET"
mkdir -p "$OUT_DIR"

# Working dir for source extract + build.
WORK_DIR="$(mktemp -d -t ffmpeg-build-XXXXXX)"
trap 'rm -rf "$WORK_DIR"' EXIT
cd "$WORK_DIR"

# Resolve the per-target ext + install prefix for headers.
EXT=""
HEADERS_NEEDED=false
case "$TARGET" in
    *windows*) EXT=".exe" ;;
esac
case "$TARGET" in
    x86_64-pc-windows-msvc | x86_64-unknown-linux-gnu)
        HEADERS_NEEDED=true
        ;;
esac

echo "==> Building FFmpeg for target: $TARGET"
echo "==> Repo root: $REPO_ROOT"
echo "==> Work dir:  $WORK_DIR"
echo "==> Out dir:   $OUT_DIR"

# 1+2. Install build deps. Idempotent — re-runs noop if cache hit
# made an earlier run install everything already.
case "$TARGET" in
    *-apple-darwin)
        bash "$SCRIPT_DIR/install-deps-macos.sh"
        BREW_PREFIX="$(brew --prefix)"
        OPENSSL_PREFIX="$(brew --prefix openssl@3)"
        # Push Homebrew's pkgconfig into PKG_CONFIG_PATH so FFmpeg's
        # configure picks up dav1d / libvpx / libvmaf / x264 / x265 /
        # openssl which all ship .pc files.
        export PKG_CONFIG_PATH="$OPENSSL_PREFIX/lib/pkgconfig:$BREW_PREFIX/lib/pkgconfig:${PKG_CONFIG_PATH:-}"
        # Homebrew's `lame` formula does NOT ship a pkg-config file,
        # so FFmpeg's configure falls back to a C compile test for
        # `lame/lame.h` + `-lmp3lame`. That test only succeeds when
        # the compiler can see Homebrew's include/lib dirs. Same
        # mechanism the Homebrew formula uses internally via its
        # `ENV.cflags` / `ENV.ldflags` plumbing — we replicate by
        # passing them through FFmpeg's own --extra-cflags /
        # --extra-ldflags configure args (set in EXTRA_CONFIGURE_FLAGS
        # below, since they're target-specific).
        export EXTRA_CONFIGURE_FLAGS=(
            "--extra-cflags=-I${BREW_PREFIX}/include"
            "--extra-ldflags=-L${BREW_PREFIX}/lib"
        )
        ;;
    *-unknown-linux-gnu)
        bash "$SCRIPT_DIR/install-deps-linux.sh"
        # apt's libmp3lame-dev installs /usr/include/lame/lame.h and
        # /usr/lib/.../libmp3lame.a in the default compiler search path,
        # so no extra cflags/ldflags needed.
        export EXTRA_CONFIGURE_FLAGS=()
        ;;
    *-pc-windows-msvc)
        bash "$SCRIPT_DIR/install-deps-windows.sh"
        # MSYS2's pkgconf already wired by setup-msys2@v2; mingw64
        # prefix is on the default search path.
        export EXTRA_CONFIGURE_FLAGS=()
        ;;
    *)
        echo "build.sh: unsupported target $TARGET" >&2
        exit 64
        ;;
esac

# 3. Verify the source tarball.
# `tee` instead of `>` so verify-source's output is streamed to the
# console live (important for CI logs) AND captured for the grep below.
# The explicit `pipefail` (already set at the top) ensures a failure in
# `bash verify-source.sh` propagates through the pipe — without it, tee's
# success would mask the upstream failure and we'd silently continue.
bash "$SCRIPT_DIR/verify-source.sh" "$WORK_DIR" | tee "$WORK_DIR/verify.log"
FFMPEG_VERSION="$(grep '^version:' "$WORK_DIR/verify.log" | cut -d: -f2)"
TARBALL_PATH="$(grep '^tarball:' "$WORK_DIR/verify.log" | cut -d: -f2)"
if [[ -z "$FFMPEG_VERSION" || -z "$TARBALL_PATH" ]]; then
    echo "build.sh: verify-source did not emit version/tarball lines" >&2
    exit 70
fi

# 4. Extract.
echo "==> Extracting $TARBALL_PATH"
tar -xf "$TARBALL_PATH"
SRC_DIR="$WORK_DIR/ffmpeg-${FFMPEG_VERSION}"
test -d "$SRC_DIR" || { echo "build.sh: extracted dir missing: $SRC_DIR" >&2; exit 70; }

# 5. Hardware-encoder headers (x86_64 Linux + Windows only).
HEADER_PREFIX="$WORK_DIR/encoder-headers"
if [[ "$HEADERS_NEEDED" == "true" ]]; then
    bash "$SCRIPT_DIR/fetch-encoder-headers.sh" "$HEADER_PREFIX"
    export PKG_CONFIG_PATH="$HEADER_PREFIX/lib/pkgconfig:${PKG_CONFIG_PATH:-}"
    export CPPFLAGS="-I$HEADER_PREFIX/include ${CPPFLAGS:-}"
    export LDFLAGS="-L$HEADER_PREFIX/lib ${LDFLAGS:-}"
fi

# 6. Configure.
INSTALL_PREFIX="$WORK_DIR/install"
mkdir -p "$INSTALL_PREFIX"

# bash 3.2 (Apple's stock /bin/bash) doesn't have `mapfile`. Use a
# portable while-read loop so the script works on the maintainer's
# macOS laptop AND on GitHub-hosted runners (which all ship bash 4+).
CONFIGURE_FLAGS=()
while IFS= read -r flag; do
    [[ -z "$flag" ]] && continue
    CONFIGURE_FLAGS+=("$flag")
done < <(bash "$SCRIPT_DIR/configure-flags.sh" "$TARGET")

# Sanity-check the flag list — empty means configure-flags.sh exited
# non-zero in a way that the while-read loop swallowed (set -e doesn't
# fire on the right side of process substitution).
if [[ "${#CONFIGURE_FLAGS[@]}" -lt 5 ]]; then
    echo "build.sh: configure-flags.sh produced only ${#CONFIGURE_FLAGS[@]} flags (expected >=5)" >&2
    bash "$SCRIPT_DIR/configure-flags.sh" "$TARGET" >&2 || true
    exit 70
fi

echo "==> Configure flags (${#CONFIGURE_FLAGS[@]}):"
printf '       %s\n' "${CONFIGURE_FLAGS[@]}"

cd "$SRC_DIR"
./configure --prefix="$INSTALL_PREFIX" "${CONFIGURE_FLAGS[@]}" "${EXTRA_CONFIGURE_FLAGS[@]:-}"

# 7. Build.
NPROC="$(getconf _NPROCESSORS_ONLN 2>/dev/null || sysctl -n hw.ncpu 2>/dev/null || echo 2)"
echo "==> make -j$NPROC"
make -j"$NPROC"

# 8. Install.
echo "==> make install"
make install

# 9. Strip + copy the single binary to the runner output dir.
INSTALLED_BIN="$INSTALL_PREFIX/bin/ffmpeg$EXT"
if [[ ! -f "$INSTALLED_BIN" ]]; then
    echo "build.sh: expected $INSTALLED_BIN after make install" >&2
    exit 70
fi

# Strip — Apple's strip syntax differs but tolerates the same flag.
if command -v strip >/dev/null; then
    strip "$INSTALLED_BIN" 2>/dev/null || strip -x "$INSTALLED_BIN" 2>/dev/null || true
fi

# Smoke test — both that ffmpeg runs and that the codec set matches
# what configure-flags.sh enabled. Fail loud if any required codec
# went missing during the build.
"$INSTALLED_BIN" -version | head -1
ENCODERS_OUT="$("$INSTALLED_BIN" -hide_banner -encoders 2>&1)"
for required in libx264 libx265 libopus libmp3lame aac; do
    if ! grep -q "$required" <<<"$ENCODERS_OUT"; then
        echo "build.sh: required encoder missing in built ffmpeg: $required" >&2
        exit 70
    fi
done
echo "==> Smoke test passed."

# Final copy with the platform-suffixed name the Tauri sidecar
# resolver and our existing main.rs::resolve_ffmpeg_sidecar both
# probe for.
DEST="$OUT_DIR/ffmpeg-$TARGET$EXT"
cp "$INSTALLED_BIN" "$DEST"
chmod +x "$DEST"

SIZE_MB="$(du -m "$DEST" | awk '{print $1}')"
echo "==> Wrote $DEST (${SIZE_MB} MiB)"
