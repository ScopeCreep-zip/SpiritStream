# Milestone Governance Ceremony

**Authority**: Delegated from [GOVERNANCE.md](./GOVERNANCE.md)
**Domain**: Process Execution

This document defines the governance workflow for SpiritStream milestones. It establishes how stakeholders align on direction, how commitments are formalized, and how development proceeds independently of governance maturity.

For the root governance specification, see [GOVERNANCE.md](./GOVERNANCE.md).

---

## Table of Contents

- [Philosophy](#philosophy)
- [Core Principles](#core-principles)
- [Branch-Governance Alignment](#branch-governance-alignment)
- [Document Types](#document-types)
- [Lifecycle Flow](#lifecycle-flow)
- [CI/CD Integration](#cicd-integration)
- [Version Cuts](#version-cuts)
- [Release Artifacts](#release-artifacts)
- [Blockers](#blockers)
- [Stakeholder Roles](#stakeholder-roles)
- [Ceremonies](#ceremonies)
- [Document Relationships](#document-relationships)
- [Examples](#examples)

---

## Philosophy

### Decoupled Development and Alignment

Development velocity and governance maturity are **independent variables**. This design enables:

- Engineering can implement speculatively before governance formalizes
- Stakeholders can align on direction without blocking development
- Agreement can be frozen while implementation continues
- Multiple versions can be planned simultaneously

### Decoupled Development and Release

Release artifacts and governance commitment are **independent**. This enables:

- Users receive patches without waiting for governance approval
- Features can be fully developed and experienced before governance commits
- Maintainer/sponsor commitment can be the LAST blocker, not the first
- Communities can validate features before formal release commitment

### Eventual Consistency

The governance model follows eventual consistency principles:

- Proposals accumulate asynchronously
- Acceptance happens when alignment is reached
- Version cuts happen when agreement matures
- Implementation happens when resources permit

There is no required ordering between these activities.

### Agreement vs. Implementation vs. Release

**Version cuts represent agreement maturity, not implementation completion or artifact release.**

| State | Agreement | Implementation | Release Artifacts |
|-------|-----------|----------------|-------------------|
| Pre-alpha | Discussing | 0-100% possible | Available from `latest` |
| Alpha | Initial agreement | 0-100% possible | Available from `latest` |
| Beta | Refined agreement | 0-100% possible | Available from `latest` |
| Stable | Locked agreement | 0-100% possible | Available from release branch |
| Patch | Amended agreement | 0-100% possible | Available from release branch |

You may:
- Freeze v1-alpha before writing any code
- Implement 80% of `latest.md` before any version cut
- Ship artifacts from `latest` before governance stabilizes
- Stabilize governance before software is ready
- Remove mature features before governance commits to them

---

## Core Principles

### 1. Separation of Concerns

| Concern | Document | Owner |
|---------|----------|-------|
| What to build | `roadmap.md` | Technical leads |
| How to govern | `MILESTONES_*.md` | Maintainers |
| How to contribute | `CONTRIBUTORS.md` | Community |
| How to review | `REVIEWERS.md` | Reviewers |

### 2. Single Source of Truth

Each document owns its domain exclusively:

- No duplication across documents
- Cross-references instead of copies
- Updates happen in one place

### 3. Explicit State

Every commitment has explicit state:

- Proposed, Accepted, Blocked, Deferred, Rejected
- No implicit assumptions
- State transitions are documented

### 4. Reversibility

Decisions can be revisited:

- Accepted items can be deferred
- Stable versions can be patched
- Rejected items can be re-proposed
- Mature features can be removed before governance commits

---

## Branch-Governance Alignment

### Branch Model

```
┌─────────────────────────────────────────────────────────────────────┐
│                              BRANCHES                                │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
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
│   v1.0.0-alpha ─────────────────────────────────────────────────    │
│     │        Frozen release branch                                   │
│     │        CI: MUST pass on every commit                          │
│     │        Aligns with: MILESTONES.v1.0.0-alpha.md                │
│     │        Artifacts: Generated on branch creation                 │
│     │                                                                │
│     │ stage promotion (alpha → beta → stable → patch)               │
│     ▼                                                                │
│   v1.0.0-stable ────────────────────────────────────────────────    │
│              Long-term support branch                                │
│              Aligns with: MILESTONES.v1.0.0-stable.md               │
│                                                                      │
└─────────────────────────────────────────────────────────────────────┘
```

### Branch-Document Mapping

| Branch | Milestone Document | CI Requirement | Purpose |
|--------|-------------------|----------------|---------|
| `main` | `MILESTONES.main.md` | May fail | Development intake, proposals |
| `latest` | `MILESTONES.latest.md` | Must pass | Production-ready upstream |
| `v<X>.<Y>.<Z>-<stage>` | `MILESTONES.v<X>.<Y>.<Z>-<stage>.md` | Must pass | Frozen governance release |

### Promotion Rules

#### main → latest

```
Trigger: All CI checks pass on main
Action:  Auto-merge to latest
Result:  Production-ready code available upstream
```

This happens **automatically** without human intervention.

#### latest → release branch

```
Trigger: Governance ACCEPTS a version (cuts MILESTONES.v<X>-<stage>.md)
Action:  Create branch v<X>.<Y>.<Z>-<stage> from latest
Result:  Release artifacts generated, frozen governance state
```

This happens **only when governance explicitly accepts**.

### Why This Matters

1. **Downstream users don't wait for governance**
   - Patches flow: `main` → `latest` → artifacts
   - Users get fixes as soon as CI passes, not when maintainers meet

2. **Features can mature before commitment**
   - Full feature in `latest`, experienced by users
   - Governance can still reject before formal release
   - Maintainer can be the LAST requirement, not first

3. **Insiders/enthusiasts get early access**
   - `latest` branch is always production-ready
   - Early adopters validate before governance commits

4. **Sponsorship/bounty model enabled**
   - Feature is fully working in `latest`
   - Community drums up support
   - Sponsor commitment is final blocker
   - Feature graduates to release when sponsored

---

## Document Types

### MILESTONES.main.md

**Purpose**: Intake funnel for governance proposals. Aligns with `main` branch.

**Characteristics**:
- Always exists
- Accumulates asynchronously
- No approval required to propose
- Items graduate out when accepted
- May contain speculative/experimental proposals

**Contents**:
- Proposal template
- Active proposals under discussion
- Graduated proposals (historical)
- Rejected proposals (with rationale)

**Branch alignment**: `main` - development that may break

### MILESTONES.latest.md

**Purpose**: Current accepted commitments being worked toward. Aligns with `latest` branch.

**Characteristics**:
- Single living document
- Contains accepted but unfrozen commitments
- May have blockers on specific sections
- Evolves until version cut

**Contents**:
- Scope definition (in/out)
- Governance milestones
- Maintainer surface area
- Process commitments
- Capacity planning
- Blocker registry

**Branch alignment**: `latest` - production-ready, always green

### MILESTONES.v\<X\>.\<Y\>.\<Z\>-\<stage\>.md

**Purpose**: Frozen agreement record for a specific version.

**Naming**: `MILESTONES.v<major>.<minor>.<patch>-<stage>.md`

**Stages**:

| Stage | Meaning | Mutability |
|-------|---------|------------|
| `alpha` | Initial agreement formalized | Open to significant changes |
| `beta` | Agreement refined after feedback | Open to minor changes |
| `stable` | Agreement locked | Frozen except patches |
| `patch` | Post-stable amendments | Append-only changes |

**Characteristics**:
- Created when agreement matures (not when implementation completes)
- Represents what was agreed, not what was built
- Immutable after creation (except stage promotions)
- Historical record for future reference

**Branch alignment**: `v<X>.<Y>.<Z>-<stage>` - frozen release branch

---

## Lifecycle Flow

```
┌─────────────────────────────────────────────────────────────────────┐
│                          MILESTONES.main.md                          │
│                         (Intake - Always Open)                       │
│                       Branch: main (may break)                       │
│                                                                      │
│   [PROP-1] ──┬──> Accepted ──> Moves to latest.md                   │
│              ├──> Deferred ──> Stays with rationale                 │
│              └──> Rejected ──> Archived with rationale              │
└─────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────┐
│                        MILESTONES.latest.md                          │
│                    (Accepted Commitments - Living)                   │
│                     Branch: latest (must pass)                       │
│                                                                      │
│   [M1] Milestone 1 ───────────────────────────────> ✓ Ready         │
│   [M2] Milestone 2 ──── BLOCKER: Needs maintainer ─> ⊘ Blocked      │
│   [M3] Milestone 3 ───────────────────────────────> ✓ Ready         │
│                                                                      │
│   Features in latest can be:                                         │
│   - Fully implemented (100%)                                         │
│   - Experienced by users via latest branch                          │
│   - REMOVED before governance commits (reversible)                   │
│                                                                      │
│   When all blockers resolved for version scope:                      │
│                         │                                            │
│                         ▼                                            │
│                   VERSION CUT                                        │
└─────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────┐
│                   MILESTONES.v1.0.0-alpha.md                         │
│                    (Frozen Agreement Record)                         │
│                  Branch: v1.0.0-alpha (must pass)                    │
│                                                                      │
│   Snapshot of latest.md at time of agreement                        │
│   Immutable record of what was committed                            │
│   Release artifacts generated on branch creation                     │
│                                                                      │
│   Stage promotions: alpha ──> beta ──> stable ──> patch             │
└─────────────────────────────────────────────────────────────────────┘
```

### Parallel Timelines

Development, governance, and releases proceed independently:

```
Governance Timeline:
  main.md ─────────> latest.md ─────────> v1-alpha ────> v1-stable
      │                 │                     │              │
      ▼                 ▼                     ▼              ▼
   (ideas)         (accepted)            (agreed)       (locked)

Development Timeline:
  exploration ─────> prototype ─────> production ──────> mature
      │                 │                 │                │
      ▼                 ▼                 ▼                ▼
   (spike)          (working)         (stable)         (proven)

Release Timeline:
  none ────────────> latest ──────────> v1.0.0-alpha ──> v1.0.0-stable
      │                 │                     │              │
      ▼                 ▼                     ▼              ▼
   (no artifacts)  (early access)       (release)       (LTS)

These timelines are INDEPENDENT. Any combination is valid:
  - v1-stable governance with exploratory software
  - v1-alpha governance with mature software
  - No version cut with multiple releases from latest
  - Multiple version cuts with no software changes
```

---

## CI/CD Integration

### Branch CI Requirements

| Branch | CI on Commit | Auto-Merge Target | Artifacts |
|--------|--------------|-------------------|-----------|
| `main` | Runs, may fail | → `latest` (on pass) | None |
| `latest` | Must pass | None (upstream) | Early access |
| `v*-*` | Must pass | None (frozen) | Release |

### Promotion Automation

```yaml
# Conceptual CI/CD Flow

on:
  push:
    branches: [main]

jobs:
  ci:
    # Run full CI suite
    steps:
      - test
      - lint
      - build

  promote-to-latest:
    needs: ci
    if: success()
    steps:
      - merge main → latest
      - # latest is now updated

on:
  create:
    branches:
      - 'v*-alpha'
      - 'v*-beta'
      - 'v*-stable'

jobs:
  release:
    steps:
      - build artifacts
      - publish release
```

### Artifact Generation Rules

| Trigger | Artifacts | Distribution |
|---------|-----------|--------------|
| Merge to `latest` | Nightly/preview | Early adopters |
| Create `v*-alpha` | Alpha release | Testers |
| Create `v*-beta` | Beta release | Wider testing |
| Create `v*-stable` | Stable release | General availability |
| Tag `v*-patch` | Patch release | Hotfix distribution |

---

## Version Cuts

### When to Cut a Version

A version cut occurs when:

1. **Scope is defined** - What's in and out is clear
2. **Blockers are resolved** - No blocking items remain
3. **Stakeholders align** - Maintainers agree on commitments
4. **Capacity is realistic** - Commitments are achievable

A version cut does NOT require:

- Any implementation progress
- Software release
- External validation
- Timeline commitments

### Cut Process

1. **Proposal**: Maintainer proposes version cut in `latest.md`
2. **Review**: Stakeholders review scope and commitments
3. **Blocker Resolution**: All blockers must be resolved or deferred
4. **Snapshot**: Create `MILESTONES.v<X>-alpha.md` from `latest.md`
5. **Branch**: Create `v<X>.<Y>.<Z>-alpha` branch from `latest`
6. **Artifacts**: CI automatically generates release artifacts
7. **Reset**: Update `latest.md` for next version cycle

### Stage Promotions

```
v<X>-alpha ──> v<X>-beta ──> v<X>-stable ──> v<X>-patch
```

| Transition | Trigger | Changes Allowed |
|------------|---------|-----------------|
| alpha → beta | Community feedback incorporated | Scope refinements |
| beta → stable | Agreement locked | None (freeze) |
| stable → patch | Amendment needed | Append-only additions |

---

## Release Artifacts

### Artifact Types by Branch

| Branch Type | Artifact Type | Stability | Audience |
|-------------|---------------|-----------|----------|
| `latest` | Preview/Nightly | May change | Early adopters, insiders |
| `v*-alpha` | Alpha | Unstable | Testers, enthusiasts |
| `v*-beta` | Beta | Mostly stable | Wider community |
| `v*-stable` | Release | Stable | General users |
| `v*-patch` | Patch | Stable | Existing users |

### Globbing Rules for Artifacts

```
branches:
  latest:
    artifacts: ["*.AppImage", "*.deb", "*.rpm", "*.dmg", "*.msi"]
    channel: "preview"
    retention: "30 days"

  v*-alpha:
    artifacts: ["*.AppImage", "*.deb", "*.rpm", "*.dmg", "*.msi"]
    channel: "alpha"
    retention: "90 days"

  v*-beta:
    artifacts: ["*.AppImage", "*.deb", "*.rpm", "*.dmg", "*.msi"]
    channel: "beta"
    retention: "1 year"

  v*-stable:
    artifacts: ["*.AppImage", "*.deb", "*.rpm", "*.dmg", "*.msi"]
    channel: "stable"
    retention: "indefinite"
```

### Why Artifacts from `latest`?

Producing artifacts from `latest` before governance cuts a version enables:

1. **Immediate patches** - Users don't wait for governance meetings
2. **Feature validation** - Real users test before commitment
3. **Reversibility** - Features can be removed before release
4. **Community momentum** - Builds excitement and support
5. **Sponsor discovery** - Maintainers emerge from users

---

## Blockers

### What is a Blocker?

A blocker is an explicit impediment preventing a section from being accepted or a version from being cut.

**Blockers are about AGREEMENT, not IMPLEMENTATION.**

Examples:
- "Cannot accept M3 until macOS maintainer confirmed"
- "Scope unclear - needs RFC"
- "Dependency on external decision"
- "Stakeholder unavailable for review"
- "No sponsor for long-term maintenance"

NOT blockers:
- "Code not written yet" (implementation, not agreement)
- "Tests not passing" (implementation, not agreement)
- "Feature not released" (implementation, not agreement)

### Maintainer/Sponsor as Final Blocker

A powerful pattern enabled by this model:

```
Feature State:
  ✓ Fully implemented in latest
  ✓ CI passes
  ✓ Users validated via preview artifacts
  ✓ Community enthusiasm high
  ⊘ BLOCKER: No maintainer committed

Resolution paths:
  a) Maintainer steps up → version cut → release
  b) Sponsor funds maintainer → version cut → release
  c) Community bounty → maintainer → version cut → release
  d) Defer to future version → stays in latest
  e) Remove from latest → experimental branch
```

This ensures:
- Features don't rot waiting for governance
- Users experience features early
- Commitment comes with resources attached
- Maintainership is valued, not assumed

### Blocker Registry

In `MILESTONES.latest.md`, blockers are tracked explicitly:

```markdown
## Blocker Registry

| ID | Section | Blocker | Owner | Status |
|----|---------|---------|-------|--------|
| B1 | M2.3 | Needs macOS maintainer | @usrbinkat | Open |
| B2 | M4 | Waiting on security audit scope | TBD | Open |
| B3 | M5.1 | No sponsor for feature X | Community | Open |
```

### Blocker Resolution

| Resolution | Meaning |
|------------|---------|
| Resolved | Impediment removed, section can proceed |
| Deferred | Moved to future version, unblocks current cut |
| Accepted | Risk accepted, proceed despite blocker |
| Rejected | Section removed from scope |

---

## Stakeholder Roles

### Individual Contributors (ICs)

**Interests**: Clear expectations, achievable scope, recognition

**Interactions**:
- Propose ideas via `MILESTONES.main.md`
- Provide feedback on `latest.md`
- Implement against agreed milestones
- Not blocked by governance delays
- Can use `latest` artifacts immediately

### Maintainers

**Interests**: Sustainable scope, clear ownership, capacity management

**Interactions**:
- Curate `main.md` → `latest.md` transitions
- Own domain areas in `latest.md`
- Initiate version cuts
- Manage blocker resolution
- Commit to long-term support

### Reviewers

**Interests**: Clear standards, manageable workload, quality outcomes

**Interactions**:
- Defined in `REVIEWERS.md`
- Review PRs against agreed milestones
- Flag scope creep
- Not responsible for governance decisions

### Users/Community

**Interests**: Predictable releases, clear roadmap, voice in direction

**Interactions**:
- Use `latest` artifacts for early access
- Provide feedback during beta stages
- Drum up support for features they want released
- Not required for internal governance decisions

### Sponsors/Patrons

**Interests**: Features they need, recognition, influence

**Interactions**:
- Fund maintainer time
- Unblock features via sponsorship
- Commission bounties
- Accelerate governance for sponsored features

### Governance (Meta)

**Interests**: Process health, document consistency, ceremony adherence

**Interactions**:
- This document (`MILESTONES_CEREMONY.md`)
- Ensures ceremony is followed
- Audits document relationships
- Proposes process improvements

---

## Ceremonies

### Proposal Review (Weekly/Async)

**Purpose**: Triage `main.md` items

**Participants**: Maintainers

**Outputs**:
- Items accepted → move to `latest.md`
- Items deferred → stay with rationale
- Items rejected → archive with rationale

### Blocker Review (Weekly/Async)

**Purpose**: Track blocker resolution progress

**Participants**: Maintainers, affected owners

**Outputs**:
- Updated blocker status
- Escalation if stuck
- Scope adjustments if needed

### Version Cut Review (Milestone)

**Purpose**: Decide readiness for version cut

**Participants**: All maintainers

**Outputs**:
- Go/no-go decision
- Blocker resolution plan
- Scope finalization
- Branch creation approval

### Stage Promotion Review (Milestone)

**Purpose**: Promote version through stages

**Participants**: Maintainers

**Outputs**:
- Promotion decision (alpha→beta→stable)
- Feedback incorporation
- Patch amendments if needed

### Retrospective (Post-Release)

**Purpose**: Evaluate governance effectiveness

**Participants**: All stakeholders

**Outputs**:
- Process improvements → `main.md`
- Document updates
- Ceremony adjustments

---

## Document Relationships

```
┌─────────────────────────────────────────────────────────────────────┐
│                      MILESTONES_CEREMONY.md                          │
│                    (This Document - Process Bible)                   │
│                                                                      │
│  Defines how all milestone documents interact                        │
│  Defines branch-governance alignment                                 │
└─────────────────────────────────────────────────────────────────────┘
                                    │
            ┌───────────────────────┼───────────────────────┐
            │                       │                       │
            ▼                       ▼                       ▼
┌───────────────────┐   ┌───────────────────┐   ┌───────────────────┐
│ MILESTONES        │   │ MILESTONES        │   │ MILESTONES        │
│ .main.md          │──▶│ .latest.md        │──▶│ .v<X>-<stage>.md  │
│                   │   │                   │   │                   │
│ Intake funnel     │   │ Living commitments│   │ Frozen records    │
│ Branch: main      │   │ Branch: latest    │   │ Branch: v<X>-*    │
└───────────────────┘   └───────────────────┘   └───────────────────┘
                                    │
                    ┌───────────────┼───────────────┐
                    │               │               │
                    ▼               ▼               ▼
            ┌─────────────┐ ┌─────────────┐ ┌─────────────┐
            │CONTRIBUTORS │ │ REVIEWERS   │ │ roadmap.md  │
            │.md          │ │ .md         │ │             │
            │             │ │             │ │             │
            │How to       │ │How to       │ │What to      │
            │contribute   │ │review       │ │build        │
            └─────────────┘ └─────────────┘ └─────────────┘

Cross-Reference Rules:
  - All documents reference MILESTONES_CEREMONY.md for process
  - CONTRIBUTORS.md and REVIEWERS.md reference each other
  - MILESTONES.latest.md references roadmap.md for technical scope
  - Versioned milestones are immutable historical records
```

### Reference Pattern

Documents should reference the PATTERN, not specific versions:

**Do**:
```markdown
See [MILESTONES_CEREMONY.md](./MILESTONES_CEREMONY.md) for governance process.
See [MILESTONES.latest.md](./MILESTONES.latest.md) for current commitments.
See [MILESTONES.main.md](./MILESTONES.main.md) to propose changes.
```

**Don't**:
```markdown
See MILESTONES.v1-alpha.md for commitments.  <!-- Hardcodes version -->
```

---

## Examples

### Example 1: Pre-Implementation Agreement

**Scenario**: Team wants to agree on v2 scope before building anything.

```
1. Proposals accumulate in main.md
2. Maintainers accept proposals into latest.md
3. All blockers resolved
4. Cut MILESTONES.v2.0.0-alpha.md
5. Create branch v2.0.0-alpha from latest
6. CI generates alpha artifacts
7. Implementation begins (0% complete)
8. Implementation completes (100%)
9. Promote to v2.0.0-stable
```

Agreement happened at step 4. Implementation happened later.

### Example 2: Post-Implementation Governance

**Scenario**: Team built features, now wants to formalize governance.

```
1. Software fully implemented in latest branch
2. Users experiencing features via latest artifacts
3. Governance still at main.md (no version cuts)
4. Team decides to formalize
5. Retrospectively document what was built
6. Cut MILESTONES.v1.0.0-stable.md
7. Create branch v1.0.0-stable from latest
```

Implementation happened first. Agreement formalized later.

### Example 3: Insider Feature Testing

**Scenario**: New experimental feature needs validation before commitment.

```
1. Feature implemented in main
2. CI passes, auto-promotes to latest
3. Users get latest artifacts with new feature
4. Community provides feedback
5. Feature refined based on feedback
6. Feature now mature in latest
7. Governance STILL hasn't committed
8. Options:
   a) Cut version with feature (commit)
   b) Remove feature from latest (retreat)
   c) Keep in latest, defer governance (experiment longer)
```

Users experienced feature. Governance decided later.

### Example 4: Sponsor-Blocked Release

**Scenario**: Feature is ready but needs maintainer commitment.

```
MILESTONES.latest.md:
  [M1] Community Foundation ──────────────> ✓ Ready
  [M2] Review Process ────────────────────> ✓ Ready
  [M3] Advanced Feature X
        ├── Implementation: 100% (in latest)
        ├── Users: Validated via latest artifacts
        └── BLOCKER: No maintainer committed

Resolution timeline:
  Month 1: Feature in latest, users love it
  Month 2: Community drums up support
  Month 3: Sponsor offers bounty for maintainer
  Month 4: Maintainer accepts, blocker resolved
  Month 5: Cut version, feature officially released
```

Feature was ready for 5 months. Governance waited for commitment.

### Example 5: Feature Removal Before Commitment

**Scenario**: Experimental feature proves problematic.

```
1. Feature X implemented in latest
2. Users experience issues via latest artifacts
3. Community feedback negative
4. Governance has NOT committed to feature
5. Decision: Remove feature from latest
6. Feature removed, latest still valid
7. No version cut disrupted
8. No release promises broken
```

Feature was fully implemented, then removed. No governance promises broken.

---

## Governance Health Indicators

### Healthy Signs

- Proposals flow through main → latest regularly
- Blockers have owners and progress
- Version cuts happen when agreement matures
- Documents stay synchronized
- Branches stay synchronized with documents
- Latest artifacts enable early validation
- Sponsors/maintainers emerge from user community

### Warning Signs

- Proposals stagnate in main.md
- Blockers without owners or progress
- latest.md grows unboundedly
- No version cuts despite mature agreement
- Documents contradict each other
- latest branch diverges from latest.md
- No users validating latest artifacts

### Corrective Actions

| Symptom | Action |
|---------|--------|
| Stagnant proposals | Schedule proposal review ceremony |
| Stuck blockers | Escalate or defer |
| Unbounded latest | Cut version or prune scope |
| No version cuts | Lower bar for alpha cut |
| Document drift | Reconciliation audit |
| Branch drift | Align branches with documents |
| No validation | Promote latest artifacts more visibly |

---

## Amendments to This Document

This document (`MILESTONES_CEREMONY.md`) is itself governed:

1. **Proposals**: Via `MILESTONES.main.md` with category "Ceremony"
2. **Acceptance**: Requires maintainer consensus
3. **Versioning**: Major changes create new ceremony version
4. **Compatibility**: Existing milestone documents remain valid

---

## Quick Reference

### Document-Branch Mapping

| Document | Branch | CI | Purpose |
|----------|--------|----|---------
| `MILESTONES.main.md` | `main` | May fail | Ideas under discussion |
| `MILESTONES.latest.md` | `latest` | Must pass | Current commitments (living) |
| `MILESTONES.v<X>-<stage>.md` | `v<X>-<stage>` | Must pass | Frozen agreement records |

### Document Purposes

| Document | One-Line Purpose |
|----------|------------------|
| `MILESTONES_CEREMONY.md` | How governance works |
| `MILESTONES.main.md` | Ideas under discussion (aligns with main branch) |
| `MILESTONES.latest.md` | Current commitments (aligns with latest branch) |
| `MILESTONES.v<X>-<stage>.md` | Frozen agreement records (aligns with release branches) |
| `CONTRIBUTORS.md` | How to contribute |
| `REVIEWERS.md` | How to review |
| `roadmap.md` | What to build |

### Key Principles

1. Agreement and implementation are decoupled
2. Implementation and release are decoupled
3. Version cuts are about agreement maturity
4. Blockers are about alignment, not implementation
5. Documents reference patterns, not specific versions
6. Branches align with governance documents
7. Every state is explicit
8. Maintainer/sponsor commitment can be final blocker

### Common Operations

| I want to... | Do this... |
|--------------|------------|
| Propose an idea | Add to `MILESTONES.main.md` |
| See current commitments | Read `MILESTONES.latest.md` |
| Check version history | Read `MILESTONES.v<X>-*.md` files |
| Get early access artifacts | Use `latest` branch artifacts |
| Get stable artifacts | Use release branch artifacts |
| Understand the process | Read this document |
| Contribute code | Read `CONTRIBUTORS.md` |
| Review code | Read `REVIEWERS.md` |

---

*This document is the process authority for milestone governance, delegated from [GOVERNANCE.md](./GOVERNANCE.md). All milestone documents follow this ceremony.*
