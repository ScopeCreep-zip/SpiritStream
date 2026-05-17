#!/usr/bin/env bash
# `discord send --profile <name>` replaces the old punt-to-HTTP stub.
# When the profile's webhook is disabled the service returns a structured
# refusal rather than attempting a network call — that is what we test
# here (so this case stays deterministic offline).
set -euo pipefail

fixture="$SPIRITSTREAM_TEST_DATA_DIR/with-discord.json"
cat >"$fixture" <<'JSON'
{
  "id": "ds-001",
  "name": "withdiscord",
  "encrypted": false,
  "input": {
    "type": "rtmp",
    "bindAddress": "127.0.0.1",
    "port": 1935,
    "application": "live"
  },
  "outputGroups": [],
  "settings": {
    "discord": {
      "webhookEnabled": false,
      "webhookUrl": "",
      "goLiveMessage": "going live",
      "imagePath": "",
      "cooldownEnabled": false,
      "cooldownSeconds": 0
    }
  }
}
JSON

"$SPIRITSTREAM_CLI" --data-dir "$SPIRITSTREAM_TEST_DATA_DIR" --quiet \
    profile save "$fixture" >/dev/null

out=$("$SPIRITSTREAM_CLI" --data-dir "$SPIRITSTREAM_TEST_DATA_DIR" --quiet \
    discord send --profile withdiscord)
echo "$out" | python3 -c '
import json, sys
b = json.load(sys.stdin)
assert b["success"] is False, b
assert "not enabled" in b["message"].lower(), b
'

# Missing profile must produce a structured error (NOT a panic).
set +e
"$SPIRITSTREAM_CLI" --data-dir "$SPIRITSTREAM_TEST_DATA_DIR" --quiet \
    discord send --profile no-such-profile >/dev/null 2>&1
code=$?
set -e
[[ $code -ne 0 ]] || { echo "missing profile should fail" >&2; exit 1; }
