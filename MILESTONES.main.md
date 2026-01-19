# MILESTONES - Main

**Type**: Intake Funnel (Permanent)
**Authority**: Delegated from [GOVERNANCE.md](./GOVERNANCE.md)
**Created**: 2026-01-18
**Ceremony**: [MILESTONES_CEREMONY.md](./MILESTONES_CEREMONY.md)
**Branch**: `main`

---

## How This Document Works

This is the **intake funnel** for governance changes. Ideas, proposals, and scope changes accumulate here before graduating into [MILESTONES.latest.md](./MILESTONES.latest.md).

This document aligns with the `main` branch:
- Development happens here
- CI is **NOT** required to pass on every commit
- When CI passes, changes auto-promote to `latest` branch

For the complete governance process, see [MILESTONES_CEREMONY.md](./MILESTONES_CEREMONY.md).

### Lifecycle

```
MILESTONES.main.md              (Intake - always exists)
Branch: main (may break)
         │
         ▼ (accepted)
MILESTONES.latest.md            (Living commitments - evolves)
Branch: latest (must pass)
         │
         ▼ (version cut when agreement matures)
MILESTONES.v<X>.<Y>.<Z>-<stage>.md   (Frozen agreement record)
Branch: v<X>.<Y>.<Z>-<stage> (must pass, artifacts generated)
```

### Document-Branch Alignment

| Document | Branch | CI Requirement | Purpose |
|----------|--------|----------------|---------|
| `main.md` | `main` | May fail | Development intake, proposals |
| `latest.md` | `latest` | Must pass | Production-ready commitments |
| `v<X>-<stage>.md` | `v<X>-<stage>` | Must pass | Frozen release governance |

### Document States

| Document | Meaning | Mutability |
|----------|---------|------------|
| `main.md` | Ideas under discussion | Anyone via PR |
| `latest.md` | Accepted commitments | Maintainers |
| `v<X>-alpha.md` | Initial agreement frozen | Stage promotions only |
| `v<X>-beta.md` | Refined agreement | Minor changes only |
| `v<X>-stable.md` | Locked agreement | Frozen |
| `v<X>-patch.md` | Post-stable amendments | Append-only |

### Current State

| Document | Status |
|----------|--------|
| [MILESTONES.main.md](./MILESTONES.main.md) | This document (intake) |
| [MILESTONES.latest.md](./MILESTONES.latest.md) | Current commitments (pre-v1 cut) |
| MILESTONES.v1-*.md | Not yet created (no version cut) |

---

## Proposal Template

Copy this template to propose new governance items:

```markdown
### PROP-<NUMBER>: <Title>

**Proposed**: <Date>
**Author**: @<github-handle>
**Target Version**: v<X>
**Category**: Milestone | Process | Scope | Domain | Ceremony

#### Summary

<1-2 sentence description>

#### Motivation

<Why is this needed?>

#### Proposal

<Detailed proposal>

#### Acceptance Criteria

- [ ] Criterion 1
- [ ] Criterion 2

#### Discussion

<Link to issue or PR for discussion>
```

---

## Active Proposals

*No active proposals. Add yours above this line.*

---

## Graduated Proposals

Proposals that have been incorporated into versioned milestones:

| ID | Title | Target | Graduated To | Date |
|----|-------|--------|--------------|------|
| - | Initial governance framework | v1 | MILESTONES.latest.md | 2026-01-18 |
| - | Branch-governance alignment | v1 | MILESTONES.latest.md | 2026-01-18 |

---

## Rejected Proposals

Proposals explicitly rejected with rationale:

| ID | Title | Rationale | Date |
|----|-------|-----------|------|
| - | *None yet* | - | - |

---

## How to Propose Changes

1. **For small changes**: Edit this document directly via PR
2. **For significant changes**: Open an issue first for discussion
3. **For scope changes**: Use the Scope Change template in issues

### What Belongs Here

- New governance milestones
- Process improvements
- Scope boundary changes
- Domain ownership proposals
- SLA adjustments
- Document structure changes
- Ceremony amendments

### What Does NOT Belong Here

- Feature requests → [roadmap.md](.claude/claudedocs/roadmap.md)
- Bug reports → GitHub Issues
- Code changes → Pull Requests (to `main` branch)
- Questions → GitHub Discussions

### Development vs. Governance

Remember: This document tracks **governance** proposals, not implementation.

| Activity | Where |
|----------|-------|
| "I want to add feature X" | Feature request → roadmap.md |
| "I want X to be part of v1 scope" | Governance proposal → this document |
| "I wrote feature X" | PR → main branch |
| "Feature X should have a maintainer" | Governance proposal → this document |

---

## Related Documents

- [GOVERNANCE.md](./GOVERNANCE.md) - Root governance specification (SSOT)
- [MILESTONES.latest.md](./MILESTONES.latest.md) - Current governing document
- [CONTRIBUTORS.md](./CONTRIBUTORS.md) - Contributor expectations
- [REVIEWERS.md](./REVIEWERS.md) - Reviewer expectations
- [roadmap.md](.claude/claudedocs/roadmap.md) - Technical feature direction

---

*This document is permanent. Proposals graduate out; the intake process remains.*
