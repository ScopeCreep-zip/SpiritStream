#!/usr/bin/env bash
# Round-trip: save a minimal profile, see it in the list, delete it, gone.
set -euo pipefail

fixture="$SPIRITSTREAM_TEST_DATA_DIR/seed.json"
cat >"$fixture" <<'JSON'
{
  "id": "seed-001",
  "name": "seedprofile",
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

# Save.
saved=$("$SPIRITSTREAM_CLI" --data-dir "$SPIRITSTREAM_TEST_DATA_DIR" --quiet profile save "$fixture")
echo "$saved" | python3 -c '
import json, sys
b = json.load(sys.stdin)
assert b["saved"] is True, b
assert b["name"] == "seedprofile", b
'

# List shows it.
listed=$("$SPIRITSTREAM_CLI" --data-dir "$SPIRITSTREAM_TEST_DATA_DIR" --quiet profile list)
echo "$listed" | python3 -c '
import json, sys
b = json.load(sys.stdin)
assert b == {"names": ["seedprofile"]}, b
'

# Show returns the profile body.
shown=$("$SPIRITSTREAM_CLI" --data-dir "$SPIRITSTREAM_TEST_DATA_DIR" --quiet profile show seedprofile)
echo "$shown" | python3 -c '
import json, sys
b = json.load(sys.stdin)
assert b["name"] == "seedprofile", b
assert b["input"]["port"] == 1935, b
'

# Delete.
deleted=$("$SPIRITSTREAM_CLI" --data-dir "$SPIRITSTREAM_TEST_DATA_DIR" --quiet profile delete seedprofile)
echo "$deleted" | python3 -c '
import json, sys
b = json.load(sys.stdin)
assert b["deleted"] is True, b
'

# List is empty again.
after=$("$SPIRITSTREAM_CLI" --data-dir "$SPIRITSTREAM_TEST_DATA_DIR" --quiet profile list)
echo "$after" | python3 -c '
import json, sys
assert json.load(sys.stdin) == {"names": []}, "expected empty after delete"
'
