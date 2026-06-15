#!/usr/bin/env bash
# `profile set` edits profile-owned settings by dotted key path (load → mutate →
# save). Unlike the old ephemeral `obs set-config`, the edit PERSISTS to the
# profile document — so we assert it survives across CLI invocations via
# `profile show`. Also proves the editor is uniform (a non-OBS `chat.*` key
# works the same way) and that an invalid typed enum value is rejected.
set -euo pipefail

fixture="$SPIRITSTREAM_TEST_DATA_DIR/seed.json"
cat >"$fixture" <<'JSON'
{
  "id": "obs-001",
  "name": "obsprofile",
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

"$SPIRITSTREAM_CLI" --data-dir "$SPIRITSTREAM_TEST_DATA_DIR" --quiet profile save "$fixture" >/dev/null

# Edit OBS settings + one non-OBS (chat) setting by dotted key path. The command
# echoes the updated `settings` object.
set_out=$("$SPIRITSTREAM_CLI" --data-dir "$SPIRITSTREAM_TEST_DATA_DIR" --quiet \
  profile set obsprofile \
  --set obs.host=192.168.1.42 \
  --set obs.port=4455 \
  --set obs.direction=bidirectional \
  --set obs.autoConnect=true \
  --set chat.twitchChannel=mychannel)
echo "$set_out" | python3 -c '
import json, sys
s = json.load(sys.stdin)
o = s["obs"]
assert o["host"] == "192.168.1.42", o
assert o["port"] == 4455, o
assert o["direction"] == "bidirectional", o
assert o["autoConnect"] is True, o
assert s["chat"]["twitchChannel"] == "mychannel", s["chat"]
'

# Persistence across invocations: re-read the saved document with `profile show`.
shown=$("$SPIRITSTREAM_CLI" --data-dir "$SPIRITSTREAM_TEST_DATA_DIR" --quiet profile show obsprofile)
echo "$shown" | python3 -c '
import json, sys
s = json.load(sys.stdin)["settings"]
o = s["obs"]
assert o["host"] == "192.168.1.42", o
assert o["port"] == 4455, o
assert o["direction"] == "bidirectional", o
assert o["autoConnect"] is True, o
assert s["chat"]["twitchChannel"] == "mychannel", s["chat"]
'

# An invalid typed enum value is rejected (re-typing through ProfileSettings
# fails the serde validation) — non-zero exit, settings unchanged.
if "$SPIRITSTREAM_CLI" --data-dir "$SPIRITSTREAM_TEST_DATA_DIR" --quiet \
     profile set obsprofile --set obs.direction=garbage >/dev/null 2>&1; then
  echo "expected 'profile set obs.direction=garbage' to fail" >&2
  exit 1
fi

# An unknown key is rejected loudly (typo guard).
if "$SPIRITSTREAM_CLI" --data-dir "$SPIRITSTREAM_TEST_DATA_DIR" --quiet \
     profile set obsprofile --set obs.hostt=oops >/dev/null 2>&1; then
  echo "expected 'profile set obs.hostt=oops' to fail" >&2
  exit 1
fi
