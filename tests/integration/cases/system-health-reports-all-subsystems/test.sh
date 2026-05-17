#!/usr/bin/env bash
# `system health` aggregates subsystem status. Same rollup as HTTP
# `GET /api/v1/health`.
set -euo pipefail

out=$("$SPIRITSTREAM_CLI" --data-dir "$SPIRITSTREAM_TEST_DATA_DIR" --quiet \
    system health)
echo "$out" | python3 -c '
import json, sys
b = json.load(sys.stdin)
assert b["status"] in ("ok", "degraded", "tampered"), b
for name in ("profiles", "settings", "themes", "audit_log"):
    assert name in b["services"], f"missing {name} in {b}"
'

# `--subsystem profiles` narrows the report to one subsystem.
narrowed=$("$SPIRITSTREAM_CLI" --data-dir "$SPIRITSTREAM_TEST_DATA_DIR" --quiet \
    system health --subsystem profiles)
echo "$narrowed" | python3 -c '
import json, sys
b = json.load(sys.stdin)
assert b["subsystem"] == "profiles", b
assert "report" in b, b
'

# Unknown subsystem must error.
set +e
"$SPIRITSTREAM_CLI" --data-dir "$SPIRITSTREAM_TEST_DATA_DIR" --quiet \
    system health --subsystem no-such >/dev/null 2>&1
code=$?
set -e
[[ $code -ne 0 ]] || { echo "unknown subsystem should fail" >&2; exit 1; }
