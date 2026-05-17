#!/usr/bin/env bash
# Activate an encrypted profile end-to-end.
#   1. Save the profile under a password
#   2. activate WITHOUT --password → exit 5 (PasswordRequired)
#   3. activate WITH wrong --password → exit 6 (PasswordIncorrect)
#   4. activate WITH '' --password → exit 5 (empty-string treated as missing)
#   5. activate WITH correct --password → succeeds, payload carries
#      encrypted=true and the ProfileActivatedEvent fields
set -euo pipefail

fixture="$SPIRITSTREAM_TEST_DATA_DIR/encrypted-activate.json"
cat >"$fixture" <<'JSON'
{
  "id": "enc-act-001",
  "name": "encactivate",
  "encrypted": false,
  "input": {
    "type": "rtmp",
    "bindAddress": "127.0.0.1",
    "port": 1935,
    "application": "live"
  },
  "outputGroups": [],
  "settings": {
    "themeId": "light",
    "language": "en",
    "showNotifications": false,
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
    profile save "$fixture" --password 'rosebud-twelve-chars' >/dev/null

# No password → exit 5 (PasswordRequired).
set +e
"$SPIRITSTREAM_CLI" --data-dir "$SPIRITSTREAM_TEST_DATA_DIR" --quiet \
    profile activate encactivate >/dev/null
code=$?
set -e
[[ $code -eq 5 ]] || { echo "no-password activate should exit 5, got $code" >&2; exit 1; }

# Wrong password → exit 6 (PasswordIncorrect).
set +e
"$SPIRITSTREAM_CLI" --data-dir "$SPIRITSTREAM_TEST_DATA_DIR" --quiet \
    profile activate encactivate --password 'wrong' >/dev/null
code=$?
set -e
[[ $code -eq 6 ]] || { echo "wrong-password activate should exit 6, got $code" >&2; exit 1; }

# Empty-string password → exit 5 (must be treated as missing, not wrong).
set +e
"$SPIRITSTREAM_CLI" --data-dir "$SPIRITSTREAM_TEST_DATA_DIR" --quiet \
    profile activate encactivate --password '' >/dev/null
code=$?
set -e
[[ $code -eq 5 ]] || { echo "empty-string password should exit 5 (PasswordRequired), got $code" >&2; exit 1; }

# Correct password → exit 0 with the resolved profile + event.
activated=$("$SPIRITSTREAM_CLI" --data-dir "$SPIRITSTREAM_TEST_DATA_DIR" --quiet \
    profile activate encactivate --password 'rosebud-twelve-chars')
echo "$activated" | python3 -c '
import json, sys
b = json.load(sys.stdin)
assert b["profile"]["name"] == "encactivate", b
assert b["profile"]["encrypted"] is True, f"expected encrypted=true on activated profile, got {b}"
event = b["event"]
assert event["name"] == "encactivate", event
assert event["themeId"] == "light", event
assert event["showNotifications"] is False, event
'
