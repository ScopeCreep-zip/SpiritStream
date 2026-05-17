# Integration & threat-model tests

These tests run the rewritten system end-to-end. They are driven by `spiritstream-cli` so they're reproducible, transport-agnostic, and easy to diff.

| Directory | Purpose |
|---|---|
| `integration/` | Golden-output shell tests that invoke `spiritstream-cli` against an isolated `--data-dir <tmpdir>`. Each test seeds a known state, runs a command, and asserts stdout/stderr matches a checked-in fixture. |
| `threat-model/` | Threat-model fixtures: chat messages containing PII, images with GPS EXIF, log payloads with API tokens. Tests assert defenses fire correctly. |

The CLI is the source of truth for what the API surface can do. If a test cannot be expressed against the CLI, that is a bug in `spiritstream-core`'s contract — fix it before adding HTTP-only tests.
