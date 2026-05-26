#!/usr/bin/env bash
# Fetch /api/v1/openapi.json from a freshly-built spiritstream-server.
# Used by @hey-api/openapi-ts to regenerate the typed API client.
#
# Steps:
#   1. Build spiritstream-server
#   2. Boot it on an ephemeral port against a tempdir
#   3. curl /api/v1/openapi.json into the output path
#   4. Shut down the server
#
# Idempotent. Safe to run anywhere; uses tempdirs and clean shutdown.
#
# Usage:
#   scripts/regenerate-openapi.sh <output-path>
#
# `<output-path>` is the destination JSON file; api-client points this
# at a tempfile and immediately feeds it to openapi-ts.

set -euo pipefail

if [[ $# -lt 1 ]]; then
    echo "usage: $0 <output-path>" >&2
    exit 2
fi
TARGET="$1"

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/.." && pwd)"

mkdir -p "$(dirname "$TARGET")"

echo "[1/3] cargo build -p spiritstream-server"
cargo build --manifest-path "$ROOT/Cargo.toml" -p spiritstream-server >&2

# Find a free port (TCP bind & release).
PORT=$(python3 -c '
import socket
s = socket.socket()
s.bind(("127.0.0.1", 0))
print(s.getsockname()[1])
s.close()
')
TMPDIR=$(mktemp -d)
trap 'rm -rf "$TMPDIR"; [[ -n "${PID:-}" ]] && kill "$PID" 2>/dev/null || true' EXIT

echo "[2/3] booting spiritstream-server on $PORT (data=$TMPDIR)"
SPIRITSTREAM_HOST=127.0.0.1 \
SPIRITSTREAM_PORT=$PORT \
SPIRITSTREAM_DATA_DIR="$TMPDIR" \
SPIRITSTREAM_UI_ENABLED=0 \
"$ROOT/target/debug/spiritstream-server" >/dev/null 2>&1 &
PID=$!

# Poll readiness.
for _ in $(seq 1 50); do
    if curl -sf "http://127.0.0.1:$PORT/api/v1/health" >/dev/null 2>&1; then
        break
    fi
    sleep 0.1
done

echo "[3/3] fetching /api/v1/openapi.json → $TARGET"
curl -sf "http://127.0.0.1:$PORT/api/v1/openapi.json" -o "$TARGET"

echo "done"
