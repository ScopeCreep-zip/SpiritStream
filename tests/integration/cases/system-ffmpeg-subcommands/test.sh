#!/usr/bin/env bash
# `system ffmpeg` is a subcommand group: check / path / update-check.
# Option A retired `download` / `cancel` / `delete` — FFmpeg ships as a
# Tauri sidecar on macOS / Windows and as a distro dependency on Linux.
set -euo pipefail

check_out=$("$SPIRITSTREAM_CLI" --data-dir "$SPIRITSTREAM_TEST_DATA_DIR" --quiet system ffmpeg check)
echo "$check_out" | python3 -c '
import json, sys
b = json.load(sys.stdin)
assert isinstance(b["available"], bool), b
'

path_out=$("$SPIRITSTREAM_CLI" --data-dir "$SPIRITSTREAM_TEST_DATA_DIR" --quiet system ffmpeg path)
echo "$path_out" | python3 -c '
import json, sys
b = json.load(sys.stdin)
assert "path" in b, b
assert b["path"] is None or isinstance(b["path"], str), b
'

# Retired subcommands must be rejected by clap so old scripts surface a
# clear error rather than silently no-op.
for retired in download cancel delete; do
  if "$SPIRITSTREAM_CLI" --data-dir "$SPIRITSTREAM_TEST_DATA_DIR" --quiet system ffmpeg "$retired" >/dev/null 2>&1; then
    echo "system ffmpeg $retired should have been rejected" >&2
    exit 1
  fi
done
