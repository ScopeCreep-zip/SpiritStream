#!/usr/bin/env bash
# `oauth start <provider>` returns the auth URL + callback port +
# state token from the typed OAuth flow.
set -euo pipefail

out=$("$SPIRITSTREAM_CLI" --data-dir "$SPIRITSTREAM_TEST_DATA_DIR" --quiet oauth start twitch)
echo "$out" | python3 -c '
import json, sys
body = json.load(sys.stdin)
assert body["auth_url"].startswith("https://"), body
assert isinstance(body["callback_port"], int) and body["callback_port"] > 0, body
assert isinstance(body["state"], str) and len(body["state"]) > 0, body
'
