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
| `stream` | `start`, `start-all`, `stop`, `stop-all`, `status`, `retry`, `toggle-target`, `target enable/disable`, `group enable/disable` |
| `chat` | `status`, `send`, `connect`, `disconnect`, `export-log` |
| `oauth` | `start`, `complete`, `refresh`, `forget`, `account`, `config` |
| `system` | `encoders`, `ffmpeg`, `test-rtmp`, `health` |
| `settings` | `get`, `set` |
| `theme` | `list` |
| `data` | `export`, `clear`, `rotate-machine-key` |
| `obs` / `discord` / `files` | integration + file-browser primitives |
| `safety` | `panic`, `blocklist {list, add, remove}` |
| `audit` | `log`, `verify` (current chain + archives) |
| `session` | `list`, `revoke-all` (cross-process — reaches a running server's sessions) |
| `confirm-token` | `issue` (one-shot tokens for destructive ops) |
| `events` | `watch` |

## Secret input model

Secrets never ride argv (`ps`/shell-history-safe), following the
docker/gh convention:

1. **By reference** — commands that operate on secrets core already
   stores take *names*, not values: `oauth refresh <provider>` reads
   the active profile's stored refresh token, `oauth forget` clears
   stored tokens, `chat connect` uses stored credentials.
2. **Net-new entry** — every secret-accepting flag is a `--*-from
   stdin|prompt` selector: `stdin` reads the first line from stdin
   (pipe-friendly: `profile activate name --password-from stdin
   <<<"$PW"`), `prompt` uses a no-echo TTY prompt. A non-TTY without
   `stdin` fails loudly instead of hanging.
3. **Bulk unlock** — `data rotate-machine-key --passwords-stdin`
   accepts `name:password` lines for encrypted profiles; without the
   flag, an interactive terminal prompts per profile.

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

In-process integration tests live in `crates/transport-cli/tests/`;
the shell-driven golden-file suite under `tests/integration/cases/`
exercises every command family against a `--data-dir <tmpdir>` install
(38 cases at the time of writing — `bash tests/integration/run.sh`
prints the authoritative count).

## Why this exists

If a UI operation can't be done from the CLI, that's a contract bug
— fix `crates/core` first, then surface it here. The CLI is the
substrate that integration tests run against, the path headless
operators use to manage their server, and the proof that the core
is genuinely transport-independent.
