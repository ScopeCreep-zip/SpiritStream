# MILESTONES - Latest

**Type**: Living Document
**Authority**: Delegated from [GOVERNANCE.md](./GOVERNANCE.md)
**Created**: 2026-01-18
**Last Updated**: 2026-01-18
**Ceremony**: [MILESTONES_CEREMONY.md](./MILESTONES_CEREMONY.md)
**Branch**: `latest`

---

## Document Purpose

This is the **living commitments document** containing accepted proposals being actively worked toward. Per the [milestone ceremony](./MILESTONES_CEREMONY.md), this document:

- Evolves until a version cut freezes it
- May have blockers on specific sections
- Represents what we've agreed to, not what we've built
- Aligns with the `latest` branch

### Branch Alignment

This document aligns with the `latest` branch:
- CI **MUST** pass on every commit to `latest`
- Production-ready code available upstream
- Artifacts generated for early adopters
- Features can be fully experienced before governance commits

```
main ───────────> latest ───────────> v1.0.0-stable
(may break)       (must pass)         (frozen release)
      │                │
      │ auto-promote   │ version cut
      │ (CI passes)    │ (governance accepts)
      ▼                ▼
   Proposals      Users experience
   develop        features BEFORE
   here           governance commits
```

### Current Branch State

| Branch | Exists | Status |
|--------|--------|--------|
| `main` | Yes | Development intake |
| `latest` | **NO** | BLOCKED: M6.1 not complete |
| `v1.0.0-*` | No | Pending version cut |

**This document's governance is not yet operational.** The `latest` branch must be created by completing M6: Release Infrastructure.

For the governance process, see [MILESTONES_CEREMONY.md](./MILESTONES_CEREMONY.md).
For proposals under discussion, see [MILESTONES.main.md](./MILESTONES.main.md).

This document establishes:

1. **Maintainer Surface Area** - Who owns what domains
2. **Scope Boundaries** - What's in/out of scope for v1
3. **Process Commitments** - What we promise to contributors
4. **Measurable Accountability** - How we track ourselves

This is **not** a feature roadmap. For technical features, see [roadmap.md](.claude/claudedocs/roadmap.md).

---

## V1.0.0 Release Criteria

**V1.0.0-stable is automatically released when ALL milestones reach `COMPLETE` status.**

```
┌─────────────────────────────────────────────────────────────────────┐
│                    V1.0.0 RELEASE GATE                               │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│   M1: Community Foundation ────────────────────────> [ ] COMPLETE   │
│   M2: Review Process ──────────────────────────────> [ ] COMPLETE   │
│   M3: Platform Distribution ───────────────────────> [ ] COMPLETE   │
│   M4: Documentation ───────────────────────────────> [ ] COMPLETE   │
│   M5: Quality Assurance ───────────────────────────> [ ] COMPLETE   │
│   M6: Release Infrastructure ──────────────────────> [ ] COMPLETE   │
│   M7: Repository Infrastructure ───────────────────> [ ] COMPLETE   │
│                                                                      │
│   ─────────────────────────────────────────────────────────────────  │
│                                                                      │
│   ALL COMPLETE ──> Semantic Release ──> v1.0.0-stable branch        │
│                                     ──> MILESTONES.v1.0.0-stable.md │
│                                     ──> Release artifacts           │
│                                                                      │
└─────────────────────────────────────────────────────────────────────┘
```

---

## Item Statuses

Per [MILESTONES_CEREMONY.md](./MILESTONES_CEREMONY.md), items in this document have explicit status:

| Status | Meaning | Test |
|--------|---------|------|
| `PROPOSED` | Under discussion | N/A |
| `ACCEPTED` | Committed with criteria | Criteria defined |
| `BLOCKED` | Has unresolved blocker | Blocker documented |
| `COMPLETE` | All criteria met | All tests pass |
| `DEFERRED` | Moved to future version | Rationale documented |
| `REJECTED` | Removed from scope | Rationale documented |

### Status Transitions

```
PROPOSED ──┬──> ACCEPTED ──┬──> COMPLETE
           │               │
           │               └──> BLOCKED ──> (resolve) ──> ACCEPTED
           │
           ├──> DEFERRED (to next version cycle)
           │
           └──> REJECTED (with rationale)
```

### Test-Driven Development Approach

Each deliverable has **acceptance criteria written as testable outcomes**:

