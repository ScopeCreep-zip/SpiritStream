#!/usr/bin/env bash
# Regenerate packages/validation/src/openapi.json + per-resource JSON Schemas
# from the live spiritstream-server OpenAPI document. Used by:
#   - the frontend's @hey-api/openapi-ts step (generates the typed API client)
#   - packages/validation runtime schema lookups (decorative form-level
#     validation; backend is always the source of truth)
#
# Steps:
#   1. Build spiritstream-server
#   2. Boot it on an ephemeral port against a tempdir
#   3. curl /api/v1/openapi.json
#   4. Split out per-component JSON Schemas
#   5. Shut down the server
#
# Idempotent. Safe to run anywhere; uses tempdirs and clean shutdown.

set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/.." && pwd)"
PACKAGE="$ROOT/packages/validation"
TARGET="$PACKAGE/src/openapi.json"
SCHEMAS_DIR="$PACKAGE/src/schemas"

mkdir -p "$SCHEMAS_DIR"

echo "[1/4] cargo build -p spiritstream-server"
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

echo "[2/4] booting spiritstream-server on $PORT (data=$TMPDIR)"
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

echo "[3/4] fetching /api/v1/openapi.json"
curl -sf "http://127.0.0.1:$PORT/api/v1/openapi.json" -o "$TARGET"

echo "[4/4] splitting per-component schemas into $SCHEMAS_DIR"
python3 - "$TARGET" "$SCHEMAS_DIR" <<'PYEOF'
import json, os, sys
src, out_dir = sys.argv[1], sys.argv[2]
with open(src) as f:
    doc = json.load(f)
components = (doc.get("components") or {}).get("schemas") or {}
for name, schema in components.items():
    schema = dict(schema)
    schema["$schema"] = "https://json-schema.org/draft/2020-12/schema"
    schema["title"] = name
    with open(os.path.join(out_dir, f"{name}.json"), "w") as f:
        json.dump(schema, f, indent=2, sort_keys=True)
        f.write("\n")
print(f"wrote {len(components)} schemas")
PYEOF

echo "done — openapi.json + $(ls "$SCHEMAS_DIR" | wc -l | tr -d ' ') schemas regenerated"
