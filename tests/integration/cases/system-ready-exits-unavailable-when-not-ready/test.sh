#!/usr/bin/env bash
# `system ready` exits 69 (EX_UNAVAILABLE) when a subsystem is degraded,
# not 64 (EX_USAGE). Distinguishes "service starting up — retry me" from
# "you called me with bad flags." Retry-loop scripts depend on this:
#   until spiritstream-cli system ready; do sleep 5; done
#
# Degradation lever: tamper the HMAC-chained audit log. `verify_chain()`
# recomputes the chain on every readiness probe; a mutated entry yields
# `AuditChainStatus::Tampered`, which flips `ready=false`. (The themes
# subsystem can't be used here — it falls back to the embedded theme
# catalog when its directory is empty, so it never reports degraded.)
set -u

audit_log="$SPIRITSTREAM_TEST_DATA_DIR/audit/audit.log"

# 1. Seed one valid audit entry so there is a chain to tamper. The HMAC
#    key derives from the machine key persisted in the data dir, so the
#    same key is re-derived on the later `ready` probe — the recompute
#    will detect the mutation below.
"$SPIRITSTREAM_CLI" --data-dir "$SPIRITSTREAM_TEST_DATA_DIR" --quiet \
    system audit-update-failure --detail "integration-test-seed" >/dev/null
if [[ ! -f "$audit_log" ]]; then
    echo "expected audit log at $audit_log after seeding" >&2
    exit 1
fi

# 2. Mutate the entry's detail in place. This keeps the line valid JSON
#    (so the chain parses) but invalidates the stored HMAC → Tampered.
python3 - "$audit_log" <<'PY'
import json, sys
path = sys.argv[1]
with open(path) as f:
    entry = json.loads(f.readline())
entry["action"]["detail"] = "tampered-after-the-fact"
with open(path, "w") as f:
    f.write(json.dumps(entry) + "\n")
PY

# 3. Readiness probe must now report not-ready and exit EX_UNAVAILABLE.
out=$("$SPIRITSTREAM_CLI" --data-dir "$SPIRITSTREAM_TEST_DATA_DIR" --quiet \
    system ready 2>/dev/null)
rc=$?

if [[ "$rc" -ne 69 ]]; then
    echo "expected exit 69 (EX_UNAVAILABLE), got $rc" >&2
    exit 1
fi

# Only the first line is the readiness payload; the error path then
# emits the standard `{kind, message, ok:false}` envelope, so parse the
# first line specifically rather than treating the whole stream as one
# JSON value.
echo "$out" | python3 -c '
import json, sys
b = json.loads(sys.stdin.readline())
assert b["ready"] is False, b
assert isinstance(b["failed"], list) and "audit_log" in b["failed"], b
'