```
Deliverable: "latest branch exists"
Test: git branch -a | grep -q "latest" && echo PASS || echo FAIL
Outcome: PASS required for COMPLETE status
```

### Validation Outcome Standards

Acceptance tests MUST follow these standards:

| Standard | Requirement | Example |
|----------|-------------|---------|
| **Deterministic** | Same inputs produce same outputs | File existence checks, not timing-dependent |
| **Automated** | No human judgment required | CI can execute without prompts |
| **Documented** | Test commands are explicit | Bash snippet, not "verify it works" |
| **Idempotent** | Running twice gives same result | Read-only operations preferred |
| **Scoped** | Tests only the deliverable | No cross-cutting concerns |

**Validation Types**:

| Type | When to Use | Example |
|------|-------------|---------|
| **Presence** | File/branch must exist | `test -f FILE` |
| **Content** | File must contain pattern | `grep -q "PATTERN" FILE` |
| **Execution** | Command must succeed | `cargo test --package X` |
| **Integration** | End-to-end workflow | CI pipeline completion |
| **External** | Depends on external system | `gh release list` |

**Cross-Cutting Concerns**:

Some validations (e.g., commit format, code style) apply across all milestones. These are:

1. **NOT duplicated** in each milestone's acceptance test
2. **Centralized** in the milestone that owns the enforcement mechanism
3. **Referenced** from dependent deliverables via `(see M#.#)`

Example: Commit format validation is owned by M6.6 (Conventional Commits enforced), not M1.

---

## Scope Definition

### In Scope for v1

| Domain | Description | Milestone |
|--------|-------------|-----------|
| **Core Streaming** | RTMP relay, output groups, stream targets | M3 |
| **Profile Management** | Encrypted profiles, import/export | M3 |
| **Platform Support** | Linux (.deb, .rpm), macOS, Windows | M3 |
| **Governance** | Full governance framework operational | M1, M2 |
| **Documentation** | User guide, troubleshooting, API docs | M4 |
| **Testing** | Platform validation, outcome testing | M5 |
| **Release Automation** | Semantic releases, branch promotion | M6 |
| **Repository** | Multi-origin, Nix builds | M7 |

### Explicitly Out of Scope for v1

| Domain | Rationale | Future Version |
|--------|-----------|----------------|
| **Scene Management** | Architectural complexity | v2.0 |
| **Desktop Capture** | Platform-specific effort | v1.2+ |
| **Plugin System** | Premature abstraction | v2.0+ |
| **Cloud Sync** | Infrastructure dependency | v3.0 |
| **Mobile Apps** | Different technology stack | Separate project |

### Scope Change Process

1. Open an issue with `scope-change` label
2. Discuss impact on maintainer capacity
3. Update this document with `DEFERRED` or `REJECTED` status
4. If accepted, add to appropriate milestone with `PROPOSED` status

---

## M1: Community Foundation

**Status**: `ACCEPTED`
**Owner**: @usrbinkat
**Target**: v1.0.0

### Deliverables

| ID | Deliverable | Status | Acceptance Test |
|----|-------------|--------|-----------------|
| M1.1 | CONTRIBUTORS.md | `COMPLETE` | File exists, covers: setup, standards, PR process, PII prohibition |
| M1.2 | REVIEWERS.md | `COMPLETE` | File exists, covers: process, checklists, templates, SLAs |
| M1.3 | GOVERNANCE.md | `COMPLETE` | File exists, defines authority hierarchy, RFC 2119 normative language |
| M1.4 | MILESTONES_CEREMONY.md | `COMPLETE` | File exists, defines branch-document alignment |
| M1.5 | MILESTONES.main.md | `COMPLETE` | File exists, proposal template present |
| M1.6 | MILESTONES.latest.md | `ACCEPTED` | This document; `COMPLETE` when all milestones testable |
| M1.7 | CODE_OF_CONDUCT.md | `PROPOSED` | Contributor Covenant adopted |
| M1.8 | Issue Templates | `PROPOSED` | Bug report, feature request, security report templates exist |
| M1.9 | PR Template | `PROPOSED` | Template exists with summary, changes, test plan, checklist |
| M1.10 | DEPENDENCIES.md | `COMPLETE` | File exists, documents all runtime/build dependencies |
| M1.11 | OpenCode Commit Tooling | `ACCEPTED` | Commits validated by commitlint or semantic-release (see M6.6) |

