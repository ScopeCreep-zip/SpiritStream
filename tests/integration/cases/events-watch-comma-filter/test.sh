#!/usr/bin/env bash
# `events watch --filter <comma-separated>` must be accepted (plan UX).
set -euo pipefail

"$SPIRITSTREAM_CLI" --data-dir "$SPIRITSTREAM_TEST_DATA_DIR" --quiet \
    events watch --filter stream_stats,chat_message --for-ms 150
