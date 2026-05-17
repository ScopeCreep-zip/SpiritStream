#!/usr/bin/env bash
# `system logs-export --out <path>` writes the current log lines to a
# file inside the data directory. Mirrors `POST /api/v1/system/logs/export`.
set -euo pipefail

# Seed a log line by running any CLI command that logs.
"$SPIRITSTREAM_CLI" --data-dir "$SPIRITSTREAM_TEST_DATA_DIR" --quiet \
    system app-version >/dev/null

out_path="$SPIRITSTREAM_TEST_DATA_DIR/exported.log"
out=$("$SPIRITSTREAM_CLI" --data-dir "$SPIRITSTREAM_TEST_DATA_DIR" --quiet \
    system logs-export --out "$out_path")
echo "$out" | python3 -c '
import json, sys
b = json.load(sys.stdin)
assert b["exported"] is True, b
assert "lines" in b, b
'

[[ -f "$out_path" ]] || { echo "expected exported file at $out_path" >&2; exit 1; }

# Path-traversal must be refused.
set +e
"$SPIRITSTREAM_CLI" --data-dir "$SPIRITSTREAM_TEST_DATA_DIR" --quiet \
    system logs-export --out /tmp/spiritstream-escaped.log >/dev/null 2>&1
code=$?
set -e
[[ $code -ne 0 ]] || { echo "out-of-tree path should be rejected" >&2; exit 1; }
