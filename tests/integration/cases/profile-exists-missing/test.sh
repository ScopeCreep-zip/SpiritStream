#!/usr/bin/env bash
# `profile exists <name>` reports false for an unknown name.
set -euo pipefail

out=$("$SPIRITSTREAM_CLI" --data-dir "$SPIRITSTREAM_TEST_DATA_DIR" --quiet profile exists nonesuch)
echo "$out" | python3 -c '
import json, sys
body = json.load(sys.stdin)
assert body == {"name": "nonesuch", "exists": False}, body
'
