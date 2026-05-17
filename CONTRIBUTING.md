# Contributing to SpiritStream

SpiritStream ships to vulnerable user communities — adult creators,
harassment-prone streamers, journalists, trans/queer/disabled users —
whose safety depends on the app being correct, predictable, and
audit-able. That shapes how we accept contributions: high bar on care,
high bar on disclosure, zero tolerance for silent fallbacks.

If you read nothing else, read [§AI-assisted contributions](#ai-assisted-contributions)
and [§Commit message format](#commit-message-format) before opening a PR.

## Quick start

```bash
# install toolchains (pin in repo)
#   - Rust:  read from rust-toolchain.toml
#   - Node:  read from .nvmrc (use `nvm use`)
pnpm install
cargo check --workspace
pnpm typecheck
```

A pre-commit harness is set up via [lefthook](https://github.com/evilmartians/lefthook):

```bash
cargo install lefthook   # one-off
lefthook install         # wires it to .git/hooks/
```

Once installed, every commit runs `cargo fmt --check`, `cargo clippy
-D warnings` on touched crates, `pnpm lint`, prettier, and `gitleaks
protect --staged` in parallel. Commit-msg hook runs commitlint.

## Before you open an issue

- Bugs → use the [Bug report template](.github/ISSUE_TEMPLATE/bug_report.yml).
- Features → use the [Feature request template](.github/ISSUE_TEMPLATE/feature_request.yml).
- Security vulnerabilities → **never** in a public issue. Use
  [Private Vulnerability Reporting](https://github.com/ScopeCreep-zip/SpiritStream/security/advisories/new).
  See [SECURITY.md](SECURITY.md) for the full disclosure policy.

## Before you open a pull request

1. Link a tracking issue (or apply the `no-issue-required` label).
2. Run `pnpm typecheck`, `cargo check --workspace`, `cargo clippy
   --workspace -- -D warnings`, `pnpm lint` locally.
3. Add or update tests in the relevant crate / `tests/` directory.
4. Fill in every section of the [PR template](.github/pull_request_template.md).
5. Keep the diff small (under 800 lines is ideal; over 3000 is hard-blocked
   without a `large-pr-approved` label).

## Coding standards

Authoritative rules live in `.claude/rules/`:

- [`architecture.md`](.claude/rules/architecture.md) — layering, transport
  isolation, no business logic in the frontend, no transport types in core.
- [`coding-standards.md`](.claude/rules/coding-standards.md) — naming,
  error handling, sensitive-data masking, DTO camelCase, no TODOs in source,
  no `#[allow(dead_code)]`.
- [`git-workflow.md`](.claude/rules/git-workflow.md) — branch naming,
  commit message format, PR title format, pre-commit checks.
- [`documentation.md`](.claude/rules/documentation.md) — where to put docs.

A few highlights:

- **Rust**: structured `CoreError` enum (no `Result<T, String>`),
  `Arc<ServiceRegistry>` wiring, `mask_sensitive()` on every log payload
  containing tokens / keys / cookies. No `#[allow(dead_code)]`; every
  adapter / field needs a live caller in the same change.
- **TypeScript**: strict mode, explicit return types, import domain
  types from `@spiritstream/types` only — never re-declare. All API
  calls go through the `api` client; never `fetch()` directly.
- **DTOs**: write transport DTOs with `#[serde(rename_all = "camelCase")]`
  from the first draft. Do not reactively add it after test failures.
- **No silent fallbacks**: where a primary / secondary implementation
  exists (e.g. keyring vs. encrypted-file secret store), the choice is
  made once at startup via probe or env override and held for the
  process lifetime. Never `try_primary().or_else(secondary)` at the
  call site. Streaming apps that fall back silently break vulnerable
  users' streams in production.
- **No TODO / FIXME / XXX markers in source.** Track future work in
  plans, GitHub issues, or `crates/transport-veilid/BLOCKERS.md` — never
  in code.

## Commit message format

We enforce [Conventional Commits](https://www.conventionalcommits.org/)
via commitlint in CI (`.github/workflows/pr-quality.yml`) and locally
via lefthook.

```
<type>(<scope>): <short summary in lower case, no trailing period>

<optional body — REQUIRED for feat and fix, explains the *why*>

<optional footer with metadata trailers>
```

**Allowed types**: `feat`, `fix`, `docs`, `style`, `refactor`, `test`,
`chore`, `perf`, `build`, `ci`, `revert`.

**Allowed scopes** (one of): `server`, `frontend`, `desktop`, `mobile`,
`models`, `services`, `api`, `core`, `transport-http`, `transport-cli`,
`transport-veilid`, `ui`, `a11y`, `build`, `ci`, `deps`, `release`,
`docs`, `security`.

**Body requirement on `feat` and `fix`**: tell us *why* the change
exists. Reviewers can read the diff; the body is the place for the
motivation, the constraint, the threat model that drove the choice.

### Example

```
feat(services): bound OAuth refresh attempts in auth_surveillance

Cap refresh attempts to 8 per rolling hour per provider. Above that,
emit an AuthSurveillanceAnomaly to the audit log and stall the queue
for 15 minutes. The cap is high enough that legitimate token rotation
under heavy multi-platform use still works, low enough that a stolen
refresh-token replay loop trips it inside a single hour.

Closes #214
```

## AI-assisted contributions

**SpiritStream allows AI assistance, but you must disclose it and you
are fully accountable for the result.**

We follow the [Linux kernel rule](https://lwn.net/Articles/991189/)
(formalized April 2026) and a stricter version of the
[Rust RFC #3950](https://github.com/rust-lang/rfcs/pull/3950) standard:

1. **Disclose with a trailer.** If AI was material to a commit
   (generated more than trivial scaffolding, drafted the code you
   submitted, or did the refactor you accepted), add an `Assisted-by:`
   trailer to that commit:

   ```
   Assisted-by: Claude (Sonnet 4.6)
   Assisted-by: GitHub Copilot
   Assisted-by: ChatGPT (GPT-5)
   ```

   The `Signed-off-by:` trailer is for the human submitter — never the AI.

2. **You are responsible.** All bugs, regressions, security flaws, and
   licensing problems in your PR attribute to you. "The AI generated
   it" is not a defense; you are the one who chose to submit it. If you
   would not stake your reputation on the code as-written, do not
   submit it.

3. **You must understand the code.** Be able to explain to a reviewer
   what each non-trivial line does, why it is correct, and what the
   failure modes are. If a reviewer asks "why did you pick this
   approach?" the answer cannot be "the AI suggested it."

4. **No vibecoding.** Fully-automated PRs, low-effort scaffolds, and
   "I ran it through an LLM and tests passed" submissions get closed
   without review.

5. **CI enforces disclosure detection.** Our `pr-quality.yml` workflow
   scans commit messages for AI tool names and fails the PR if any
   commit body mentions AI without the matching `Assisted-by:` trailer.
   The check can be bypassed (with maintainer approval) by applying the
   `skip-ai-disclosure-check` label — use it only for fixtures / docs
   where the mention is part of the content itself.

### What "material" means

Use the `Assisted-by:` trailer if:

- You asked an AI to write or rewrite the function and accepted its
  output with limited modification.
- You used AI to debug and applied its suggested fix.
- You let an AI generate the tests, the docs, or the comments.

You don't need to disclose if:

- The AI only autocompleted variable names you were typing anyway.
- You used AI to look something up but wrote the code yourself.
- You ran spell-check or grammar tooling on the prose.

When in doubt, disclose.

## Testing

- **Rust**: `cargo test --workspace`. Integration tests under
  `tests/integration/` are CLI-driven and golden-compared
  (`bash tests/integration/run.sh` after building the CLI).
- **Threat-model fixtures**: `tests/threat-model/` covers doxxing /
  EXIF / PII regression cases. Add to it if your change touches the
  safety service, PII filter, or anonymous-mode pseudonymizer.
- **Accessibility**: `pnpm tsx tests/a11y/axe-run.ts apps/web/dist`
  runs axe-core against the built frontend. CI fails on serious /
  critical violations.
- **Frontend**: `pnpm --filter @spiritstream/web test` (when present).

## Branch and PR conventions

See [`.claude/rules/git-workflow.md`](.claude/rules/git-workflow.md)
for full details. Short version: `feature/`, `fix/`, `refactor/`,
`docs/`, `hotfix/` prefixes; PR title matches the commit message format.

## Security and threat-model considerations

Anything touching auth, secrets, IPC, network, file I/O, OAuth,
encryption, audit log, PII filter, panic disconnect, or EXIF/PII
stripping is **security-sensitive**. Such PRs require:

- Explicit threat-model reasoning in the PR description.
- Tests covering the failure modes you considered.
- A reviewer review from a security-domain CODEOWNER.

See [`.claude/claudedocs/`](.claude/claudedocs/) for population-specific
threat docs.

## Documentation

Per [`.claude/rules/documentation.md`](.claude/rules/documentation.md),
Claude-generated docs land under `.claude/claudedocs/`. User-facing
documentation lives in `docs/`. Per-crate READMEs live in
`crates/<crate>/README.md`.

## License

SpiritStream is [ISC-licensed](LICENSE). Contributions are accepted
under the same license; opening a PR signals you have the right to
submit the code under ISC.
