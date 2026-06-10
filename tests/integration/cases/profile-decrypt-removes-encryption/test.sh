#!/usr/bin/env bash
# `profile decrypt` atomically removes encryption from a profile:
#   1. Save under a password
#   2. is-encrypted reports true
#   3. profile decrypt --password <pw> succeeds and reports decrypted: true
#   4. is-encrypted reports false
#   5. Show without --password succeeds (profile is plaintext now)
set -euo pipefail

fixture="$SPIRITSTREAM_TEST_DATA_DIR/decrypt-seed.json"
cat >"$fixture" <<'JSON'
{
  "id": "dec-001",
  "name": "decryptme",
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
    profile save "$fixture" --password-from stdin >/dev/null <<<'pw1-twelve-chars'

probe=$("$SPIRITSTREAM_CLI" --data-dir "$SPIRITSTREAM_TEST_DATA_DIR" --quiet \
    profile is-encrypted decryptme)
echo "$probe" | python3 -c '
import json, sys
assert json.load(sys.stdin)["encrypted"] is True, "expected encrypted: true"
'

# Atomic encryption removal.
removed=$("$SPIRITSTREAM_CLI" --data-dir "$SPIRITSTREAM_TEST_DATA_DIR" --quiet \
    profile decrypt decryptme --password-from stdin <<<'pw1-twelve-chars')
echo "$removed" | python3 -c '
import json, sys
b = json.load(sys.stdin)
assert b["decrypted"] is True, b
assert b["name"] == "decryptme", b
'

# Profile is now plaintext on disk.
probe2=$("$SPIRITSTREAM_CLI" --data-dir "$SPIRITSTREAM_TEST_DATA_DIR" --quiet \
    profile is-encrypted decryptme)
echo "$probe2" | python3 -c '
import json, sys
assert json.load(sys.stdin)["encrypted"] is False, "expected encrypted: false after decrypt"
'

# Show works without a password.
shown=$("$SPIRITSTREAM_CLI" --data-dir "$SPIRITSTREAM_TEST_DATA_DIR" --quiet \
    profile show decryptme)
echo "$shown" | python3 -c '
import json, sys
b = json.load(sys.stdin)
assert b["name"] == "decryptme", b
'

# Wrong password during decrypt still rejects (exit code 6 — PasswordIncorrect).
"$SPIRITSTREAM_CLI" --data-dir "$SPIRITSTREAM_TEST_DATA_DIR" --quiet \
    profile save "$fixture" --password-from stdin >/dev/null <<<'pw2-twelve-chars'
set +e
"$SPIRITSTREAM_CLI" --data-dir "$SPIRITSTREAM_TEST_DATA_DIR" --quiet \
    profile decrypt decryptme --password-from stdin >/dev/null <<<'wrong'
code=$?
set -e
[[ $code -eq 6 ]] || { echo "expected exit 6 (PasswordIncorrect), got $code" >&2; exit 1; }
