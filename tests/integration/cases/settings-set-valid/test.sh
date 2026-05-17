#!/usr/bin/env bash
# `settings set --set logRetentionDays=90` persists a valid bound and is
# observable by a subsequent `settings get`.
set -euo pipefail

"$SPIRITSTREAM_CLI" --data-dir "$SPIRITSTREAM_TEST_DATA_DIR" --quiet \
    settings set --set logRetentionDays=90 >/dev/null

val=$("$SPIRITSTREAM_CLI" --data-dir "$SPIRITSTREAM_TEST_DATA_DIR" --quiet \
    settings get logRetentionDays)
if [[ "$val" != "90" ]]; then
    echo "expected logRetentionDays=90 after set, got: $val" >&2
    exit 1
fi
