#!/usr/bin/env bash
# `events watch --for-ms <n>` exits cleanly when the timer fires.
set -euo pipefail

"$SPIRITSTREAM_CLI" --data-dir "$SPIRITSTREAM_TEST_DATA_DIR" --quiet events watch --for-ms 150
