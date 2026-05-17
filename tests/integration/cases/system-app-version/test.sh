#!/usr/bin/env bash
# `system app-version` returns the CARGO_PKG_VERSION of the CLI binary,
# matching the shape of HTTP `GET /api/v1/system/app-version`.
set -euo pipefail

out=$("$SPIRITSTREAM_CLI" --data-dir "$SPIRITSTREAM_TEST_DATA_DIR" --quiet \
    system app-version)
echo "$out" | python3 -c '
import json, re, sys
b = json.load(sys.stdin)
assert "version" in b, b
# semver-ish (major.minor.patch with optional pre/build) — release
# workflow refuses to ship a tag whose components disagree.
assert re.match(r"^\d+\.\d+\.\d+", b["version"]), b
'
