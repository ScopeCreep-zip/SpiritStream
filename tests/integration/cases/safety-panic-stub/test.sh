#!/usr/bin/env bash
# `safety panic` runs the full disconnect flow in-process.
# On a fresh install with no streams active, `streams_stopped` is 0 and
# `elapsed_ms` is a number.
set -euo pipefail

out=$("$SPIRITSTREAM_CLI" --data-dir "$SPIRITSTREAM_TEST_DATA_DIR" safety panic)

echo "$out" | python3 -c '
import json, sys
b = json.load(sys.stdin)
assert b["streams_stopped"] == 0, b
assert isinstance(b["elapsed_ms"], int), b
assert b["elapsed_ms"] < 30000, "panic took too long: " + str(b)
'

# The audit log must contain a panic_triggered entry.
audit_log="$SPIRITSTREAM_TEST_DATA_DIR/audit/audit.log"
if [[ ! -f "$audit_log" ]]; then
    echo "audit log missing at $audit_log" >&2
    exit 1
fi
if ! grep -q '"kind":"panic_triggered"' "$audit_log"; then
    echo "audit log did not record panic_triggered" >&2
    cat "$audit_log" >&2
    exit 1
fi
