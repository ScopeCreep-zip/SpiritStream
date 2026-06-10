#!/usr/bin/env bash
# `oauth account <provider> --profile <name>` reads stored OAuth user info
# off the named profile's settings. Mirrors HTTP `GET /api/v1/oauth/{provider}/account`.
set -euo pipefail

fixture="$SPIRITSTREAM_TEST_DATA_DIR/with-twitch.json"
cat >"$fixture" <<'JSON'
{
  "id": "tw-001",
  "name": "creator",
  "encrypted": false,
  "input": {
    "type": "rtmp",
    "bindAddress": "127.0.0.1",
    "port": 1935,
    "application": "live"
  },
  "outputGroups": [],
  "settings": {
    "oauth": {
      "twitch": {
        "userId": "12345",
        "username": "creator_handle",
        "displayName": "Creator"
      },
      "youtube": {}
    }
  }
}
JSON

"$SPIRITSTREAM_CLI" --data-dir "$SPIRITSTREAM_TEST_DATA_DIR" --quiet \
    profile save "$fixture" >/dev/null

out=$("$SPIRITSTREAM_CLI" --data-dir "$SPIRITSTREAM_TEST_DATA_DIR" --quiet \
    oauth account twitch --profile creator)
echo "$out" | python3 -c '
import json, sys
b = json.load(sys.stdin)
assert b["loggedIn"] is True, b
assert b["userId"] == "12345", b
assert b["username"] == "creator_handle", b
assert b["displayName"] == "Creator", b
'

# YouTube has no stored account → loggedIn must be false.
yt=$("$SPIRITSTREAM_CLI" --data-dir "$SPIRITSTREAM_TEST_DATA_DIR" --quiet \
    oauth account youtube --profile creator)
echo "$yt" | python3 -c '
import json, sys
b = json.load(sys.stdin)
assert b["loggedIn"] is False, b
assert b["userId"] == "", b
'

# kick / facebook are queryable too now (all four providers store
# accounts on the profile); an account that was never connected reads
# logged-out rather than erroring.
kk=$("$SPIRITSTREAM_CLI" --data-dir "$SPIRITSTREAM_TEST_DATA_DIR" --quiet \
    oauth account kick --profile creator)
echo "$kk" | python3 -c '
import json, sys
b = json.load(sys.stdin)
assert b["loggedIn"] is False, b
'

# A genuinely unknown provider must error out.
set +e
err=$("$SPIRITSTREAM_CLI" --data-dir "$SPIRITSTREAM_TEST_DATA_DIR" --quiet \
    oauth account myspace --profile creator 2>&1)
code=$?
set -e
[[ $code -ne 0 ]] || { echo "unknown provider should fail" >&2; exit 1; }