### Acceptance Test

```bash
# M1 Acceptance Test
# Document existence validation
test -f CONTRIBUTORS.md && \
test -f REVIEWERS.md && \
test -f GOVERNANCE.md && \
test -f MILESTONES_CEREMONY.md && \
test -f MILESTONES.main.md && \
test -f MILESTONES.latest.md && \
test -f DEPENDENCIES.md && \
test -f CODE_OF_CONDUCT.md && \
test -d .github/ISSUE_TEMPLATE && \
test -f .github/PULL_REQUEST_TEMPLATE.md && \
# OpenCode tooling presence
test -f opencode.json && \
test -d .opencode/command && \
test -d .opencode/skill && \
echo "M1: PASS" || echo "M1: FAIL"

# NOTE: Commit conformance is NOT validated here.
# Conventional commit enforcement is a cross-cutting concern
# validated by M6.6 (commitlint pre-commit hooks or CI).
# See: M1.11 acceptance criteria.
```

**Graduation Criteria**: All deliverables reach `COMPLETE` status.

### M1.11: OpenCode Commit Tooling — Detailed Specification

**Purpose**: Provide AI-assisted commit tooling that enforces conventional commit standards during development, ensuring consistent commit messages before they reach CI validation (M6.6).

#### Deliverable Components

| File | Type | Purpose |
|------|------|---------|
| `opencode.json` | Configuration | OpenCode AI tool configuration with MCP servers for Kubernetes, Gitea, NixOS, browser automation |
| `.opencode/command/commit.md` | Slash Command | `/commit` command definition that invokes the git-commit skill |
| `.opencode/skill/git-commit/SKILL.md` | Skill Definition | 500+ line conventional commits skill implementing conventionalcommits.org v1.0.0 specification |

#### Justification

| Component | Why It's Needed |
|-----------|-----------------|
| **opencode.json** | Configures the AI development environment with project-specific MCP servers. Enables consistent tooling across all contributors using OpenCode. Uses `{env:VAR}` placeholders for secrets—no credentials in repository. |
| **commit.md** | Provides discoverable `/commit` command for developers. Reduces friction for conventional commit adoption. Maps to skill for implementation. |
| **SKILL.md** | Implements full conventional commits specification. Provides AI with detailed rules for type selection, scope formatting, breaking change detection, and message composition. Ensures commits pass M6.6 validation. |

#### Acceptance Criteria

| Criterion | Validation | Status |
|-----------|------------|--------|
| `opencode.json` exists at repo root | `test -f opencode.json` | Presence |
| `opencode.json` contains no secrets | `grep -qE '(password|token|key).*:.*[^{]' opencode.json && echo FAIL \|\| echo PASS` | Content |
| `.opencode/command/commit.md` exists | `test -f .opencode/command/commit.md` | Presence |
| `.opencode/skill/git-commit/SKILL.md` exists | `test -f .opencode/skill/git-commit/SKILL.md` | Presence |
| Skill references conventionalcommits.org | `grep -q "conventionalcommits.org" .opencode/skill/git-commit/SKILL.md` | Content |
| Skill defines commit types | `grep -qE "^(feat|fix|docs|style|refactor|perf|test|build|ci|chore|revert)" .opencode/skill/git-commit/SKILL.md` | Content |

#### Validation Scope

**In Scope**:
- File presence validation
- No-secrets validation for configuration
- Skill content references correct specification

**Out of Scope** (validated by M6.6):
- Actual commit message validation at commit time
- Pre-commit hook enforcement
- CI rejection of non-conforming commits

#### Relationship to M6.6

```
M1.11 (OpenCode Tooling)          M6.6 (Conventional Commits Enforced)
─────────────────────────         ────────────────────────────────────
AI-assisted authoring             Automated enforcement
Guidance during development   →   Rejection of non-conforming commits
Skill teaches the rules           commitlint/semantic-release validates

Developer Experience              CI/CD Pipeline
(helpful suggestions)             (hard requirements)
```

M1.11 is `ACCEPTED` (not `COMPLETE`) because full validation requires M6.6 infrastructure. The tooling exists and guides developers, but enforcement awaits release infrastructure.

---

## M2: Review Process Operational

**Status**: `PROPOSED`
**Owner**: TBD
**Target**: v1.0.0

### Deliverables

