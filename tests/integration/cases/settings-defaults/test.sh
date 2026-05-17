#!/usr/bin/env bash
# Default settings on a fresh install: 30-day retention, no last profile.
# `autoDownloadFfmpeg` was retired with Option A — FFmpeg is bundled at
# build time on macOS/Windows and installed via distro dep on Linux.
set -euo pipefail

out=$("$SPIRITSTREAM_CLI" --data-dir "$SPIRITSTREAM_TEST_DATA_DIR" --quiet settings get)
echo "$out" | python3 -c '
import json, sys
s = json.load(sys.stdin)
assert "autoDownloadFfmpeg" not in s, f"autoDownloadFfmpeg should be retired: {s}"
assert s["logRetentionDays"] == 30, s
assert s["lastProfile"] in (None, ""), s
'
