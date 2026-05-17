#!/usr/bin/env bash
# A fresh install has no active streams.
set -euo pipefail

out=$("$SPIRITSTREAM_CLI" --data-dir "$SPIRITSTREAM_TEST_DATA_DIR" --quiet stream status)
echo "$out" | python3 -c '
import json, sys
body = json.load(sys.stdin)
assert body == {"active": [], "count": 0}, body
'
