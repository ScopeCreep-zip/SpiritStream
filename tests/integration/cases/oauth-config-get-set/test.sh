#!/usr/bin/env bash
# `oauth config get` returns per-provider setup summaries (configured
# truth flag, needs_secret, override client id, registration URL —
# placeholder credentials report unconfigured; the dead-link fix).
# Secret VALUES never print. `set-provider` persists through the secret
# store, so a SECOND CLI invocation (fresh process = app restart) must
# still see the credentials — the in-app setup UX contract.
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
providers = {p["provider"]: p for p in c["providers"]}
assert set(providers) == {"twitch", "youtube", "kick", "facebook", "trovo"}, providers
# Fresh install (no env injection in the test harness): every provider
# honestly reports unconfigured, and no secret value appears anywhere.
for name, p in providers.items():
    assert p["configured"] is False, p
    assert p["registrationUrl"].startswith("https://"), p
    assert "clientSecret" not in p and "client_secret" not in p, p
# The setup form needs to know which providers take a secret; Twitch is
# the public-client (Device Code Flow) exception.
assert providers["twitch"]["needsSecret"] is False, providers["twitch"]
assert providers["kick"]["needsSecret"] is True, providers["kick"]
'

# Store one provider through the in-app/CLI setup path. The secret rides
# stdin, never argv.
set_out=$(printf 'kick-test-secret' | "$SPIRITSTREAM_CLI" \
  --data-dir "$SPIRITSTREAM_TEST_DATA_DIR" --quiet \
  oauth config set-provider kick --client-id 'kick-test-id' --client-secret-from stdin)
echo "$set_out" | python3 -c '
import json, sys
c = json.load(sys.stdin)
kick = next(p for p in c["providers"] if p["provider"] == "kick")
assert kick["configured"] is True, kick
assert kick["overrideClientId"] == "kick-test-id", kick
'
# The secret value must not echo back on any surface.
if echo "$set_out" | grep -q 'kick-test-secret'; then
  echo "secret value leaked into set-provider output" >&2
  exit 1
fi

# Fresh CLI process over the same data dir = restart. Credentials must
# still be there (persisted via the secret store, not process memory).
out=$("$SPIRITSTREAM_CLI" --data-dir "$SPIRITSTREAM_TEST_DATA_DIR" --quiet \
  oauth config get)
echo "$out" | python3 -c '
import json, sys
c = json.load(sys.stdin)
kick = next(p for p in c["providers"] if p["provider"] == "kick")
assert kick["configured"] is True, ("credentials must survive restart", kick)
assert kick["overrideClientId"] == "kick-test-id", kick
'
