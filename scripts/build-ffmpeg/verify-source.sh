#!/usr/bin/env bash
#
# Verify the FFmpeg source tarball pinned in scripts/ffmpeg-pins.json
# against ffmpeg.org's published GPG signature AND our SHA-256 pin.
#
# Both checks must pass — they catch independent failure modes:
#   - GPG verifies that the bytes were signed by the FFmpeg release key
#     (catches TLS-intercept or mirror substitution).
#   - SHA-256 verifies that the bytes match what we last reviewed
#     (catches a compromised release key signing different bytes —
#     low-probability but cheap to defend against).
#
# Usage:
#   bash scripts/build-ffmpeg/verify-source.sh [out_dir]
#
# Writes ffmpeg-<version>.tar.xz to $out_dir (default cwd) on success.
# Exits non-zero on any verification failure.
#
# The pin file is the single source of truth:
#   scripts/ffmpeg-pins.json::ffmpegVersion
#   scripts/ffmpeg-pins.json::sourceSha256
#   scripts/ffmpeg-pins.json::gpgFingerprint
#
# Inspired by Debian's reproducible-builds + Homebrew formula audit;
# FFmpeg signing key documented at https://ffmpeg.org/download.html
# and hosted on the project's own HTTPS at https://ffmpeg.org/ffmpeg-devel.asc

set -euo pipefail

OUT_DIR="${1:-$PWD}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
PIN_FILE="$REPO_ROOT/scripts/ffmpeg-pins.json"

if [[ ! -f "$PIN_FILE" ]]; then
    echo "verify-source: pin file not found: $PIN_FILE" >&2
    exit 65
fi

# jq is available on every supported runner (preinstalled on GitHub-hosted
# macOS/ubuntu, available via MSYS2 base install). Fail loud if missing.
command -v jq >/dev/null || { echo "verify-source: jq not in PATH" >&2; exit 69; }
command -v gpg >/dev/null || { echo "verify-source: gpg not in PATH" >&2; exit 69; }
command -v curl >/dev/null || { echo "verify-source: curl not in PATH" >&2; exit 69; }

FFMPEG_VERSION="$(jq -r .ffmpegVersion "$PIN_FILE")"
EXPECTED_SHA256="$(jq -r .sourceSha256 "$PIN_FILE")"
EXPECTED_FINGERPRINT="$(jq -r .gpgFingerprint "$PIN_FILE")"

if [[ -z "$FFMPEG_VERSION" || "$FFMPEG_VERSION" == "null" ]]; then
    echo "verify-source: ffmpegVersion missing in $PIN_FILE" >&2
    exit 65
fi
if [[ -z "$EXPECTED_SHA256" || "$EXPECTED_SHA256" == "null" ]]; then
    echo "verify-source: sourceSha256 missing in $PIN_FILE" >&2
    exit 65
fi
if [[ -z "$EXPECTED_FINGERPRINT" || "$EXPECTED_FINGERPRINT" == "null" ]]; then
    echo "verify-source: gpgFingerprint missing in $PIN_FILE" >&2
    exit 65
fi

TARBALL="ffmpeg-${FFMPEG_VERSION}.tar.xz"
ASC="${TARBALL}.asc"
TARBALL_URL="https://ffmpeg.org/releases/${TARBALL}"
ASC_URL="https://ffmpeg.org/releases/${ASC}"
KEY_URL="https://ffmpeg.org/ffmpeg-devel.asc"

mkdir -p "$OUT_DIR"
cd "$OUT_DIR"

echo "==> Downloading ${TARBALL_URL}"
curl -fLsS "$TARBALL_URL" -o "$TARBALL"
echo "==> Downloading ${ASC_URL}"
curl -fLsS "$ASC_URL" -o "$ASC"

# Use a throwaway GPG home so we don't touch the runner's keyring.
TMPGPG="$(mktemp -d)"
export GNUPGHOME="$TMPGPG"
trap 'rm -rf "$TMPGPG"' EXIT

echo "==> Importing FFmpeg release-signing key from ${KEY_URL}"
curl -fLsS "$KEY_URL" | gpg --batch --import 2>&1 | grep -E 'imported|unchanged|secret' || true

# Pull the fingerprint of every imported key. There should be exactly one.
# `gpg --list-keys --with-colons` prints `fpr:...:FINGERPRINT:` lines.
IMPORTED_FINGERPRINT="$(gpg --list-keys --with-colons | awk -F: '/^fpr:/{print $10; exit}')"
if [[ -z "$IMPORTED_FINGERPRINT" ]]; then
    echo "verify-source: no key fingerprint returned from gpg --list-keys" >&2
    exit 65
fi

# Uppercase both sides (gpg can print mixed case on some platforms).
IMPORTED_UP="$(echo "$IMPORTED_FINGERPRINT" | tr '[:lower:]' '[:upper:]')"
EXPECTED_UP="$(echo "$EXPECTED_FINGERPRINT" | tr '[:lower:]' '[:upper:]')"
if [[ "$IMPORTED_UP" != "$EXPECTED_UP" ]]; then
    echo "verify-source: FFmpeg signing key fingerprint mismatch" >&2
    echo "  expected: $EXPECTED_UP" >&2
    echo "  got:      $IMPORTED_UP" >&2
    exit 65
fi

echo "==> Verifying detached signature ${ASC} against ${TARBALL}"
gpg --batch --verify "$ASC" "$TARBALL" 2>&1 | tee /tmp/verify-source-gpg.log
if ! grep -q "Good signature" /tmp/verify-source-gpg.log; then
    echo "verify-source: gpg --verify did not report a Good signature" >&2
    exit 65
fi

# Cross-platform sha256: macOS has shasum; Linux has sha256sum.
echo "==> Verifying SHA-256 of ${TARBALL}"
if command -v sha256sum >/dev/null; then
    ACTUAL_SHA256="$(sha256sum "$TARBALL" | awk '{print $1}')"
else
    ACTUAL_SHA256="$(shasum -a 256 "$TARBALL" | awk '{print $1}')"
fi
if [[ "$ACTUAL_SHA256" != "$EXPECTED_SHA256" ]]; then
    echo "verify-source: SHA-256 mismatch" >&2
    echo "  expected: $EXPECTED_SHA256" >&2
    echo "  got:      $ACTUAL_SHA256" >&2
    exit 65
fi

rm -f /tmp/verify-source-gpg.log
echo "==> OK: ffmpeg-${FFMPEG_VERSION}.tar.xz verified (GPG + SHA-256)."
echo "tarball:$OUT_DIR/$TARBALL"
echo "version:$FFMPEG_VERSION"
