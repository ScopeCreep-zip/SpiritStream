#!/usr/bin/env bash
# Plan UX: `stream stop [--group <id>]`. Omitting --group must succeed
# (stops every active group; on a fresh install this is a no-op).
set -euo pipefail

out=$("$SPIRITSTREAM_CLI" --data-dir "$SPIRITSTREAM_TEST_DATA_DIR" --quiet stream stop)
echo "$out" | python3 -c '
import json, sys
b = json.load(sys.stdin)
assert b == {"stopped": []}, b
'
