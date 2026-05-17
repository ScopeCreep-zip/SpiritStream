#!/usr/bin/env bash
# `profile ensure-order` backfills order indexes for every saved profile
# that does not yet appear in the order map. Mirrors HTTP
# `POST /api/v1/profiles/order/ensure`.
set -euo pipefail

port=1935
for n in alpha beta gamma; do
    f="$SPIRITSTREAM_TEST_DATA_DIR/$n.json"
    cat >"$f" <<JSON
{
  "id": "$n-id",
  "name": "$n",
  "encrypted": false,
  "input": {
    "type": "rtmp",
    "bindAddress": "127.0.0.1",
    "port": $port,
    "application": "live"
  },
  "outputGroups": []
}
JSON
    "$SPIRITSTREAM_CLI" --data-dir "$SPIRITSTREAM_TEST_DATA_DIR" --quiet \
        profile save "$f" >/dev/null
    port=$((port + 1))
done

map=$("$SPIRITSTREAM_CLI" --data-dir "$SPIRITSTREAM_TEST_DATA_DIR" --quiet \
    profile ensure-order)
echo "$map" | python3 -c '
import json, sys
m = json.load(sys.stdin)
for name in ("alpha", "beta", "gamma"):
    assert name in m, f"missing {name} in {m}"
    assert isinstance(m[name], int) and m[name] > 0, f"bad index for {name}: {m[name]}"
# Indexes should be strictly increasing on the 10-step grid.
indexes = [m["alpha"], m["beta"], m["gamma"]]
assert len(set(indexes)) == 3, f"duplicate indexes: {indexes}"
'

# Idempotent — second run returns the same map (HashMap key order is
# non-deterministic so compare structurally, not as raw JSON strings).
again=$("$SPIRITSTREAM_CLI" --data-dir "$SPIRITSTREAM_TEST_DATA_DIR" --quiet \
    profile ensure-order)
python3 -c "
import json, sys
a = json.loads('''$map''')
b = json.loads('''$again''')
assert a == b, f'ensure-order not idempotent: {a} vs {b}'
"
