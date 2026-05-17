#!/usr/bin/env bash
# The bundled themes (spirit-light, spirit-dark, etc.) must always be present.
set -euo pipefail

out=$("$SPIRITSTREAM_CLI" --data-dir "$SPIRITSTREAM_TEST_DATA_DIR" --quiet theme list)
echo "$out" | python3 -c '
import json, sys
themes = json.load(sys.stdin)
ids = {t["id"] for t in themes}
required = {"spirit-light", "spirit-dark"}
missing = required - ids
assert not missing, f"missing required themes: {missing}; got {ids}"
assert len(themes) >= 4, f"expected at least 4 themes, got {len(themes)}"
'
