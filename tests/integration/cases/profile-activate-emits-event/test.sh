#!/usr/bin/env bash
# `profile activate` runs the core orchestration (chat settings + OBS config +
# auto-connect) and prints the resolved profile + ProfileActivatedEvent payload
# the HTTP transport would emit on `/api/v1/events`.
set -euo pipefail

fixture="$SPIRITSTREAM_TEST_DATA_DIR/activate-seed.json"
cat >"$fixture" <<'JSON'
{
  "id": "act-001",
  "name": "activateme",
  "encrypted": false,
  "input": {
    "type": "rtmp",
    "bindAddress": "127.0.0.1",
    "port": 1935,
    "application": "live"
  },
  "outputGroups": [],
  "settings": {
    "themeId": "dark",
    "language": "en",
    "showNotifications": true,
    "encryptStreamKeys": false,
    "obs": {
      "host": "127.0.0.1",
      "port": 4455,
      "password": "",
      "useAuth": false,
      "direction": "disabled",
      "autoConnect": false
    }
  }
}
JSON

"$SPIRITSTREAM_CLI" --data-dir "$SPIRITSTREAM_TEST_DATA_DIR" --quiet \
    profile save "$fixture" >/dev/null

activated=$("$SPIRITSTREAM_CLI" --data-dir "$SPIRITSTREAM_TEST_DATA_DIR" --quiet \
    profile activate activateme)
echo "$activated" | python3 -c '
import json, sys
b = json.load(sys.stdin)
assert b["profile"]["name"] == "activateme", b
event = b["event"]
assert event["name"] == "activateme", event
assert event["themeId"] == "dark", event
assert event["language"] == "en", event
assert event["showNotifications"] is True, event
assert event["obs"]["host"] == "127.0.0.1", event
assert event["obs"]["port"] == 4455, event
assert event["obs"]["autoConnect"] is False, event
'