| ID | Deliverable | Status | Acceptance Test |
|----|-------------|--------|-----------------|
| M2.1 | Review SLA Tracking | `PROPOSED` | GitHub Action measures time-to-first-review |
| M2.2 | CODEOWNERS | `PROPOSED` | File exists, auto-assigns reviewers |
| M2.3 | First External PR | `PROPOSED` | Non-maintainer PR merged following REVIEWERS.md |
| M2.4 | Process Retrospective | `BLOCKED` | Requires M2.3 complete first |

### Blockers

| ID | Blocker | Plan | Owner |
|----|---------|------|-------|
| B-M2.1 | No external contributors yet | Promote project, good-first-issues | @usrbinkat |
| B-M2.4 | Depends on M2.3 | Sequential dependency | - |

### Acceptance Test

```bash
# M2 Acceptance Test
test -f .github/CODEOWNERS && \
gh pr list --state merged --author "!@usrbinkat" --limit 1 | grep -q . && \
echo "M2: PASS" || echo "M2: FAIL"
```

**Graduation Criteria**: 3 external PRs reviewed following documented process.

---

## M3: Platform Distribution

**Status**: `ACCEPTED`
**Owner**: @usrbinkat
**Target**: v1.0.0

### Deliverables

| ID | Deliverable | Status | Acceptance Test |
|----|-------------|--------|-----------------|
| M3.1 | Linux .deb | `ACCEPTED` | `dpkg -i spiritstream.deb` succeeds on Ubuntu 22.04+ |
| M3.2 | Linux .rpm | `ACCEPTED` | `rpm -i spiritstream.rpm` succeeds on Fedora 38+ |
| M3.3 | macOS .dmg | `BLOCKED` | Opens and installs on macOS 13+ (Intel + ARM) |
| M3.4 | Windows .msi | `BLOCKED` | Installs on Windows 10+ |
| M3.5 | Linux AppImage | `ACCEPTED` | Downloads and runs on any Linux |
| M3.6 | Nix Flake | `BLOCKED` | `nix run github:ScopeCreep-zip/SpiritStream` works |
| M3.7 | CI Build Pipeline | `ACCEPTED` | All platforms build in CI |
| M3.8 | Cross-platform patch script | `COMPLETE` | Nix-built binaries work on non-NixOS |

### Blockers

| ID | Blocker | Discovery Plan | Owner |
|----|---------|----------------|-------|
| B-M3.3 | macOS code signing | Research Apple Developer Program requirements | TBD |
| B-M3.4 | Windows code signing | Research Authenticode requirements | TBD |
| B-M3.6 | Nix flake not created | Create flake.nix with devshell and package | @usrbinkat |

### Acceptance Test

```bash
# M3 Acceptance Test (CI validates per-platform)
# Each platform build produces artifact in CI
gh run list --workflow=release.yml --status=success --limit 1 | grep -q . && \
echo "M3: PASS" || echo "M3: FAIL"
```

**Graduation Criteria**: All packages install and basic streaming works on each platform.

---

## M4: Documentation Complete

**Status**: `PROPOSED`
**Owner**: TBD
**Target**: v1.0.0

### Deliverables

| ID | Deliverable | Status | Acceptance Test |
|----|-------------|--------|-----------------|
| M4.1 | User Guide | `PROPOSED` | New user can stream within 10 minutes following guide |
| M4.2 | Troubleshooting Guide | `PROPOSED` | Top 10 issues documented with solutions |
| M4.3 | Architecture Overview | `PROPOSED` | Diagram + explanation exists for contributors |
| M4.4 | API Documentation | `PROPOSED` | All Tauri commands documented with examples |
| M4.5 | Setup Scripts Documented | `PROPOSED` | setup.sh and setup.ps1 usage documented |

### Blockers

| ID | Blocker | Discovery Plan | Owner |
|----|---------|----------------|-------|
| B-M4.0 | No documentation owner | Recruit or maintainer assumes | @usrbinkat |

### Acceptance Test

```bash
# M4 Acceptance Test
test -f docs/USER_GUIDE.md && \
test -f docs/TROUBLESHOOTING.md && \
test -f docs/ARCHITECTURE.md && \
test -f docs/API.md && \
echo "M4: PASS" || echo "M4: FAIL"
```

**Graduation Criteria**: Documentation reviewed by someone unfamiliar with project.

---

## M5: Quality Assurance

