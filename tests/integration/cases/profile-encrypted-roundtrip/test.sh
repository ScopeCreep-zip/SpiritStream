#!/usr/bin/env bash
# Encrypted profile round-trip:
#   1. Save under a password
#   2. is-encrypted reports true
#   3. Show without --password fails
#   4. Show with wrong --password fails
#   5. Show with correct --password returns the profile body
set -euo pipefail

fixture="$SPIRITSTREAM_TEST_DATA_DIR/encrypted.json"
cat >"$fixture" <<'JSON'
{
  "id": "lock-001",
  "name": "lockedprofile",
  "encrypted": false,
  "input": {
    "type": "rtmp",
    "bindAddress": "127.0.0.1",
    "port": 1935,
    "application": "live"
  },
  "outputGroups": []
}
JSON

"$SPIRITSTREAM_CLI" --data-dir "$SPIRITSTREAM_TEST_DATA_DIR" --quiet \
    profile save "$fixture" --password 'correct horse battery staple' >/dev/null

probe=$("$SPIRITSTREAM_CLI" --data-dir "$SPIRITSTREAM_TEST_DATA_DIR" --quiet \
    profile is-encrypted lockedprofile)
echo "$probe" | python3 -c '
import json, sys
assert json.load(sys.stdin)["encrypted"] is True, "expected encrypted: true"
'

# No password → fail
set +e
"$SPIRITSTREAM_CLI" --data-dir "$SPIRITSTREAM_TEST_DATA_DIR" --quiet \
    profile show lockedprofile >/dev/null
code=$?
set -e
[[ $code -ne 0 ]] || { echo "show without password should fail" >&2; exit 1; }

# Wrong password → fail
set +e
"$SPIRITSTREAM_CLI" --data-dir "$SPIRITSTREAM_TEST_DATA_DIR" --quiet \
    profile show lockedprofile --password 'wrong' >/dev/null
code=$?
set -e
[[ $code -ne 0 ]] || { echo "show with wrong password should fail" >&2; exit 1; }

# Correct password → succeed; encrypted flag must be true regardless of
# what was in the fixture (load() overwrites profile.encrypted from the
# file extension found, so .mgs always reports encrypted=true).
shown=$("$SPIRITSTREAM_CLI" --data-dir "$SPIRITSTREAM_TEST_DATA_DIR" --quiet \
    profile show lockedprofile --password 'correct horse battery staple')
echo "$shown" | python3 -c '
import json, sys
b = json.load(sys.stdin)
assert b["name"] == "lockedprofile", b
assert b["encrypted"] is True, f"expected encrypted=true on .mgs profile, got {b}"
'

# Empty-string password on an encrypted profile must be treated as
# "no password" (exit 5 = PasswordRequired), NOT "wrong password"
# (exit 6 = PasswordIncorrect).
set +e
"$SPIRITSTREAM_CLI" --data-dir "$SPIRITSTREAM_TEST_DATA_DIR" --quiet \
    profile show lockedprofile --password '' >/dev/null
code=$?
set -e
[[ $code -eq 5 ]] || { echo "empty-string password should yield exit 5 (PasswordRequired), got $code" >&2; exit 1; }
