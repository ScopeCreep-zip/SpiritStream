#!/usr/bin/env bash
# A fresh install has no chat connections.
set -euo pipefail

out=$("$SPIRITSTREAM_CLI" --data-dir "$SPIRITSTREAM_TEST_DATA_DIR" --quiet chat status)
echo "$out" | python3 -c '
import json, sys
body = json.load(sys.stdin)
assert body == [], body
'
