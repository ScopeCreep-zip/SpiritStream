#!/usr/bin/env bash
# `obs config` returns the in-memory OBS WebSocket configuration. A
# fresh install has the default-shape config. The `set-config` write
# path is in-memory only per CLI invocation; the HTTP layer with a
# long-lived server is where persistence happens — so this test
# verifies the GET shape and the SET command's success response
# without asserting cross-invocation persistence.
set -euo pipefail

out=$("$SPIRITSTREAM_CLI" --data-dir "$SPIRITSTREAM_TEST_DATA_DIR" --quiet obs config)
echo "$out" | python3 -c '
import json, sys
c = json.load(sys.stdin)
# Default-shape sanity: required fields present, correct types.
assert isinstance(c.get("host"), str), c
assert isinstance(c.get("port"), int), c
assert isinstance(c.get("useAuth"), bool), c
assert isinstance(c.get("autoConnect"), bool), c
# Direction is a typed enum on the wire — must be one of the known variants.
assert c.get("direction") in (
    "disabled", "obs-to-spiritstream",
    "spiritstream-to-obs", "bidirectional",
), c
'

# `obs set-config` accepts the documented flags and returns a typed
# acknowledgement.
set_out=$("$SPIRITSTREAM_CLI" --data-dir "$SPIRITSTREAM_TEST_DATA_DIR" --quiet \
  obs set-config \
  --host 192.168.1.42 \
  --port 4455 \
  --direction bidirectional \
  --use-auth \
  --auto-connect)
echo "$set_out" | python3 -c '
import json, sys
r = json.load(sys.stdin)
# CLI `obs set-config` emits `{"saved": true}` on success — the in-memory
# config update is fire-and-forget at the CLI layer. The HTTP equivalent
# returns the updated config; the CLI keeps it terse.
assert r.get("saved") is True, r
'
