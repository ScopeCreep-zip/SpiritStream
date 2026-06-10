#!/usr/bin/env bash
# SpiritStream 600 LOC ceiling check.
#
# Counts NON-BLANK, NON-COMMENT lines per source file across the
# workspace. Reads `.loc-allowlist` (TOML-style `path = max` lines)
# for grandfathered files; any file beyond its allowlisted ceiling
# (or absent from the allowlist when over the default 600 cap) fails
# the gate. Sprint T artefact; backs the "human-maintainable" rule
# documented in CLAUDE.md and .claude/rules/coding-standards.md.
#
# Usage:
#   scripts/check-loc.sh                # full repo scan
#   scripts/check-loc.sh --changed-only # only files in `git diff --cached`
#   scripts/check-loc.sh --max=900      # temporary higher cap (for migration)
#
# Exit:
#   0 — every counted file is under its ceiling
#   1 — at least one offender
#   2 — usage / internal error

set -euo pipefail

DEFAULT_MAX=600
CUSTOM_MAX=""
CHANGED_ONLY=0
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/.." && pwd)"
ALLOWLIST_FILE="${ROOT}/.loc-allowlist"

while [[ $# -gt 0 ]]; do
    case "$1" in
        --changed-only) CHANGED_ONLY=1; shift ;;
        --max=*) CUSTOM_MAX="${1#--max=}"; shift ;;
        -h|--help) sed -n '2,18p' "$0"; exit 0 ;;
        *) echo "unknown arg: $1" >&2; exit 2 ;;
    esac
done

cap=${CUSTOM_MAX:-$DEFAULT_MAX}

# Collect candidate files.
candidates=()
if [[ $CHANGED_ONLY -eq 1 ]]; then
    # Read staged file names; tolerate empty stage.
    while IFS= read -r f; do
        [[ -n "$f" ]] && candidates+=("$f")
    done < <(git -C "$ROOT" diff --cached --name-only --diff-filter=ACMR)
else
    # Walk the workspace via git ls-files so we honour .gitignore +
    # only count tracked source.
    while IFS= read -r f; do
        candidates+=("$f")
    done < <(git -C "$ROOT" ls-files \
        'crates/**/*.rs' \
        'apps/**/*.ts' \
        'apps/**/*.tsx' \
        'packages/**/*.ts' \
        'packages/**/*.tsx' \
        'scripts/**/*.sh' \
        'scripts/**/*.ts' \
        'scripts/**/*.mjs' \
        'server/**/*.rs' \
        'setup.sh')
fi

# Exclusions (generated / vendored / data).
should_skip() {
    local path="$1"
    case "$path" in
        */target/*) return 0 ;;
        */node_modules/*) return 0 ;;
        */dist/*) return 0 ;;
        packages/types/src/generated/*) return 0 ;;
        packages/api-client/src/generated/*) return 0 ;;
        */locales/*.json) return 0 ;;
        */data/*.json) return 0 ;;
        # Generated platform table on the web side is also data.
        apps/web/src/types/generated-platforms.ts) return 0 ;;
    esac
    return 1
}

# Count non-blank, non-comment lines.
count_loc() {
    local f="$1"
    case "$f" in
        *.rs|*.ts|*.tsx|*.mjs)
            # Rust + TS: strip blank lines and `// ...` line comments.
            # Multi-line `/* */` blocks aren't perfectly stripped (would
            # need a real lexer) but the common case dominates and a
            # rough overcount can only ever push a borderline file
            # OVER the cap, not UNDER it — false positives are safe.
            awk 'NF && !/^[[:space:]]*\/\// && !/^[[:space:]]*\*/ && !/^[[:space:]]*\/\*/' "$f" | wc -l
            ;;
        *.sh)
            # Shell: strip blank + `#` comment lines.
            awk 'NF && !/^[[:space:]]*#/' "$f" | wc -l
            ;;
        *)
            wc -l <"$f"
            ;;
    esac
}

# Allowlist lookup (portable: macOS bash 3.2 lacks associative arrays).
# Format per line:
#   path = max
# `#` introduces a comment. Blank lines are ignored.
allowlist_ceiling() {
    local needle="$1"
    [[ -f "$ALLOWLIST_FILE" ]] || return 1
    awk -v needle="$needle" '
        # Strip everything from a `#` onward.
        { sub(/#.*$/, "", $0) }
        # Skip blanks.
        /^[[:space:]]*$/ { next }
        # Split on `=`.
        {
            idx = index($0, "=")
            if (idx == 0) next
            path = substr($0, 1, idx - 1)
            max = substr($0, idx + 1)
            gsub(/^[[:space:]]+|[[:space:]]+$/, "", path)
            gsub(/^[[:space:]]+|[[:space:]]+$/, "", max)
            if (path == needle) {
                print max
                exit 0
            }
        }
    ' "$ALLOWLIST_FILE"
}

violations=0
# Bash 3.2 (macOS default) trips `set -u` on `${array[@]}` when empty.
# Common in `--changed-only` runs with nothing staged.
if [[ ${#candidates[@]} -eq 0 ]]; then
    exit 0
fi
for f in "${candidates[@]}"; do
    [[ -z "$f" ]] && continue
    [[ -f "$ROOT/$f" ]] || continue
    if should_skip "$f"; then continue; fi
    n=$(count_loc "$ROOT/$f" | tr -d ' ')
    ceiling="$cap"
    # `|| true` so the absent-allowlist case (`return 1`) doesn't trip errexit.
    allowed=$(allowlist_ceiling "$f" || true)
    if [[ -n "$allowed" ]]; then
        ceiling="$allowed"
    fi
    if (( n > ceiling )); then
        over=$(( n - ceiling ))
        echo "OVER LIMIT: $f:$n (ceiling $ceiling, +$over)"
        violations=$(( violations + 1 ))
    fi
done

if (( violations > 0 )); then
    echo "" >&2
    echo "$violations file(s) exceed the LOC ceiling." >&2
    echo "Either split the offender (see .claude/rules/coding-standards.md#file-size-limit) " >&2
    echo "or, if you're shrinking an allowlisted file, drop the .loc-allowlist entry." >&2
    exit 1
fi

exit 0
