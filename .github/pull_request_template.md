<!--
Thanks for contributing. SpiritStream ships to vulnerable users
(adult creators, harassment targets, journalists, trans/queer/disabled
streamers). Their safety depends on us shipping carefully — please fill
every section below honestly.

Sections marked REQUIRED are gated by CI; PR will fail without them.
-->

## Summary
<!-- 1–2 sentences. What does this change do and why? -->


## Linked issue (REQUIRED)
<!--
Use a closing keyword + issue number. Examples:
  Fixes #123
  Closes #456
  Resolves ScopeCreep-zip/SpiritStream#789

If this is a refactor / docs change / something genuinely without an
issue, apply the `no-issue-required` label after opening the PR.
-->
Fixes #


## Type of change
<!-- Check all that apply. -->
- [ ] Bug fix
- [ ] New feature
- [ ] Refactor (no behavior change)
- [ ] Documentation
- [ ] Build / CI
- [ ] Security fix (consider [private vulnerability reporting](https://github.com/ScopeCreep-zip/SpiritStream/security/advisories/new) instead)


## Testing performed
<!--
What did you actually run? "It compiles" is not a test.
List the cargo / pnpm / integration commands and any manual UI flows you
clicked through. For UI / a11y changes, screenshots or a screen recording
help reviewers see what you saw.
-->


## Checklist
- [ ] `pnpm typecheck` passes
- [ ] `cargo check --workspace` passes
- [ ] `cargo clippy --workspace -- -D warnings` passes (touched crates)
- [ ] `pnpm lint` passes
- [ ] No hardcoded secrets, tokens, or personal paths
- [ ] Commit messages follow [Conventional Commits](../CONTRIBUTING.md#commit-message-format)
- [ ] No new `TODO` / `FIXME` markers in source (track in plans / issues)
- [ ] No new `#[allow(dead_code)]` and no fields without a live caller


## AI-assistance disclosure (REQUIRED if applicable)
<!--
SpiritStream follows the Linux-kernel rule: AI assistance is allowed,
but if it was material to the change you MUST disclose it via an
`Assisted-by:` trailer on each affected commit, e.g.

    Assisted-by: Claude (Sonnet 4.6)
    Assisted-by: GitHub Copilot
    Assisted-by: ChatGPT (GPT-5)

You — the human submitter — remain fully responsible for correctness,
security, and licensing. If a reviewer can tell the code was generated
and you didn't disclose, the PR will be closed.

If no AI was used, leave the box unchecked.
-->
- [ ] AI assistance was used and is disclosed via `Assisted-by:` trailers on the relevant commits
- [ ] I have read and understand every line of code in this change


## Threat-model impact (security-sensitive changes only)
<!--
Skip this section unless your change touches: auth, secrets, IPC,
network, file I/O, OAuth, encryption, audit log, PII filter, panic
disconnect, EXIF/PII strip, or a new external dependency.
-->
- [ ] Does this change weaken any guarantee in `.claude/claudedocs/` threat-model docs?
- [ ] Have you considered the impact on harassment-prone / journalist / adult-creator users specifically?
- [ ] Have you added or updated tests in `tests/threat-model/`?


## Screenshots (UI changes)
<!-- Drag-drop or paste. Include light + dark mode if relevant. -->
