#!/usr/bin/env bash
# `oauth config get` returns client-id overrides + the derived
# per-provider `configured` truth flags (placeholder credentials report
# false — the dead-link fix). Secret VALUES never print. The `set` path
# is in-memory only (one CLI invocation = fresh process), so we don't
# test roundtrip — that's an HTTP-mode property where the server stays
# running.
set -euo pipefail

# A developer shell may have real credentials exported — drop them so
# the fresh-install truth-flags assertion is hermetic.
for v in TWITCH YOUTUBE KICK FACEBOOK TROVO; do
  unset "SPIRITSTREAM_${v}_CLIENT_ID" "SPIRITSTREAM_${v}_CLIENT_SECRET" || true
done

out=$("$SPIRITSTREAM_CLI" --data-dir "$SPIRITSTREAM_TEST_DATA_DIR" --quiet \
  oauth config get)
echo "$out" | python3 -c '
import json, sys
c = json.load(sys.stdin)
assert set(c.keys()) == {"overrides", "configured"}, c
# No secret values anywhere in the output.
assert not any("Secret" in k for k in c["overrides"]), c
flags = c["configured"]
assert set(flags.keys()) == {"twitch", "youtube", "kick", "facebook", "trovo"}, flags
# Fresh install (no env injection in the test harness): every provider
# honestly reports unconfigured.
assert all(v is False for v in flags.values()), flags
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
