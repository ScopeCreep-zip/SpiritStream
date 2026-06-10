#!/usr/bin/env bash
# Activate an encrypted profile end-to-end.
#   1. Save the profile under a password
#   2. activate WITHOUT --password → exit 5 (PasswordRequired)
#   3. activate WITH wrong --password → exit 6 (PasswordIncorrect)
#   4. activate with empty stdin secret → exit 64 (usage error)
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
    profile save "$fixture" --password-from stdin >/dev/null <<<'rosebud-twelve-chars'

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
    profile activate encactivate --password-from stdin >/dev/null <<<'wrong'
code=$?
set -e
[[ $code -eq 6 ]] || { echo "wrong-password activate should exit 6, got $code" >&2; exit 1; }

# Empty stdin where a password was promised → usage error (64): the
# secret-input layer refuses an empty secret rather than guessing
# between "missing" and "wrong". (Plaintext --password '' no longer
# exists — secrets never ride argv.)
set +e
"$SPIRITSTREAM_CLI" --data-dir "$SPIRITSTREAM_TEST_DATA_DIR" --quiet \
    profile activate encactivate --password-from stdin >/dev/null </dev/null
code=$?
set -e
[[ $code -eq 64 ]] || { echo "empty stdin secret should exit 64 (usage), got $code" >&2; exit 1; }

# Correct password → exit 0 with the resolved profile + event.
activated=$("$SPIRITSTREAM_CLI" --data-dir "$SPIRITSTREAM_TEST_DATA_DIR" --quiet \
    profile activate encactivate --password-from stdin <<<'rosebud-twelve-chars')
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
