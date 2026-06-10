#!/usr/bin/env bash
# PII filter must block outbound chat messages atomically across every
# targeted platform. With "alice" in the active profile's PII blocklist
# and Twitch send enabled, `chat send "hi alice"` must fail with
# `chat_blocked_by_pii` for every target — proves the filter lives in
# core (ChatManager::send_message), is reachable from the CLI transport,
# and surfaces a stable error code over the wire.
set -euo pipefail

fixture="$SPIRITSTREAM_TEST_DATA_DIR/profile.json"
cat >"$fixture" <<'JSON'
{
  "id": "pii-test-001",
  "name": "piitest",
  "encrypted": false,
  "input": {
    "type": "rtmp",
    "bindAddress": "127.0.0.1",
    "port": 1935,
    "application": "live"
  },
  "outputGroups": [],
  "piiBlocklist": ["alice"],
  "piiFuzzy": false,
  "settings": {
    "chat": {
      "twitchChannel": "fake",
      "twitchSendEnabled": true
    }
  }
}
JSON

"$SPIRITSTREAM_CLI" --data-dir "$SPIRITSTREAM_TEST_DATA_DIR" --quiet \
  profile save "$fixture" >/dev/null

# Real activation flow — ProfileActivationService now persists
# Settings::last_profile, so the next stateless CLI invocation will
# read this profile as active.
"$SPIRITSTREAM_CLI" --data-dir "$SPIRITSTREAM_TEST_DATA_DIR" --quiet \
  profile activate piitest >/dev/null

# Send a message that matches the blocklist. The CLI emits per-platform
# results as JSON; exit code is 0 because every-platform-fails is a
# valid result shape, not a CLI-level error. The PII gate's contract is
# in the per-platform errorCode field.
out=$("$SPIRITSTREAM_CLI" --data-dir "$SPIRITSTREAM_TEST_DATA_DIR" --quiet \
  chat send "hi alice")

echo "$out" | python3 -c '
import json, sys
results = json.load(sys.stdin)
assert isinstance(results, list) and len(results) >= 1, results
for entry in results:
    assert entry["success"] is False, ("expected failure for every target", entry)
    assert entry["errorCode"] == "chat_blocked_by_pii", (
        "expected chat_blocked_by_pii errorCode", entry,
    )
'

# Plan-cited verification: the PII block must also produce a
# `ChatMessagePiiBlocked` audit entry. Read the chain; assert exactly
# one matching row with our targeted platform list and a non-empty
# phrase_id; assert the matched text is NOT present anywhere in the
# audit file (the chain must store only the HMAC-keyed phrase_id).
audit_out=$("$SPIRITSTREAM_CLI" --data-dir "$SPIRITSTREAM_TEST_DATA_DIR" --quiet \
  audit log --filter chat_message_pii_blocked)

echo "$audit_out" | python3 -c '
import json, sys
entries = json.load(sys.stdin)
assert isinstance(entries, list), entries
assert len(entries) == 1, ("expected exactly one ChatMessagePiiBlocked entry", entries)
action = entries[0]["action"]
assert action["kind"] == "chat_message_pii_blocked", action
assert action["platforms"] == ["twitch"], action
assert isinstance(action["phrase_id"], str) and action["phrase_id"], action
'

# Defense-in-depth: the raw audit file must never contain the matched
# blocklist phrase verbatim. The forensic trail is HMAC-anchored.
if grep -i "alice" "$SPIRITSTREAM_TEST_DATA_DIR/audit/"*.jsonl 2>/dev/null; then
  echo "audit log leaked PII phrase 'alice'" >&2
  exit 1
fi

# Sending a non-matching message after the same activation must succeed
# the PII gate. There is no connected Twitch connector so we expect
# `chat_platform_not_connected` — distinct error code proves the PII
# filter is not over-matching.
out=$("$SPIRITSTREAM_CLI" --data-dir "$SPIRITSTREAM_TEST_DATA_DIR" --quiet \
  chat send "hello world")

echo "$out" | python3 -c '
import json, sys
results = json.load(sys.stdin)
for entry in results:
    assert entry["errorCode"] == "chat_platform_not_connected", (
        "expected platform-not-connected after PII clear", entry,
    )
'

# Sanity: PII-clear send produced no new ChatMessagePiiBlocked entries.
# The chain count after the clean send must still be exactly 1.
audit_out=$("$SPIRITSTREAM_CLI" --data-dir "$SPIRITSTREAM_TEST_DATA_DIR" --quiet \
  audit log --filter chat_message_pii_blocked)

echo "$audit_out" | python3 -c '
import json, sys
entries = json.load(sys.stdin)
assert len(entries) == 1, ("PII-clear send must not produce another block entry", entries)
'
