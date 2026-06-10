#!/usr/bin/env bash
# `profile unlock` validates a password against an encrypted profile blob.
# Mirrors the HTTP `/profiles/{name}/unlock` route. Wrong password produces
# exit code 6 (PasswordIncorrect). `profile lock` and `profile locked-list`
# echo the expected single-shot CLI shape.
set -euo pipefail

fixture="$SPIRITSTREAM_TEST_DATA_DIR/unlock-seed.json"
cat >"$fixture" <<'JSON'
{
  "id": "unl-001",
  "name": "unlockme",
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
    profile save "$fixture" --password-from stdin >/dev/null <<<'right-twelve-chars'

# Correct password → unlocked: true.
ok=$("$SPIRITSTREAM_CLI" --data-dir "$SPIRITSTREAM_TEST_DATA_DIR" --quiet \
    profile unlock unlockme --password-from stdin <<<'right-twelve-chars')
echo "$ok" | python3 -c '
import json, sys
b = json.load(sys.stdin)
assert b["unlocked"] is True, b
assert b["name"] == "unlockme", b
'

# Wrong password → exit code 6.
set +e
"$SPIRITSTREAM_CLI" --data-dir "$SPIRITSTREAM_TEST_DATA_DIR" --quiet \
    profile unlock unlockme --password-from stdin >/dev/null <<<'wrong'
code=$?
set -e
[[ $code -eq 6 ]] || { echo "expected exit 6 (PasswordIncorrect), got $code" >&2; exit 1; }

# Lock echoes the single-shot shape.
locked=$("$SPIRITSTREAM_CLI" --data-dir "$SPIRITSTREAM_TEST_DATA_DIR" --quiet \
    profile lock unlockme)
echo "$locked" | python3 -c '
import json, sys
b = json.load(sys.stdin)
assert b["locked"] is True, b
'

# Locked-list is always empty in CLI (one-shot session).
listed=$("$SPIRITSTREAM_CLI" --data-dir "$SPIRITSTREAM_TEST_DATA_DIR" --quiet \
    profile locked-list)
echo "$listed" | python3 -c '
import json, sys
b = json.load(sys.stdin)
assert b == {"unlocked": []}, b
'
