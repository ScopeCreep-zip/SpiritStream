#!/usr/bin/env bash
# `data export <path>` produces a real non-empty zip at the requested path.
set -euo pipefail

target="$SPIRITSTREAM_TEST_DATA_DIR/export.zip"
out=$("$SPIRITSTREAM_CLI" --data-dir "$SPIRITSTREAM_TEST_DATA_DIR" --quiet data export "$target")
echo "$out" | python3 -c '
import json, sys
assert json.load(sys.stdin)["exported"] is True
'

[[ -s "$target" ]] || { echo "expected non-empty zip at $target" >&2; exit 1; }
