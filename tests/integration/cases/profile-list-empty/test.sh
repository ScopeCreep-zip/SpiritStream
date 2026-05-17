#!/usr/bin/env bash
# A fresh data dir reports zero profiles.
set -euo pipefail

out=$("$SPIRITSTREAM_CLI" --data-dir "$SPIRITSTREAM_TEST_DATA_DIR" --quiet profile list)
echo "$out" | python3 -c '
import json, sys
body = json.load(sys.stdin)
assert body == {"names": []}, body
'
