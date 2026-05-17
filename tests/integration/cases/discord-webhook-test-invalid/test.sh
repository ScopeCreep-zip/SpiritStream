#!/usr/bin/env bash
# `discord test-webhook` with an obviously invalid URL must return a
# structured failure (not a panic, not a successful send). Pins the
# fact that webhook URL validation is server-side, not frontend-only.
set -euo pipefail

out=$("$SPIRITSTREAM_CLI" --data-dir "$SPIRITSTREAM_TEST_DATA_DIR" --quiet \
  discord test-webhook "https://not-discord.example/api/webhooks/x/y")
echo "$out" | python3 -c '
import json, sys
r = json.load(sys.stdin)
assert r["success"] is False, r
assert "Invalid" in r["message"] or "invalid" in r["message"], r
'
