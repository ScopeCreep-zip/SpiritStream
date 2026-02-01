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
main ───────────> latest ───────────> v1.0.0-alpha
(may break)       (must pass)         (frozen release)
                       │
                       ▼
              Users can experience
              features via artifacts
              BEFORE governance commits
```

For the governance process, see [MILESTONES_CEREMONY.md](./MILESTONES_CEREMONY.md).
For proposals under discussion, see [MILESTONES.main.md](./MILESTONES.main.md).

This document establishes:

1. **Maintainer Surface Area** - Who owns what domains
2. **Scope Boundaries** - What's in/out of scope for v1
3. **Process Commitments** - What we promise to contributors
4. **Measurable Accountability** - How we track ourselves

This is **not** a feature roadmap. For technical features, see [roadmap.md](.claude/claudedocs/roadmap.md).

---

## Item Statuses

Per [MILESTONES_CEREMONY.md](./MILESTONES_CEREMONY.md), items in this document have explicit status:

| Status | Meaning |
|--------|---------|
| `PROPOSED` | Under discussion, not yet committed |
| `ACCEPTED` | Committed with measurable criteria |
| `BLOCKED` | Accepted but has unresolved blocker |
| `COMPLETE` | All acceptance criteria met |
| `DEFERRED` | Moved to future version |
| `REJECTED` | Explicitly removed from scope |

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

---

## Scope Definition

### In Scope for v1

| Domain | Description |
|--------|-------------|
| **Core Streaming** | RTMP relay, output groups, stream targets |
| **Profile Management** | Encrypted profiles, import/export |
| **Platform Support** | Linux (.deb, .rpm), macOS, Windows |
| **Governance** | CONTRIBUTORS.md, REVIEWERS.md, this document |
| **Documentation** | User guide, troubleshooting, API docs |
| **Testing** | Platform validation, load testing |

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

## Governance Milestones

### M1: Community Foundation

**Status**: `PROPOSED`
**Owner**: @usrbinkat
**Target**: v1.0 release

| ID | Deliverable | Status | Acceptance Criteria |
|----|-------------|--------|---------------------|
| M1.1 | CONTRIBUTORS.md | `PROPOSED` | Covers: setup, standards, PR process, expectations |
| M1.2 | REVIEWERS.md | `PROPOSED` | Covers: process, checklists, templates, SLAs |
| M1.3 | MILESTONES.v1.md | `PROPOSED` | This document accepted by maintainers |
| M1.4 | CODE_OF_CONDUCT.md | `PROPOSED` | Contributor Covenant adopted |
| M1.5 | Issue Templates | `PROPOSED` | Bug report, feature request, security report |
| M1.6 | PR Template | `PROPOSED` | Summary, changes, test plan, checklist |

**Graduation Criteria**: All deliverables reach `ACCEPTED` status.

---

### M2: Review Process Operational

**Status**: `PROPOSED`
**Owner**: TBD
**Target**: v1.0 release

| ID | Deliverable | Status | Acceptance Criteria |
|----|-------------|--------|---------------------|
| M2.1 | Review SLA Tracking | `PROPOSED` | Mechanism to measure response times |
| M2.2 | First External PR Reviewed | `PROPOSED` | Non-maintainer PR reviewed per REVIEWERS.md |
| M2.3 | Review Process Retrospective | `PROPOSED` | Document learnings, update REVIEWERS.md |

**Graduation Criteria**: 3 external PRs reviewed following documented process.

---

### M3: Platform Distribution

**Status**: `PROPOSED`
**Owner**: @usrbinkat
**Target**: v1.0 release

| ID | Deliverable | Status | Acceptance Criteria |
|----|-------------|--------|---------------------|
| M3.1 | Linux .deb Package | `PROPOSED` | Installs and runs on Ubuntu 22.04+ |
| M3.2 | Linux .rpm Package | `PROPOSED` | Installs and runs on Fedora 38+ |
| M3.3 | macOS .dmg | `PROPOSED` | Installs and runs on macOS 13+ |
| M3.4 | Windows .msi | `PROPOSED` | Installs and runs on Windows 10+ |
| M3.5 | Nix Flake | `PROPOSED` | `nix run github:ScopeCreep-zip/SpiritStream` works |
| M3.6 | CI/CD Pipeline | `PROPOSED` | Automated builds for all platforms |

**Graduation Criteria**: All packages install and basic streaming works.

---

### M4: Documentation Complete

**Status**: `PROPOSED`
**Owner**: TBD
**Target**: v1.0 release

| ID | Deliverable | Status | Acceptance Criteria |
|----|-------------|--------|---------------------|
| M4.1 | User Guide | `PROPOSED` | First-time user can stream within 10 minutes |
| M4.2 | Troubleshooting Guide | `PROPOSED` | Top 10 issues documented with solutions |
| M4.3 | Architecture Overview | `PROPOSED` | Diagram + explanation for contributors |
| M4.4 | API Documentation | `PROPOSED` | All Tauri commands documented |

**Graduation Criteria**: Documentation reviewed by someone unfamiliar with project.

---

### M5: Quality Assurance

**Status**: `PROPOSED`
**Owner**: TBD
**Target**: v1.0 release

| ID | Deliverable | Status | Acceptance Criteria |
|----|-------------|--------|---------------------|
| M5.1 | Platform Testing Matrix | `PROPOSED` | All platforms tested with documented results |
| M5.2 | Load Testing | `PROPOSED` | 5+ simultaneous streams stable for 1 hour |
| M5.3 | Security Audit | `PROPOSED` | No critical/high vulnerabilities |
| M5.4 | Performance Baseline | `PROPOSED` | Memory/CPU benchmarks documented |

**Graduation Criteria**: All tests pass, results documented.

---

## Maintainer Surface Area

### Domain Ownership

| Domain | Owner | Backup | Status |
|--------|-------|--------|--------|
| **Rust Backend** | @usrbinkat | TBD | Active |
| **React Frontend** | @usrbinkat | TBD | Active |
| **Build/CI** | @usrbinkat | TBD | Active |
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
5. Update this document

---

## Process Commitments

### What We Promise Contributors

| Commitment | Measurement | Target |
|------------|-------------|--------|
| **PR Response Time** | Time to first review | Per REVIEWERS.md SLA |
| **Issue Triage** | Time to label/assign | Within 72 hours |
| **Release Cadence** | Time between releases | Monthly for v1.x |
| **Breaking Change Notice** | Advance warning | 2 weeks minimum |
| **Security Response** | Time to acknowledge | Within 24 hours |

### What We Ask of Contributors

See [CONTRIBUTORS.md](./CONTRIBUTORS.md) for full details:

- Follow coding standards
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
| Domains with Owners | 4/9 | 9/9 | -5 |

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

Active blockers preventing sections from reaching `ACCEPTED` or version cuts:

| ID | Section | Blocker | Owner | Status |
|----|---------|---------|-------|--------|
| B1 | M3.4 | No macOS maintainer confirmed | @usrbinkat | Open |
| B2 | M3.5 | No Windows maintainer confirmed | @usrbinkat | Open |
| B3 | M4 | Documentation owner TBD | TBD | Open |
| B4 | M5.3 | Security audit scope undefined | TBD | Open |

### Blocker Resolution

Per [MILESTONES_CEREMONY.md](./MILESTONES_CEREMONY.md#blockers):

- **Resolved**: Impediment removed, section proceeds
- **Deferred**: Moved to future version, unblocks current cut
- **Accepted**: Risk acknowledged, proceed despite blocker
- **Rejected**: Section removed from scope

---

## Version Cut Readiness

### Current State

This document (`MILESTONES.latest.md`) will be frozen as `MILESTONES.v1-alpha.md` when:

- [ ] All `PROPOSED` items reach `ACCEPTED` or `DEFERRED`
- [ ] All blockers resolved, deferred, or risk-accepted
- [ ] Maintainers sign off on scope
- [ ] Capacity assessment validated

### Pending Actions for v1-alpha Cut

1. Resolve or defer platform maintainer blockers (B1, B2)
2. Assign documentation owner (B3)
3. Define security audit scope (B4)
4. Maintainer sign-off

---

## Document Changelog

| Date | Change | Author |
|------|--------|--------|
| 2026-01-18 | Initial draft as latest.md | @usrbinkat |

---

## Related Documents

Per [MILESTONES_CEREMONY.md](./MILESTONES_CEREMONY.md#document-relationships):

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
