#!/usr/bin/env bash
# `system ready` is a binary readiness probe. With a fresh data dir
# every subsystem reports Ok (registry built synchronously, themes
# loaded from SPIRITSTREAM_THEMES_DIR), so the command exits 0 and
# reports ready=true.
set -euo pipefail

out=$("$SPIRITSTREAM_CLI" --data-dir "$SPIRITSTREAM_TEST_DATA_DIR" --quiet \
    system ready)
echo "$out" | python3 -c '
import json, sys
b = json.load(sys.stdin)
assert b["ready"] is True, b
assert b["failed"] == [], b
'
