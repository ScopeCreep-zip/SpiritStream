#!/usr/bin/env bash
# `theme refresh` triggers a re-scan of the themes directory and updates
# the cached list. The list must include the bundled themes after the
# refresh — pins that ThemeManager's reload path actually re-populates.
set -euo pipefail

# Refresh and read.
"$SPIRITSTREAM_CLI" --data-dir "$SPIRITSTREAM_TEST_DATA_DIR" --quiet theme refresh > /dev/null
out=$("$SPIRITSTREAM_CLI" --data-dir "$SPIRITSTREAM_TEST_DATA_DIR" --quiet theme list)
echo "$out" | python3 -c '
import json, sys
themes = json.load(sys.stdin)
assert isinstance(themes, list), themes
ids = {t["id"] for t in themes}
# spirit-dark and spirit-light are bundled embedded themes — they MUST
# be present after refresh on a fresh install.
assert "spirit-dark" in ids, ids
assert "spirit-light" in ids, ids
'
