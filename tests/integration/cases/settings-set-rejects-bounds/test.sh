#!/usr/bin/env bash
# Out-of-range settings values surface as `validation_failed` with exit code 7
# (CoreError::ValidationFailed -> EX_DATAERR-ish, see crates/transport-cli/src/error.rs).
set -euo pipefail

set +e
out=$("$SPIRITSTREAM_CLI" --data-dir "$SPIRITSTREAM_TEST_DATA_DIR" --quiet \
    settings set --set logRetentionDays=0 2>&1)
code=$?
set -e

if [[ $code -ne 7 ]]; then
    echo "expected exit 7 for log_retention out-of-range, got $code (output: $out)" >&2
    exit 1
fi

# The error payload must name the violated bound so scripts can branch on it.
if ! echo "$out" | grep -q "log_retention_days_out_of_range"; then
    echo "expected log_retention_days_out_of_range in error output: $out" >&2
    exit 1
fi

# State was not corrupted — load still returns the default.
val=$("$SPIRITSTREAM_CLI" --data-dir "$SPIRITSTREAM_TEST_DATA_DIR" --quiet \
    settings get logRetentionDays)
if [[ "$val" != "30" ]]; then
    echo "rejected save must not mutate state; expected 30, got: $val" >&2
    exit 1
fi
