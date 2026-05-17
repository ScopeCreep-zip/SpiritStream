#!/usr/bin/env bash
# `oauth config get` returns the OAuth client config as JSON. On a fresh
# install (no operator overrides), every field is None / omitted —
# `skip_serializing_if = "Option::is_none"` keeps the wire shape compact.
# The `set` path is in-memory only (one CLI invocation = fresh process),
# so we don't test roundtrip — that's an HTTP-mode property where the
# server stays running.
set -euo pipefail

out=$("$SPIRITSTREAM_CLI" --data-dir "$SPIRITSTREAM_TEST_DATA_DIR" --quiet \
  oauth config get)
echo "$out" | python3 -c '
import json, sys
c = json.load(sys.stdin)
# Wire shape: omitted-on-None fields. A fresh install should serialize as
# an empty object (or only contain Some(...) fields if operator-overridden,
# which fresh installs do not have).
assert isinstance(c, dict), c
# Spot-check that no unexpected keys leak through.
allowed = {"twitchClientId", "twitchClientSecret", "youtubeClientId", "youtubeClientSecret"}
unexpected = set(c.keys()) - allowed
assert not unexpected, f"unexpected oauth config keys: {unexpected}"
'

# `oauth config set` returns success even though it does not persist
# across CLI invocations (in-memory state in this CLI process only).
# That is the documented behaviour; the HTTP layer with a long-lived
# server is where persistence happens.
set_out=$("$SPIRITSTREAM_CLI" --data-dir "$SPIRITSTREAM_TEST_DATA_DIR" --quiet \
  oauth config set --twitch-client-id 'tw-test-id')
echo "$set_out" | python3 -c '
import json, sys
r = json.load(sys.stdin)
assert r.get("updated") is True, r
'
