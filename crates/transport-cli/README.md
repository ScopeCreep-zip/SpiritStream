# spiritstream-cli

In-process command-line transport for `spiritstream-core`. **Not a
debug tool** — a first-class deliverable that proves the
transport-agnostic core promise. Every operation reachable through
the React UI is reachable through this CLI; every CLI command
exercises the same `ServiceRegistry` and the same business rules.

## How it works

```
$ spiritstream-cli --data-dir /path/to/data <subcommand>
```

The binary builds a `ServiceRegistry` from `crates/core` directly —
no HTTP shell-out, no IPC. Output is JSON by default for scripting,
`--pretty` for humans, `--quiet` to suppress non-error output.

## Subcommand catalog

| Family | Commands |
|---|---|
| `profile` | `list`, `exists`, `show`, `save`, `delete`, `is-encrypted`, `summaries`, `validate-input`, `reorder`, `order`, `activate`, `decrypt`, `unlock`, `lock`, `locked-list` |
| `stream` | `start`, `start-all`, `stop`, `stop-all`, `status`, `retry`, `toggle-target` |
| `chat` | `status`, `send` |
| `oauth` | `start`, `complete`, `account`, `config` |
| `system` | `encoders`, `ffmpeg` |
| `settings` | `get`, `set` |
| `data` | `export` |
| `safety` | `panic`, `blocklist {list, add, remove}` |
| `events` | `watch` |

## Exit codes

CLI errors map `CoreError` variants to deterministic exit codes for
shell-scripting:

| Code | Meaning |
|---|---|
| 0 | Success |
| 4 | `ProfileNotFound` |
| 5 | `PasswordRequired` |
| 6 | `PasswordIncorrect` |
| 7 | `InvalidStreamConfig` / `ValidationFailed` / `ChatMessageLengthExceeded` |
| 8 | `EncoderUnavailable` |
| 9 | `PortConflict` / `ProfileAlreadyExists` |
| 10 | `FfmpegNotFound` |
| 13 | `PathOutsideAllowedRoot` |
| 14 | `RateLimited` |
| 15 | `Unauthorized` |
| 16 | `NoActiveProfile` |
| 17 | `ChatPlatformNotConnected` |
| 18 | `ChatSendingDisabled` |
| 19 | `ChatBlockedByPii` |
| 64 | `CliError::Argument` (EX_USAGE) |
| 65 | `CliError::Serialization` (EX_DATAERR) |
| 69 | `CoreError::NetworkError` (EX_UNAVAILABLE) |
| 70 | `CoreError::Internal` (EX_SOFTWARE) |
| 74 | `CliError::Io` (EX_IOERR) |
| 78 | `CoreError::NotImplemented` (EX_CONFIG) |

## Tests

```bash
cargo test -p spiritstream-cli
bash tests/integration/run.sh
```

24 in-process integration tests in `crates/transport-cli/tests/` plus
23 shell-driven golden-file tests under `tests/integration/cases/`.
Every command is exercised against a `--data-dir <tmpdir>` install.

## Why this exists

If a UI operation can't be done from the CLI, that's a contract bug
— fix `crates/core` first, then surface it here. The CLI is the
substrate that integration tests run against, the path headless
operators use to manage their server, and the proof that the core
is genuinely transport-independent.
