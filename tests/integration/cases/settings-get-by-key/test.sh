#!/usr/bin/env bash
# `settings get <key>` returns just the value; unknown keys exit 64.
set -euo pipefail

val=$("$SPIRITSTREAM_CLI" --data-dir "$SPIRITSTREAM_TEST_DATA_DIR" --quiet settings get logRetentionDays)
if [[ "$val" != "30" ]]; then
    echo "expected default logRetentionDays=30, got: $val" >&2
    exit 1
fi

set +e
"$SPIRITSTREAM_CLI" --data-dir "$SPIRITSTREAM_TEST_DATA_DIR" --quiet settings get madeUpKey >/dev/null
code=$?
set -e
if [[ $code -ne 64 ]]; then
    echo "expected EX_USAGE 64 for unknown key, got $code" >&2
    exit 1
fi
