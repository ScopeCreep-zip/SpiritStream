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
    profile save "$fixture" --password-from stdin >/dev/null \
    <<<'correct horse battery staple'

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
    profile show lockedprofile --password-from stdin >/dev/null \
    <<<'wrong'
code=$?
set -e
[[ $code -ne 0 ]] || { echo "show with wrong password should fail" >&2; exit 1; }

# Correct password → succeed; encrypted flag must be true regardless of
# what was in the fixture (load() overwrites profile.encrypted from the
# file extension found, so .mgs always reports encrypted=true).
shown=$("$SPIRITSTREAM_CLI" --data-dir "$SPIRITSTREAM_TEST_DATA_DIR" --quiet \
    profile show lockedprofile --password-from stdin \
    <<<'correct horse battery staple')
echo "$shown" | python3 -c '
import json, sys
b = json.load(sys.stdin)
assert b["name"] == "lockedprofile", b
assert b["encrypted"] is True, f"expected encrypted=true on .mgs profile, got {b}"
'

# Empty stdin where a password was promised → usage error (64): the
# secret-input layer refuses an empty secret outright (plaintext
# --password '' no longer exists — secrets never ride argv).
set +e
"$SPIRITSTREAM_CLI" --data-dir "$SPIRITSTREAM_TEST_DATA_DIR" --quiet \
    profile show lockedprofile --password-from stdin >/dev/null \
    </dev/null
code=$?
set -e
[[ $code -eq 64 ]] || { echo "empty stdin secret should exit 64 (usage), got $code" >&2; exit 1; }
