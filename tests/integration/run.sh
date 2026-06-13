#!/usr/bin/env bash
# Reproducible integration tests driven by spiritstream-cli.
#
# Every case under cases/<name>/ has:
#   - test.sh             — runs the CLI against a fresh tempdir and asserts.
# Cases are expected to be silent on success and noisy on failure. This
# harness runs them all, reports pass/fail per case, and exits non-zero if
# any fail.
#
# Usage:
#   tests/integration/run.sh          # run every case
#   tests/integration/run.sh foo bar  # run only `foo` and `bar`
#
# Prerequisites: spiritstream-cli must be built. Run
#   cargo build -p spiritstream-cli
# first if it isn't.

set -u

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE="$(cd "$HERE/../.." && pwd)"
CLI="$WORKSPACE/target/debug/spiritstream-cli"

if [[ ! -x "$CLI" ]]; then
    echo "error: $CLI is missing. Run 'cargo build -p spiritstream-cli' first." >&2
    exit 2
fi

export SPIRITSTREAM_CLI="$CLI"
export SPIRITSTREAM_THEMES_DIR="$WORKSPACE/themes"
# Hermetic secrets: pin the encrypted-file store so cases that persist
# credentials (oauth config) write inside their throwaway data dir, not
# the developer's OS keychain (the default platform probe on macOS).
export SPIRITSTREAM_SECRET_STORE=file

cases=()
if [[ $# -gt 0 ]]; then
    for name in "$@"; do
        cases+=("$HERE/cases/$name")
    done
else
    while IFS= read -r dir; do
        cases+=("$dir")
    done < <(find "$HERE/cases" -mindepth 1 -maxdepth 1 -type d | sort)
fi

pass=0
fail=0
fail_names=()

for case_dir in "${cases[@]}"; do
    name="$(basename "$case_dir")"
    test_script="$case_dir/test.sh"
    if [[ ! -x "$test_script" ]]; then
        echo "skip: $name (no executable test.sh)"
        continue
    fi
    tmp="$(mktemp -d)"
    if SPIRITSTREAM_TEST_DATA_DIR="$tmp" "$test_script" >"$tmp/.stdout" 2>"$tmp/.stderr"; then
        echo "ok:   $name"
        pass=$((pass + 1))
    else
        echo "FAIL: $name"
        echo "  stdout:" && sed 's/^/    /' "$tmp/.stdout"
        echo "  stderr:" && sed 's/^/    /' "$tmp/.stderr"
        fail=$((fail + 1))
        fail_names+=("$name")
    fi
    rm -rf "$tmp"
done

echo
echo "Summary: $pass passed, $fail failed"
if [[ $fail -gt 0 ]]; then
    printf '  failures: %s\n' "${fail_names[@]}"
    exit 1
fi
