#!/usr/bin/env bash
# `system audit-update-failure --detail <msg>` appends an
# AppUpdateSignatureFailed entry to the HMAC-chained audit log. The
# audit log itself is the read surface; `audit log` shows the entry.
# Mirrors HTTP `POST /api/v1/system/audit/app-update-failure`.
set -euo pipefail

ack=$("$SPIRITSTREAM_CLI" --data-dir "$SPIRITSTREAM_TEST_DATA_DIR" --quiet \
    system audit-update-failure --detail 'signature verification failed for v0.0.0-test')
echo "$ack" | python3 -c '
import json, sys
assert json.load(sys.stdin)["recorded"] is True
'

# Read the audit log and verify the entry is present.
log=$("$SPIRITSTREAM_CLI" --data-dir "$SPIRITSTREAM_TEST_DATA_DIR" --quiet \
    audit log)
echo "$log" | python3 -c '
import json, sys
b = json.load(sys.stdin)
entries = b.get("entries", b) if isinstance(b, dict) else b
found = False
for e in entries:
    action = e.get("action", {}) if isinstance(e, dict) else {}
    kind = action.get("kind") if isinstance(action, dict) else None
    if kind == "app_update_signature_failed":
        assert action.get("detail", "").startswith("signature verification failed"), action
        found = True
        break
assert found, f"AppUpdateSignatureFailed entry not found in audit log: {b}"
'
