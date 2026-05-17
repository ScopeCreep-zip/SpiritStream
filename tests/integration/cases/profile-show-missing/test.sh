#!/usr/bin/env bash
# `profile show` on a missing profile exits 4 (ProfileNotFound) with
# kind=profile_not_found.
set -euo pipefail

set +e
out=$("$SPIRITSTREAM_CLI" --data-dir "$SPIRITSTREAM_TEST_DATA_DIR" --quiet profile show missing)
code=$?
set -e

if [[ $code -ne 4 ]]; then
    echo "expected exit 4, got $code (stdout: $out)" >&2
    exit 1
fi

echo "$out" | python3 -c '
import json, sys
b = json.load(sys.stdin)
assert b["ok"] is False, b
assert b["kind"] == "profile_not_found", b
'