**Status**: `PROPOSED`
**Owner**: TBD
**Target**: v1.0.0

### Deliverables

| ID | Deliverable | Status | Acceptance Test |
|----|-------------|--------|-----------------|
| M5.1 | Platform Test Matrix | `PROPOSED` | All platforms tested, results in CI artifacts |
| M5.2 | Integration Tests | `PROPOSED` | Rust integration tests pass: `cargo test` |
| M5.3 | Frontend Tests | `PROPOSED` | TypeScript tests pass: `npm test` |
| M5.4 | E2E Stream Test | `BLOCKED` | Automated test: start stream, verify output |
| M5.5 | Load Testing | `PROPOSED` | 5+ simultaneous streams stable for 1 hour |
| M5.6 | Security Audit | `BLOCKED` | No critical/high vulnerabilities in dependencies |
| M5.7 | Performance Baseline | `PROPOSED` | Memory/CPU benchmarks documented |

### Blockers

| ID | Blocker | Discovery Plan | Owner |
|----|---------|----------------|-------|
| B-M5.4 | E2E test infrastructure | Research: Playwright + FFmpeg test harness | TBD |
| B-M5.6 | Security audit scope undefined | Define scope: deps only vs full audit | TBD |

### Acceptance Test

```bash
# M5 Acceptance Test
cargo test --manifest-path src-tauri/Cargo.toml && \
npm run typecheck && \
npm run lint && \
echo "M5: PASS" || echo "M5: FAIL"
```

**Graduation Criteria**: All tests pass, results documented.

---

## M6: Release Infrastructure

**Status**: `BLOCKED`
**Owner**: @usrbinkat
**Target**: v1.0.0
**Priority**: CRITICAL - Blocks governance operationalization

This milestone implements the promotion pipeline defined in [GOVERNANCE.md](./GOVERNANCE.md) and [MILESTONES_CEREMONY.md](./MILESTONES_CEREMONY.md).

### Promotion Pipeline

```
┌─────────────────────────────────────────────────────────────────────┐
│                     PROMOTION PIPELINE                               │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│   Developer pushes to main                                          │
│         │                                                            │
│         ▼                                                            │
│   CI runs on main (may fail)                                        │
│         │                                                            │
│         ├── FAIL ──> Developer fixes                                │
│         │                                                            │
│         └── PASS ──> Auto-merge to latest (M6.2)                    │
│                           │                                          │
│                           ▼                                          │
│                   Semantic Release analyzes commits (M6.3)          │
│                           │                                          │
│                           ├── No release commits ──> No action      │
│                           │                                          │
│                           └── Release commits ──> Determine version │
│                                       │                              │
│                                       ▼                              │
│                               Preview release from latest            │
│                               (nightly/preview artifacts)            │
│                                                                      │
│   ─────────────────────────────────────────────────────────────────  │
│                                                                      │
│   Governance accepts version (manual trigger)                        │
│         │                                                            │
│         ▼                                                            │
│   Create v1.0.0-alpha branch from latest (M6.4)                     │
│         │                                                            │
│         ▼                                                            │
│   Semantic Release creates alpha artifacts                          │
│         │                                                            │
│         ▼                                                            │
│   Stage promotion: alpha ──> beta ──> stable (M6.5)                 │
│                                                                      │
└─────────────────────────────────────────────────────────────────────┘
```

### Deliverables

| ID | Deliverable | Status | Acceptance Test |
|----|-------------|--------|-----------------|
| M6.1 | `latest` branch exists | `BLOCKED` | `git branch -a \| grep -q latest` |
| M6.2 | Auto-promote main→latest | `BLOCKED` | CI workflow promotes on green build |
| M6.3 | Semantic Release configured | `BLOCKED` | `.releaserc` or `release.config.js` exists |
| M6.4 | Version branch automation | `BLOCKED` | `v*` branch created on governance trigger |
| M6.5 | Stage promotion workflow | `BLOCKED` | alpha→beta→stable promotion works |
| M6.6 | Conventional Commits enforced | `PROPOSED` | commitlint or similar validates commits |
| M6.7 | Changelog generation | `BLOCKED` | CHANGELOG.md auto-generated from commits |
| M6.8 | GitHub Release creation | `BLOCKED` | Releases created with artifacts attached |
| M6.9 | Preview artifacts from latest | `BLOCKED` | Nightly/preview builds available |

### Blockers

