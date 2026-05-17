# Integration & threat-model tests

These tests run the rewritten system end-to-end against an isolated
`--data-dir <tmpdir>`. Cases are reproducible (every run starts from a fresh
data dir), transport-agnostic (driven through `spiritstream-cli`, never
HTTP), and easy to bisect.

| Directory | Purpose |
|---|---|
| `cases/` | One subdirectory per case; each contains an executable `test.sh` that exits 0 on success, non-zero on failure. The harness reports per-case pass/fail. |
| `threat-model/` | Threat-model fixtures: chat messages containing PII, images with GPS EXIF, log payloads with API tokens. Tests assert defenses fire correctly. |

## Test framework

The rewrite plan called for "golden files (insta or shell-based `diff`)".
The shell-based suite under `cases/` uses **assertion-driven shell scripts**
rather than literal file diffs, because:

- Several command outputs contain machine-local values (tempdir paths,
  timestamps, hardware encoder names) that change every run. A literal diff
  would be brittle and require constant golden-file updates.
- Python's `json` module is universally available and produces clearer
  failure messages than `diff`.
- Each `test.sh` documents the contract it pins in plain English at the top.

The Rust-side mirror in `crates/transport-cli/tests/cli_surface.rs` covers
the same surface with `cargo test` integration; both must stay in sync.
The CLI dispatches in-process against `spiritstream-core` — never HTTP —
so the surface tests under `cases/` directly validate the contract the
`@hey-api/openapi-ts`-generated frontend client and any future
transport adapter must honor.

## Running

```bash
# Build the CLI binary first.
cargo build -p spiritstream-cli

# Run every case.
tests/integration/run.sh

# Run a subset.
tests/integration/run.sh profile-list-empty settings-defaults
```

The CLI is the source of truth for what the API surface can do. If a test
cannot be expressed against the CLI, that is a bug in `spiritstream-core`'s
contract — fix it before adding HTTP-only tests.
