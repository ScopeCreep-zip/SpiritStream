#!/usr/bin/env bash
# `chat connect` for an unimplemented platform (Kick / Facebook) must
# return a typed error — not panic, not silently succeed. This pins
# the contract for ChatPlatform variants that have no connector yet.
set -euo pipefail

# Kick is not implemented (see ChatManager::create_platform_connector).
# Expect a non-zero exit code and a structured error body.
set +e
out=$("$SPIRITSTREAM_CLI" --data-dir "$SPIRITSTREAM_TEST_DATA_DIR" --quiet \
  chat connect kick --channel testch 2>&1)
rc=$?
set -e

if [ "$rc" -eq 0 ]; then
  echo "expected non-zero exit for unimplemented platform, got 0: $out"
  exit 1
fi

# CLI's own argument-validation exit code is 64 (EX_USAGE). The
# platform-unimplemented case is surfaced via CliError::Argument, so 64
# is the expected code.
if [ "$rc" -ne 64 ]; then
  echo "expected exit code 64 (Argument), got $rc"
  exit 1
fi
