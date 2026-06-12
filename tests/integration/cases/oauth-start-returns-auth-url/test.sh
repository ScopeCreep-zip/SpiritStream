#!/usr/bin/env bash
# `oauth start <provider>`:
# - unconfigured (placeholder credentials) → typed refusal, exit 78
#   (EX_CONFIG). Pre-fix this happily built an authorize URL containing
#   the literal placeholder and sent users to a provider 400 page.
# - configured (env-injected credentials)  → auth URL + callback port +
#   state from the loopback flow.
set -euo pipefail

# Hermetic: a developer shell may have real Kick credentials exported.
unset SPIRITSTREAM_KICK_CLIENT_ID SPIRITSTREAM_KICK_CLIENT_SECRET || true

set +e
err=$("$SPIRITSTREAM_CLI" --data-dir "$SPIRITSTREAM_TEST_DATA_DIR" --quiet oauth start kick 2>&1)
code=$?
set -e
if [ "$code" -ne 78 ]; then
  echo "expected exit 78 for unconfigured provider, got $code: $err" >&2
  exit 1
fi

out=$(SPIRITSTREAM_KICK_CLIENT_ID='itest-kick-id' SPIRITSTREAM_KICK_CLIENT_SECRET='itest-kick-secret'   "$SPIRITSTREAM_CLI" --data-dir "$SPIRITSTREAM_TEST_DATA_DIR" --quiet oauth start kick)
echo "$out" | python3 -c '
import json, sys
body = json.load(sys.stdin)
assert body["auth_url"].startswith("https://id.kick.com/"), body
assert "itest-kick-id" in body["auth_url"], body
assert isinstance(body["callback_port"], int) and body["callback_port"] > 0, body
assert isinstance(body["state"], str) and len(body["state"]) > 0, body
'
