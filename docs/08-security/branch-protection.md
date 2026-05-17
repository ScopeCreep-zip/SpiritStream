# Branch protection and repository security settings

This document records the GitHub-side settings that the SpiritStream CI/CD
hardening pass relies on. They cannot be checked into the repo directly
(we're not on GitHub Enterprise rulesets-as-code), so the script at
`scripts/verify-branch-protection.sh` audits them on demand.

If a setting drifts, CI will keep passing but the security posture
silently weakens. Re-run the audit script after any organisation
permissions change, after adding a maintainer, or quarterly as a
hygiene check.

Authority backing for every setting below is in
`.claude/plans/velvety-wibbling-gadget.md` and the spine of
[CISA / NSA "Defending CI/CD Environments" (June 2023)][cisa-csi],
[NIST SP 800-218 SSDF v1.1][nist-218], and the
[OWASP Top 10 CI/CD Security Risks][owasp-cicd].

## Required repository settings

Settings → Code security:

- **Dependency graph** — on
- **Dependabot alerts** — on
- **Dependabot security updates** — on
- **Dependabot version updates** — on (configured by `.github/dependabot.yml`)
- **Secret scanning** — on (push protection enabled)
- **Push protection bypass** — restricted to org admins; document every
  bypass in the security advisory it relates to.
- **Code scanning** — on, using the `.github/workflows/codeql.yml`
  workflow (not the GitHub default setup).
- **Private vulnerability reporting** — on (this is the disclosure
  channel referenced from `SECURITY.md`).

Settings → Actions → General:

- **Actions permissions** — Allow ScopeCreep-zip, and select non-ScopeCreep-zip,
  actions and reusable workflows. Pin the allow-list to the action
  publishers we already use:
  `actions/*, github/codeql-action/*, sigstore/cosign-installer,
  pnpm/action-setup, dtolnay/rust-toolchain, Swatinem/rust-cache,
  tauri-apps/tauri-action, softprops/action-gh-release,
  step-security/harden-runner, msys2/setup-msys2,
  gitleaks/gitleaks-action, wagoid/commitlint-github-action`.
- **Require approval for first-time contributors** — on. This is the
  OWASP CICD-SEC-4 (poisoned-pipeline-execution) mitigation for fork
  PRs; it forces a maintainer click before any workflow runs against
  unreviewed contributor code.
- **Fork pull request workflows from outside collaborators** — Require
  approval for all outside collaborators.
- **Workflow permissions** — Read repository contents and packages
  permissions (default-deny token). Workflows that need more declare
  it inline via `permissions:` blocks.
- **Allow GitHub Actions to create and approve pull requests** — off
  (Dependabot uses its own automation; no other auto-PR-approval path
  should exist).

Settings → Webhooks: none should be active that send build / commit
metadata outside the org without a documented receiver. Review quarterly.

## `main` branch ruleset

Settings → Rules → Rulesets → New branch ruleset:

- **Target branches**: `main`
- **Enforcement**: active

Branch rules:

- **Restrict creations** — block. New branches must come from forks or
  feature branches, not be created on the protected branch.
- **Restrict updates** — block direct pushes; PR-only.
- **Restrict deletions** — block. The branch is permanent.
- **Require linear history** — on (no merge commits; squash- or rebase-merge).
- **Require deployments to succeed** — off (no GitHub deployments
  configured yet).
- **Require signed commits** — on. Maps to NIST SSDF PS.2 / SLSA
  source-integrity. Dependabot's commits are auto-signed by GitHub
  since 2023 so this does not break dep bumps.
- **Require a pull request before merging** — on, with:
  - Required approvals: **1** (raise to 2 when a second maintainer
    joins per the governance roadmap)
  - Dismiss stale pull request approvals when new commits are pushed: on
  - Require review from Code Owners: on
  - Require approval of the most recent reviewable push: on
  - Require conversation resolution before merging: on
- **Require status checks to pass** — on, with the following checks
  marked as required. Each is a job name (or matrix entry) from the
  workflows in `.github/workflows/`:
  - `CI / pnpm-audit`
  - `CI / a11y-axe`
  - `CI / test (macos-latest)`
  - `CI / test (ubuntu-22.04)`
  - `CI / test (windows-latest)`
  - `CodeQL / Analyze (javascript-typescript)`
  - `CodeQL / Analyze (rust)`
  - `cargo-deny / cargo-deny (advisories)`
  - `cargo-deny / cargo-deny (bans)`
  - `cargo-deny / cargo-deny (licenses)`
  - `cargo-deny / cargo-deny (sources)`
  - `Dependency Review / dependency-review`
  - `Secret Scan / gitleaks`
  - `PR Quality / commitlint`
  - `PR Quality / ai-disclosure`
  - `PR Quality / issue-link`
  - `PR Quality / diff-size`
  - **Require branches to be up to date before merging**: on
- **Block force pushes** — on.
- **Require code scanning results** — on, requiring CodeQL and
  reasonable severity thresholds (alert level: high; security level:
  high).

Bypass list:

- Organisation owners may bypass for emergencies. Every bypass must
  produce a public post-mortem within 7 days.

## `kali/rewrite` branch ruleset (rewrite-period)

Same as `main` with these adjustments while the branch is still
single-maintainer:

- Required approvals: **0** (single maintainer)
- Require review from Code Owners: **off** (would self-block)
- Required status checks: same list as `main`
- Linear history, signed commits, push restrictions, force-push block:
  all same as `main`

When a second maintainer joins, raise the approval count to 1 and turn
CODEOWNERS review back on.

## Release-branch / tag protection

Settings → Tags → New protection rule:

- Tag name pattern: `v*`
- Restrict creation: org admins only (release process runs from a
  maintainer machine; no contributor should be able to push a release
  tag).

## Re-auditing

Run the script periodically:

```bash
bash scripts/verify-branch-protection.sh
```

It hits the GitHub REST API (with your `gh` token) and reports any
missing protection. The script is read-only — it never modifies
settings; correcting drift is a human action.

[cisa-csi]: https://www.cisa.gov/news-events/alerts/2023/06/28/cisa-and-nsa-release-joint-guidance-defending-continuous-integrationcontinuous-delivery-cicd
[nist-218]: https://csrc.nist.gov/pubs/sp/800/218/final
[owasp-cicd]: https://owasp.org/www-project-top-10-ci-cd-security-risks/
