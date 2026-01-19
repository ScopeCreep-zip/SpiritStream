# SpiritStream Governance Specification

**Document Identifier**: GOVERNANCE.md
**Version**: 1.0.0-draft
**Status**: PROPOSED
**Authority Level**: ROOT
**Effective Date**: Upon acceptance
**Last Modified**: 2026-01-18

---

## Preface

### Abstract

This document establishes the governance framework for the SpiritStream project. It defines the authoritative structure, decision-making processes, stakeholder responsibilities, lifecycle management, and sustainability model that govern all project activities.

### Document Status

This document is the **Single Source of Truth (SSOT)** for the governance domain. All other governance-related documents derive their authority from this specification.

| Stage | Meaning | This Document |
|-------|---------|---------------|
| PROPOSED | Under review, not yet binding | Current |
| ACCEPTED | Binding upon all stakeholders | Target |
| AMENDED | Modified via governance process | Future |

### Normative Language

The key words "SHALL", "SHALL NOT", "REQUIRED", "SHOULD", "SHOULD NOT", "RECOMMENDED", "MAY", and "OPTIONAL" in this document are to be interpreted as described in [RFC 2119](https://www.ietf.org/rfc/rfc2119.txt).

| Keyword | Requirement Level |
|---------|-------------------|
| SHALL / REQUIRED / MUST | Absolute requirement |
| SHALL NOT / MUST NOT | Absolute prohibition |
| SHOULD / RECOMMENDED | Strong recommendation with valid exceptions |
| SHOULD NOT / NOT RECOMMENDED | Strong discouragement with valid exceptions |
| MAY / OPTIONAL | Truly optional |

### Change Authority

This document MAY be amended only through the governance process defined herein. Amendments SHALL follow the same lifecycle as any governance proposal.

---

## 1. Scope

### 1.1 Purpose

This specification:

1. **Establishes Authority** - Defines the authoritative hierarchy of project documents
2. **Defines Process** - Specifies how decisions are made and documented
3. **Assigns Responsibility** - Clarifies stakeholder roles and accountabilities
4. **Manages Lifecycle** - Governs features, versions, support, and end-of-life
5. **Ensures Sustainability** - Enables long-term project viability

### 1.2 Applicability

This specification applies to:

- All project repositories under the SpiritStream organization
- All contributors, maintainers, reviewers, and stakeholders
- All governance documents, processes, and ceremonies
- All versions, branches, and releases

### 1.3 Exclusions

This specification does NOT govern:

- Technical implementation decisions (see `roadmap.md`)
- Code style and formatting (see `.claude/rules/`)
- Third-party dependencies and their licenses
- Individual contributor conduct (see `CODE_OF_CONDUCT.md`)

### 1.4 Relationship to Other Standards

This specification is informed by:

- [Semantic Versioning 2.0.0](https://semver.org/)
- [Conventional Commits](https://www.conventionalcommits.org/)
- [RFC 2119](https://www.ietf.org/rfc/rfc2119.txt) - Requirement Levels
- [Contributor Covenant](https://www.contributor-covenant.org/)

---

## 2. Normative References

The following documents are indispensable for the application of this specification. For dated references, only the edition cited applies. For undated references, the latest edition applies.

### 2.1 Internal References

| Document | Authority | Domain |
|----------|-----------|--------|
| `GOVERNANCE.md` | ROOT | This document - governance SSOT |
| `MILESTONES_CEREMONY.md` | DELEGATED | Process execution |
| `MILESTONES.main.md` | DELEGATED | Proposal intake |
| `MILESTONES.latest.md` | DELEGATED | Current commitments |
| `MILESTONES.v*-*.md` | DELEGATED | Frozen agreements |
| `CONTRIBUTORS.md` | DELEGATED | Contribution process |
| `REVIEWERS.md` | DELEGATED | Review process |
| `CODE_OF_CONDUCT.md` | DELEGATED | Community standards |
| `roadmap.md` | INFORMATIVE | Technical direction |

### 2.2 Authority Hierarchy

```
┌─────────────────────────────────────────────────────────────────────┐
│                         GOVERNANCE.md                                │
│                    (ROOT - This Document)                            │
│                                                                      │
│  - Defines authority hierarchy                                       │
│  - Establishes governance principles                                 │
│  - Cannot be overridden by any other document                        │
│  - Amendments require explicit governance ceremony                   │
└─────────────────────────────────────────────────────────────────────┘
                                  │
                    ┌─────────────┼─────────────┐
                    │             │             │
                    ▼             ▼             ▼
         ┌──────────────┐ ┌──────────────┐ ┌──────────────┐
         │ MILESTONES   │ │ CONTRIBUTORS │ │ CODE_OF      │
         │ _CEREMONY.md │ │ .md          │ │ CONDUCT.md   │
         │              │ │              │ │              │
         │ DELEGATED    │ │ DELEGATED    │ │ DELEGATED    │
         │ Process      │ │ Contribution │ │ Conduct      │
         └──────────────┘ └──────────────┘ └──────────────┘
                │
        ┌───────┼───────┐
        │       │       │
        ▼       ▼       ▼
   ┌─────────┐ ┌─────────┐ ┌─────────┐
   │main.md  │ │latest.md│ │v*-*.md  │
   │         │ │         │ │         │
   │DELEGATED│ │DELEGATED│ │DELEGATED│
   │Proposals│ │Living   │ │Frozen   │
   └─────────┘ └─────────┘ └─────────┘
```

### 2.3 Conflict Resolution

In case of conflict between documents:

1. `GOVERNANCE.md` (this document) SHALL prevail over all others
2. `MILESTONES_CEREMONY.md` SHALL prevail over milestone documents
3. Frozen versions (`v*-*.md`) SHALL NOT be modified to resolve conflicts
4. Conflicts SHALL be documented and resolved in `MILESTONES.main.md`

---

## 3. Terms and Definitions

For the purposes of this specification, the following terms and definitions apply.

### 3.1 Governance Terms

**3.1.1 Governance**
The framework of rules, practices, and processes by which the project is directed and controlled.

**3.1.2 Single Source of Truth (SSOT)**
An authoritative document that exclusively owns a specific domain. Updates to that domain SHALL occur only in that document.

**3.1.3 Ceremony**
A defined process for making governance decisions, including required participants, inputs, and outputs.

**3.1.4 Blocker**
An explicit impediment preventing a governance item from advancing. Blockers concern AGREEMENT, not implementation.

**3.1.5 Version Cut**
The act of freezing a governance agreement into an immutable record, creating a versioned milestone document.

### 3.2 Document Terms

**3.2.1 Living Document**
A document that evolves continuously until frozen by a version cut.

**3.2.2 Frozen Document**
A document that SHALL NOT be modified except through stage promotion or patch amendments.

**3.2.3 Intake Funnel**
A permanent document that accumulates proposals before they graduate to living documents.

### 3.3 Stakeholder Terms

**3.3.1 Maintainer**
An individual with commit access who has accepted responsibility for one or more project domains.

**3.3.2 Reviewer**
An individual authorized to approve or request changes on pull requests.

**3.3.3 Contributor**
Any individual who submits changes, documentation, or feedback to the project.

**3.3.4 Sponsor**
An individual or organization providing financial or resource support to the project.

**3.3.5 User**
Any individual or organization that uses the project software.

### 3.4 Lifecycle Terms

**3.4.1 Feature Lifecycle**
The progression of a feature from proposal through implementation, release, maintenance, and retirement.

**3.4.2 Version Lifecycle**
The progression of a version through alpha, beta, stable, and patch stages.

**3.4.3 Support Lifecycle**
The commitment to maintain, patch, and support a version or feature over time.

**3.4.4 End-of-Life (EOL)**
The termination of support for a version or feature, after which no updates will be provided.

### 3.5 Branch Terms

**3.5.1 Development Branch (`main`)**
The primary branch where development occurs. CI MAY fail on individual commits.

**3.5.2 Production Branch (`latest`)**
The upstream branch containing production-ready code. CI SHALL pass on every commit.

**3.5.3 Release Branch (`v*-*`)**
A frozen branch representing a specific version release. CI SHALL pass on every commit.

---

## 4. Governance Model

### 4.1 Core Principles

The governance model is founded on four principles:

#### 4.1.1 Decoupled Development and Alignment

Development velocity and governance maturity are INDEPENDENT variables.

- Engineering MAY implement speculatively before governance formalizes
- Stakeholders MAY align on direction without blocking development
- Agreement MAY be frozen while implementation continues
- Multiple versions MAY be planned simultaneously

#### 4.1.2 Decoupled Development and Release

Release artifacts and governance commitment are INDEPENDENT.

- Users SHALL receive patches without waiting for governance approval
- Features MAY be fully developed and experienced before governance commits
- Maintainer/sponsor commitment MAY be the LAST blocker, not the first
- Communities MAY validate features before formal release commitment

#### 4.1.3 Eventual Consistency

The governance model follows eventual consistency principles:

- Proposals accumulate asynchronously
- Acceptance happens when alignment is reached
- Version cuts happen when agreement matures
- Implementation happens when resources permit

There is no REQUIRED ordering between these activities.

#### 4.1.4 Reversibility

All governance decisions MAY be revisited:

- Accepted items MAY be deferred
- Stable versions MAY be patched
- Rejected items MAY be re-proposed
- Mature features MAY be removed before governance commits

### 4.2 Separation of Concerns

Each document SHALL own its domain exclusively:

| Concern | SSOT Document | Owner |
|---------|---------------|-------|
| Governance authority | `GOVERNANCE.md` | Maintainers |
| Process execution | `MILESTONES_CEREMONY.md` | Maintainers |
| Current commitments | `MILESTONES.latest.md` | Maintainers |
| Proposals | `MILESTONES.main.md` | Community |
| Contribution process | `CONTRIBUTORS.md` | Maintainers |
| Review process | `REVIEWERS.md` | Reviewers |
| Technical direction | `roadmap.md` | Technical leads |

### 4.3 Explicit State

Every governance item SHALL have explicit state:

| State | Meaning | Transitions To |
|-------|---------|----------------|
| `PROPOSED` | Under discussion | ACCEPTED, DEFERRED, REJECTED |
| `ACCEPTED` | Committed with criteria | COMPLETE, BLOCKED |
| `BLOCKED` | Has unresolved impediment | ACCEPTED (when resolved) |
| `COMPLETE` | All criteria met | (terminal) |
| `DEFERRED` | Moved to future version | PROPOSED (in future) |
| `REJECTED` | Explicitly removed | PROPOSED (if re-proposed) |

State transitions SHALL be documented with rationale.

---

## 5. Branch-Document Alignment

### 5.1 Branch Model

The project SHALL maintain the following branch structure:

```
┌─────────────────────────────────────────────────────────────────────┐
│   main ─────────────────────────────────────────────────────────    │
│     │        Development intake                                      │
│     │        CI: NOT required to pass on every commit               │
│     │        Aligns with: MILESTONES.main.md                        │
│     │                                                                │
│     │ auto-promote (when CI passes)                                 │
│     ▼                                                                │
│   latest ───────────────────────────────────────────────────────    │
│     │        Production-ready upstream                              │
│     │        CI: MUST pass on every commit                          │
│     │        Aligns with: MILESTONES.latest.md                      │
│     │                                                                │
│     │ create release branch (when governance ACCEPTS)               │
│     ▼                                                                │
│   v<X>.<Y>.<Z>-<stage> ─────────────────────────────────────────    │
│              Frozen release branch                                   │
│              CI: MUST pass on every commit                          │
│              Aligns with: MILESTONES.v<X>.<Y>.<Z>-<stage>.md        │
└─────────────────────────────────────────────────────────────────────┘
```

### 5.2 Branch-Document Mapping

| Branch | Document | CI Requirement | Artifact Generation |
|--------|----------|----------------|---------------------|
| `main` | `MILESTONES.main.md` | MAY fail | None |
| `latest` | `MILESTONES.latest.md` | SHALL pass | Preview/Nightly |
| `v*-alpha` | `MILESTONES.v*-alpha.md` | SHALL pass | Alpha release |
| `v*-beta` | `MILESTONES.v*-beta.md` | SHALL pass | Beta release |
| `v*-stable` | `MILESTONES.v*-stable.md` | SHALL pass | Stable release |

### 5.3 Promotion Rules

#### 5.3.1 Automatic Promotion: main → latest

**Trigger**: All CI checks pass on `main`
**Action**: Automatic merge to `latest`
**Human Intervention**: None REQUIRED

This ensures users receive patches as soon as CI validates them.

#### 5.3.2 Governance Promotion: latest → release

**Trigger**: Governance ACCEPTS a version (cuts `MILESTONES.v*-<stage>.md`)
**Action**: Create branch `v*-<stage>` from `latest`
**Human Intervention**: REQUIRED (maintainer ceremony)

This ensures releases only occur when governance explicitly commits.

### 5.4 Rationale

This model enables:

1. **Downstream users don't wait** - Patches flow automatically
2. **Features mature before commitment** - Users experience via `latest`
3. **Maintainer is LAST blocker** - Not first
4. **Sponsorship model** - Features can be 100% ready, awaiting only commitment
5. **Reversibility** - Features can be removed before release

---

## 6. Stakeholder Framework

### 6.1 Role Definitions

#### 6.1.1 Maintainers

**Authority**: Highest within project
**Responsibilities**:
- Domain ownership and stewardship
- Governance ceremony execution
- Version cut decisions
- Blocker resolution
- Capacity management

**Accountability**:
- Response time SLAs (see `REVIEWERS.md`)
- Domain quality standards
- Mentorship of contributors

#### 6.1.2 Reviewers

**Authority**: Code review approval
**Responsibilities**:
- Technical review per `REVIEWERS.md`
- Quality gate enforcement
- Constructive feedback

**Accountability**:
- Review SLA compliance
- Blocking/non-blocking categorization
- Process adherence

#### 6.1.3 Contributors

**Authority**: Proposal submission
**Responsibilities**:
- Follow `CONTRIBUTORS.md`
- Respond to feedback
- Test changes

**Rights**:
- Timely review
- Constructive feedback
- Recognition

#### 6.1.4 Sponsors

**Authority**: Resource provision
**Responsibilities**:
- Financial or resource commitment
- Clear scope definition
- Timeline expectations

**Benefits**:
- Accelerated governance for sponsored features
- Recognition
- Influence on prioritization

#### 6.1.5 Users

**Authority**: Feedback provision
**Responsibilities**:
- Issue reporting
- Feature validation via `latest`

**Rights**:
- Access to `latest` artifacts
- Transparent roadmap
- Security patch access

### 6.2 Domain Ownership

Domains SHALL have explicit owners:

| Domain | SSOT | Ownership Requirement |
|--------|------|----------------------|
| Governance | `GOVERNANCE.md` | Maintainer consensus |
| Process | `MILESTONES_CEREMONY.md` | Maintainer |
| Commitments | `MILESTONES.latest.md` | Maintainer |
| Contribution | `CONTRIBUTORS.md` | Maintainer |
| Review | `REVIEWERS.md` | Reviewer lead |
| Technical | `roadmap.md` | Technical lead |

Ownership changes SHALL follow the claiming process in `MILESTONES.latest.md`.

### 6.3 Mutual Commitments

#### 6.3.1 Project Commits To Stakeholders

| Commitment | Measurement | Target |
|------------|-------------|--------|
| PR Response | Time to first review | Per `REVIEWERS.md` SLA |
| Issue Triage | Time to label/assign | 72 hours |
| Security Response | Time to acknowledge | 24 hours |
| Breaking Changes | Advance notice | 2 weeks minimum |

#### 6.3.2 Stakeholders Commit To Project

| Stakeholder | Commitment |
|-------------|------------|
| Contributors | Follow `CONTRIBUTORS.md` |
| Reviewers | Follow `REVIEWERS.md` |
| Maintainers | Meet SLAs, maintain domains |
| Sponsors | Honor commitments |

---

## 7. Lifecycle Management

### 7.1 Feature Lifecycle

```
┌─────────────────────────────────────────────────────────────────────┐
│                        FEATURE LIFECYCLE                             │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  PROPOSAL ──────> IMPLEMENTATION ──────> RELEASE ──────> MAINTENANCE │
│      │                  │                   │                  │     │
│      ▼                  ▼                   ▼                  ▼     │
│  main.md            main branch         v*-* branch        Patches   │
│  (governance)       (code)              (artifacts)        (support) │
│      │                  │                   │                  │     │
│      └──────────────────┴───────────────────┴──────────────────┘     │
│                           INDEPENDENT TIMELINES                       │
│                                                                      │
│                                   ▼                                  │
│                              END-OF-LIFE                             │
└─────────────────────────────────────────────────────────────────────┘
```

#### 7.1.1 Proposal Stage

- Feature proposed in `MILESTONES.main.md`
- Discussion and refinement
- Acceptance into `MILESTONES.latest.md`

#### 7.1.2 Implementation Stage

- Development in `main` branch
- CI validation
- Auto-promotion to `latest` on pass

#### 7.1.3 Release Stage

- Governance cuts version
- Release branch created
- Artifacts generated

#### 7.1.4 Maintenance Stage

- Bug fixes and security patches
- Backports to supported versions
- Documentation updates

#### 7.1.5 End-of-Life

- Announced in advance (see Section 7.4)
- Final security patch window
- Archive and deprecation

### 7.2 Version Lifecycle

```
┌─────────────────────────────────────────────────────────────────────┐
│                        VERSION LIFECYCLE                             │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  latest.md ────> v<X>-alpha ────> v<X>-beta ────> v<X>-stable       │
│      │               │               │               │               │
│      ▼               ▼               ▼               ▼               │
│   Living         Initial          Refined          Locked            │
│   commitments    agreement        agreement        agreement         │
│                                                                      │
│                      Mutability decreases ────────────────────>      │
│                                                                      │
│                                                        │             │
│                                                        ▼             │
│                                                   v<X>-patch         │
│                                                   Append-only        │
└─────────────────────────────────────────────────────────────────────┘
```

#### 7.2.1 Alpha Stage

- Initial agreement formalized
- Open to significant changes
- Early adopter testing

#### 7.2.2 Beta Stage

- Agreement refined from feedback
- Open to minor changes only
- Wider testing

#### 7.2.3 Stable Stage

- Agreement locked
- No changes except patches
- General availability

#### 7.2.4 Patch Stage

- Append-only amendments
- Critical fixes only
- Maintains stability

### 7.3 Support Lifecycle

#### 7.3.1 Support Tiers

| Tier | Duration | Coverage |
|------|----------|----------|
| Active | Current + 1 prior | All fixes |
| Security | Current + 2 prior | Security only |
| Extended | Sponsored | Per agreement |
| EOL | None | No support |

#### 7.3.2 Support Matrix

```
Version Timeline:
  v1.0 ──────> v1.1 ──────> v2.0 ──────> v2.1 ──────> v3.0
               │             │             │             │
               ▼             ▼             ▼             ▼
            EOL when      EOL when      EOL when      Current
            v2.0+2       v3.0+2        v3.0 stable

Support at v3.0 release:
  - v3.0: Active (all fixes)
  - v2.1: Active (all fixes)
  - v2.0: Security (security only)
  - v1.x: EOL (no support)
```

### 7.4 End-of-Life (EOL)

#### 7.4.1 EOL Announcement

- SHALL be announced minimum 3 months before EOL
- SHALL be documented in release notes
- SHOULD include migration guidance

#### 7.4.2 EOL Grace Period

- 30 days after EOL for final security patches
- Critical vulnerabilities only
- No new features or non-security fixes

#### 7.4.3 EOL Extension

EOL MAY be extended via sponsorship (see Section 8.3).

---

## 8. Sustainability Model

### 8.1 Principles

The project SHALL maintain sustainability through:

1. **Bounded Scope** - Capacity limits scope, not inverse
2. **Explicit Capacity** - Published in `MILESTONES.latest.md`
3. **Sponsor Enablement** - Clear path for resource provision
4. **Recognition** - Contributors and sponsors acknowledged

### 8.2 Sponsorship Model

#### 8.2.1 Sponsorship Types

| Type | Scope | Benefit |
|------|-------|---------|
| Feature | Specific feature | Accelerated governance |
| Maintenance | Ongoing support | Extended EOL |
| Security | Security patches | Priority response |
| General | Project health | Recognition, influence |

#### 8.2.2 Sponsorship Process

1. **Proposal**: Sponsor contacts maintainers
2. **Scoping**: Mutual agreement on deliverables
3. **Commitment**: Formalized in governance
4. **Delivery**: Per agreed timeline
5. **Recognition**: Per sponsor preferences

#### 8.2.3 Sponsored Features

Sponsorship enables the "maintainer as final blocker" pattern:

```
Feature State:
  ✓ Fully implemented in latest
  ✓ CI passes
  ✓ Users validated via preview artifacts
  ✓ Community enthusiasm high
  ⊘ BLOCKER: No maintainer committed

Resolution via sponsorship:
  1. Sponsor commits resources
  2. Maintainer accepts commitment
  3. Blocker resolved
  4. Version cut proceeds
```

### 8.3 Extended Support

#### 8.3.1 Extended Support Eligibility

Versions MAY receive extended support when:

- Sponsor commits to maintenance costs
- Maintainer capacity available
- Security patch feasibility confirmed

#### 8.3.2 Extended Support Terms

| Term | Requirement |
|------|-------------|
| Duration | Minimum 6 months |
| Scope | Security patches only |
| Response | Per standard SLA |
| Cost | Sponsor-maintainer agreement |

### 8.4 Bounty Program

#### 8.4.1 Bounty Eligibility

Features MAY be bounty-eligible when:

- Feature is fully specified in `MILESTONES.latest.md`
- Feature is blocked only on maintainer commitment
- Bounty is sufficient to fund maintenance

#### 8.4.2 Bounty Process

1. **Feature identified** as bounty-eligible
2. **Bounty posted** by sponsor or community
3. **Maintainer claims** bounty and commitment
4. **Feature graduates** to release
5. **Bounty disbursed** per agreement

---

## 9. Governance Evolution

### 9.1 Self-Amendment

This specification (`GOVERNANCE.md`) is itself governed by the governance process:

1. **Proposals** via `MILESTONES.main.md` with category "Governance"
2. **Acceptance** requires maintainer consensus
3. **Versioning** follows semantic versioning
4. **Backward Compatibility** with existing milestone documents

### 9.2 Amendment Process

```
┌─────────────────────────────────────────────────────────────────────┐
│                      AMENDMENT PROCESS                               │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  1. PROPOSAL                                                         │
│     └── Add to MILESTONES.main.md with category "Governance"        │
│                                                                      │
│  2. DISCUSSION                                                       │
│     └── Open period for stakeholder feedback                        │
│                                                                      │
│  3. CONSENSUS                                                        │
│     └── Maintainer consensus required                                │
│     └── No dissenting maintainer after discussion period            │
│                                                                      │
│  4. ACCEPTANCE                                                       │
│     └── Move to MILESTONES.latest.md                                │
│                                                                      │
│  5. INTEGRATION                                                      │
│     └── Update GOVERNANCE.md                                        │
│     └── Increment version                                           │
│                                                                      │
│  6. ANNOUNCEMENT                                                     │
│     └── Notify stakeholders                                         │
└─────────────────────────────────────────────────────────────────────┘
```

### 9.3 Amendment Types

| Type | Version Impact | Approval |
|------|----------------|----------|
| Clarification | Patch | Single maintainer |
| Minor change | Minor | Maintainer majority |
| Major change | Major | Maintainer consensus |
| Principle change | Major | All maintainers + community input |

### 9.4 Compatibility

Amendments SHALL:

- NOT invalidate existing frozen milestone documents
- NOT retroactively change historical decisions
- Provide transition guidance when needed
- Maintain document relationship integrity

---

## 10. Compliance and Auditing

### 10.1 Governance Health Indicators

#### 10.1.1 Healthy Signs

- Proposals flow through main → latest regularly
- Blockers have owners and progress
- Version cuts occur when agreement matures
- Documents stay synchronized
- Branches align with documents
- Artifacts enable early validation
- Sponsors/maintainers emerge from community

#### 10.1.2 Warning Signs

- Proposals stagnate in `main.md`
- Blockers without owners or progress
- `latest.md` grows unboundedly
- No version cuts despite mature agreement
- Documents contradict each other
- `latest` branch diverges from `latest.md`
- No users validating `latest` artifacts

### 10.2 Audit Process

#### 10.2.1 Regular Audits

| Audit | Frequency | Owner |
|-------|-----------|-------|
| Document sync | Monthly | Maintainers |
| Branch alignment | Per release | CI/CD |
| Blocker review | Weekly | Maintainers |
| Capacity check | Quarterly | Maintainers |

#### 10.2.2 Audit Checklist

- [ ] All documents reference correct versions
- [ ] No conflicting statements between documents
- [ ] All blockers have owners
- [ ] All domains have owners
- [ ] Branch CI status matches requirements
- [ ] Artifacts generated for appropriate branches

### 10.3 Corrective Actions

| Finding | Action |
|---------|--------|
| Stagnant proposals | Schedule proposal review ceremony |
| Stuck blockers | Escalate or defer |
| Unbounded `latest.md` | Cut version or prune scope |
| No version cuts | Lower bar for alpha cut |
| Document drift | Reconciliation audit |
| Branch drift | Align branches with documents |
| No validation | Promote `latest` artifacts |

---

## 11. Quick Reference

### 11.1 Document Purposes

| Document | One-Line Purpose |
|----------|------------------|
| `GOVERNANCE.md` | Governance SSOT - you are here |
| `MILESTONES_CEREMONY.md` | How governance process works |
| `MILESTONES.main.md` | Ideas under discussion |
| `MILESTONES.latest.md` | Current commitments |
| `MILESTONES.v*-*.md` | Frozen agreement records |
| `CONTRIBUTORS.md` | How to contribute |
| `REVIEWERS.md` | How to review |
| `CODE_OF_CONDUCT.md` | Community standards |
| `roadmap.md` | Technical direction |

### 11.2 Key Principles

1. Agreement and implementation are DECOUPLED
2. Implementation and release are DECOUPLED
3. Version cuts are about AGREEMENT maturity
4. Blockers are about ALIGNMENT, not implementation
5. Documents reference PATTERNS, not specific versions
6. Branches ALIGN with governance documents
7. Every state is EXPLICIT
8. Maintainer/sponsor commitment MAY be final blocker

### 11.3 Common Operations

| I want to... | Do this... |
|--------------|------------|
| Propose a governance change | Add to `MILESTONES.main.md` |
| See current commitments | Read `MILESTONES.latest.md` |
| Understand the process | Read `MILESTONES_CEREMONY.md` |
| Contribute code | Read `CONTRIBUTORS.md` |
| Review code | Read `REVIEWERS.md` |
| Get early access | Use `latest` branch artifacts |
| Get stable release | Use release branch artifacts |
| Sponsor a feature | Contact maintainers |
| Extend support | Contact maintainers |

### 11.4 Normative Keywords Summary

| Keyword | Meaning |
|---------|---------|
| SHALL | Absolute requirement |
| SHALL NOT | Absolute prohibition |
| SHOULD | Recommendation |
| SHOULD NOT | Discouragement |
| MAY | Optional |

---

## Annexes

### Annex A: Document Templates

See `MILESTONES.main.md` for proposal template.

### Annex B: Ceremony Checklists

See `MILESTONES_CEREMONY.md` for ceremony procedures.

### Annex C: Technical Checklists

See `REVIEWERS.md` for review checklists.

### Annex D: Change Log

| Version | Date | Change | Author |
|---------|------|--------|--------|
| 1.0.0-draft | 2026-01-18 | Initial specification | @usrbinkat |

---

## Approval

This specification requires acceptance per Section 9.2.

| Role | Name | Date | Status |
|------|------|------|--------|
| Maintainer | @usrbinkat | TBD | Pending |

---

*This is the root governance document. All governance authority derives from this specification.*