| ID | Blocker | Discovery Plan | Owner |
|----|---------|----------------|-------|
| B-M6.1 | Latest branch not created | Manual creation, then automation | @usrbinkat |
| B-M6.2 | Auto-promotion not configured | **DISCOVERY**: Research GitHub Actions for branch promotion | @usrbinkat |
| B-M6.3 | Semantic release tooling unknown | **DISCOVERY**: Evaluate semantic-release vs release-please vs custom | @usrbinkat |
| B-M6.4 | Version branch creation manual | Depends on M6.3 tooling choice | @usrbinkat |
| B-M6.5 | Stage promotion undefined | Depends on M6.3 tooling choice | @usrbinkat |
| B-M6.7 | Changelog tooling unknown | Depends on M6.3 tooling choice | @usrbinkat |
| B-M6.8 | Release creation manual | Depends on M6.3 tooling choice | @usrbinkat |
| B-M6.9 | Preview artifacts not configured | Depends on M6.2 | @usrbinkat |

### Discovery: Semantic Release Tooling

**Status**: `BLOCKED` - Discovery required

**Options to evaluate**:

| Tool | Pros | Cons | Research Link |
|------|------|------|---------------|
| semantic-release | Industry standard, plugin ecosystem | Node.js dependency, complex config | [semantic-release.gitbook.io](https://semantic-release.gitbook.io) |
| release-please | Google-maintained, simpler | Less flexible, GitHub-specific | [github.com/google-github-actions/release-please-action](https://github.com/google-github-actions/release-please-action) |
| cargo-release | Rust-native | Frontend not covered | [crates.io/crates/cargo-release](https://crates.io/crates/cargo-release) |
| Custom workflow | Full control | Maintenance burden | N/A |

**Discovery deliverable**: Decision document with chosen tool and rationale.

### Acceptance Test

```bash
# M6 Acceptance Test
git branch -a | grep -q "origin/latest" && \
test -f .releaserc.json -o -f release.config.js -o -f .release-please-manifest.json && \
test -f CHANGELOG.md && \
gh release list --limit 1 | grep -q . && \
echo "M6: PASS" || echo "M6: FAIL"
```

**Graduation Criteria**: Full promotion pipeline operational, v1.0.0-alpha released via automation.

---

## M7: Repository Infrastructure

**Status**: `BLOCKED`
**Owner**: @usrbinkat
**Target**: v1.0.0

### Deliverables

| ID | Deliverable | Status | Acceptance Test |
|----|-------------|--------|-----------------|
| M7.1 | Nix flake.nix | `BLOCKED` | `nix flake check` passes |
| M7.2 | Nix devshell | `BLOCKED` | `nix develop` provides dev environment |
| M7.3 | Nix package | `BLOCKED` | `nix build` produces working binary |
| M7.4 | Nix CI runner | `BLOCKED` | CI uses Nix for reproducible builds |
| M7.5 | git.braincraft.io mirror | `BLOCKED` | Push to GitHub mirrors to Braincraft |
| M7.6 | Multi-origin CI | `BLOCKED` | CI triggers from both origins |

### Blockers

| ID | Blocker | Discovery Plan | Owner |
|----|---------|----------------|-------|
| B-M7.1 | Nix flake not created | Create flake.nix with Tauri + Rust + Node | @usrbinkat |
| B-M7.4 | Nix CI unknown | **DISCOVERY**: Research cachix, garnix, or self-hosted | @usrbinkat |
| B-M7.5 | Braincraft repo not created | Create repo at git.braincraft.io | @usrbinkat |
| B-M7.6 | Multi-origin CI unknown | **DISCOVERY**: Research Forgejo/Gitea CI or webhook triggers | @usrbinkat |

### Discovery: Nix CI Runner

**Status**: `BLOCKED` - Discovery required

**Options to evaluate**:

| Option | Pros | Cons | Research Link |
|--------|------|------|---------------|
| Cachix | Binary caching, GitHub integration | Paid for private repos | [cachix.org](https://cachix.org) |
| Garnix | Free tier, Nix-native | Newer, less documentation | [garnix.io](https://garnix.io) |
| Self-hosted | Full control | Infrastructure overhead | N/A |
| GitHub Actions + Nix | Native integration | No binary cache without Cachix | N/A |

### Discovery: Multi-Origin Sync

**Status**: `BLOCKED` - Discovery required

**Options to evaluate**:

| Option | Pros | Cons |
|--------|------|------|
| Push mirror | Simple, GitHub-native | One-way only |
| Bidirectional sync | True multi-origin | Complex, conflict risk |
| Forgejo CI webhook | Independent CI | Separate configuration |

### Acceptance Test

```bash
# M7 Acceptance Test
test -f flake.nix && \
nix flake check && \
git remote -v | grep -q "braincraft" && \
echo "M7: PASS" || echo "M7: FAIL"
```

**Graduation Criteria**: Nix builds work, both origins receive pushes.

---

## Maintainer Surface Area

### Domain Ownership

| Domain | Owner | Backup | Status |
|--------|-------|--------|--------|
| **Rust Backend** | @usrbinkat | TBD | Active |
| **React Frontend** | @usrbinkat | TBD | Active |
| **Build/CI** | @usrbinkat | TBD | Active |
| **Release Automation** | @usrbinkat | TBD | Active |
| **Nix Infrastructure** | @usrbinkat | TBD | Active |
| **Documentation** | TBD | TBD | Needs Owner |
| **Community/Triage** | TBD | TBD | Needs Owner |
| **Security** | TBD | TBD | Needs Owner |
| **Platform: Linux** | @usrbinkat | TBD | Active |
| **Platform: macOS** | TBD | TBD | Needs Owner |
| **Platform: Windows** | TBD | TBD | Needs Owner |

### Ownership Responsibilities

Domain owners commit to:

1. **Response Time**: Acknowledge issues/PRs within SLA (see REVIEWERS.md)
2. **Quality**: Ensure domain meets project standards
3. **Documentation**: Keep domain docs current
4. **Mentorship**: Help new contributors in domain
5. **Escalation**: Flag capacity issues early

### Claiming a Domain

1. Demonstrate contribution history in domain
2. Review existing issues/PRs in domain
3. Propose ownership in an issue
4. Current owner (or maintainers) approve
5. Update this document via PR

---

## Process Commitments

### What We Promise Contributors

| Commitment | Measurement | Target |
|------------|-------------|--------|
| **PR Response Time** | Time to first review | Per REVIEWERS.md SLA |
| **Issue Triage** | Time to label/assign | Within 72 hours |
| **Release Cadence** | Time between releases | Continuous from `latest` |
| **Breaking Change Notice** | Advance warning | 2 weeks minimum |
| **Security Response** | Time to acknowledge | Within 24 hours |

### What We Ask of Contributors

See [CONTRIBUTORS.md](./CONTRIBUTORS.md):

- Follow coding standards
- **No PII in commits** (enforced)
- Respond to review feedback within 48 hours
- Keep PRs focused and reviewable
- Test changes before submission

---

## Capacity Planning

### Current Capacity

| Resource | Current | Sustainable | Gap |
|----------|---------|-------------|-----|
| Active Maintainers | 1 | 2-3 | -1 to -2 |
| Weekly Hours Available | ~10 | ~20 | -10 |
| Domains with Owners | 6/11 | 11/11 | -5 |

### Capacity Triggers

| Trigger | Action |
|---------|--------|
| SLA missed 3x consecutively | Reduce scope or recruit help |
| Domain without owner for 30 days | Mark domain as unsupported |
| >20 open issues in domain | Triage and close/defer |
| >5 PRs waiting review | Pause new features, clear backlog |

### Scaling Strategy

1. **Phase 1 (Current)**: Solo maintainer, limited scope
2. **Phase 2**: 2-3 maintainers, full v1 scope
3. **Phase 3**: Domain specialists, community reviewers
4. **Phase 4**: If scope exceeds capacity, split into sub-projects

---

## Scope Governance

### Scope Creep Indicators

| Indicator | Threshold | Action |
|------------|-----------|--------|
| Open issues | >50 | Triage and close/defer |
| Open PRs | >10 | Review sprint or close stale |
| Domains without owners | >3 | Defer features in those domains |
| Milestones overdue | >2 | Retrospective and scope reduction |

### Domain Splitting Criteria

If a domain becomes unmaintainable, consider splitting into separate repo:

1. **Size**: >30% of codebase
2. **Velocity**: >50% of commits
3. **Independence**: Can release independently
4. **Expertise**: Requires specialized knowledge

Potential split candidates for future:
- `spiritstream-core` (Rust backend)
- `spiritstream-ui` (React frontend)
- `spiritstream-plugins` (Extension system, v2+)

---

## Blocker Registry

All active blockers across milestones:

| ID | Milestone | Blocker | Type | Owner | Status |
|----|-----------|---------|------|-------|--------|
| B-M2.1 | M2 | No external contributors | Adoption | @usrbinkat | Open |
| B-M3.3 | M3 | macOS code signing | Discovery | TBD | Open |
| B-M3.4 | M3 | Windows code signing | Discovery | TBD | Open |
| B-M3.6 | M3 | Nix flake not created | Implementation | @usrbinkat | Open |
| B-M4.0 | M4 | No documentation owner | Ownership | @usrbinkat | Open |
| B-M5.4 | M5 | E2E test infrastructure | Discovery | TBD | Open |
| B-M5.6 | M5 | Security audit scope | Discovery | TBD | Open |
| B-M6.1 | M6 | Latest branch not created | Implementation | @usrbinkat | **Critical** |
| B-M6.2 | M6 | Auto-promotion config | Discovery | @usrbinkat | Open |
| B-M6.3 | M6 | Semantic release tooling | Discovery | @usrbinkat | **Critical** |
| B-M7.1 | M7 | Nix flake not created | Implementation | @usrbinkat | Open |
| B-M7.4 | M7 | Nix CI unknown | Discovery | @usrbinkat | Open |
| B-M7.5 | M7 | Braincraft repo | Implementation | @usrbinkat | Open |

### Critical Path

The following blockers are on the critical path to v1.0.0:

1. **B-M6.3**: Choose semantic release tooling (unblocks M6.2-M6.9)
2. **B-M6.1**: Create `latest` branch (unblocks governance operationalization)
3. **B-M7.1**: Create Nix flake (unblocks M7.2-M7.4)

### Blocker Resolution

Per [MILESTONES_CEREMONY.md](./MILESTONES_CEREMONY.md#blockers):

- **Resolved**: Impediment removed, section proceeds
- **Deferred**: Moved to future version, unblocks current cut
- **Accepted**: Risk acknowledged, proceed despite blocker
- **Rejected**: Section removed from scope

---

## Version Cut Readiness

### Current State

| Criterion | Status |
|-----------|--------|
| M1: Community Foundation | Partially complete |
| M2: Review Process | Not started |
| M3: Platform Distribution | In progress |
| M4: Documentation | Not started |
| M5: Quality Assurance | In progress |
| M6: Release Infrastructure | **BLOCKED** - Critical |
| M7: Repository Infrastructure | **BLOCKED** |

### V1.0.0-alpha Cut Criteria

- [ ] M6.1: `latest` branch exists
- [ ] M6.2: Auto-promotion operational
- [ ] M6.3: Semantic release configured
- [ ] All milestones have defined acceptance tests
- [ ] No `Critical` blockers remain

### V1.0.0-stable Cut Criteria

- [ ] All milestones reach `COMPLETE` status
- [ ] All acceptance tests pass
- [ ] 1 week soak on `latest` with no regressions
- [ ] Maintainer sign-off

---

## Document Changelog

| Date | Change | Author |
|------|--------|--------|
| 2026-01-18 | Initial draft | @usrbinkat |
| 2026-01-18 | Added M6, M7; TDD acceptance criteria; discovery blockers | @usrbinkat |
| 2026-01-18 | Added M1.11 (OpenCode Commit Tooling); validation outcome standards; fixed M1 acceptance test | @usrbinkat |

---

## Related Documents

| Document | Purpose |
|----------|---------|
| [GOVERNANCE.md](./GOVERNANCE.md) | Root governance specification (SSOT) |
| [MILESTONES_CEREMONY.md](./MILESTONES_CEREMONY.md) | How governance works |
| [MILESTONES.main.md](./MILESTONES.main.md) | Ideas under discussion |
| [CONTRIBUTORS.md](./CONTRIBUTORS.md) | How to contribute |
| [REVIEWERS.md](./REVIEWERS.md) | How to review |
| [roadmap.md](.claude/claudedocs/roadmap.md) | What to build |

---

## Signatories

For version cut approval:

| Maintainer | Date | Status |
|------------|------|--------|
| @usrbinkat | TBD | Pending |

---

*This is a living document. See [MILESTONES_CEREMONY.md](./MILESTONES_CEREMONY.md) for the governance process.*
