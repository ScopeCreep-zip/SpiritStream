# Reviewer Guidelines

**Authority**: Delegated from [GOVERNANCE.md](./GOVERNANCE.md)
**Domain**: Review Process

This document defines the code review process for SpiritStream. It serves as the complement to [CONTRIBUTORS.md](./CONTRIBUTORS.md), forming a mutual agreement between contributors and reviewers.

## Table of Contents

- [Review Philosophy](#review-philosophy)
- [Reviewer Responsibilities](#reviewer-responsibilities)
- [Review Process](#review-process)
- [Technical Checklist](#technical-checklist)
- [Providing Feedback](#providing-feedback)
- [Submitting Reviews](#submitting-reviews)
- [Special Cases](#special-cases)
- [Becoming a Reviewer](#becoming-a-reviewer)

## Review Philosophy

### Principles

1. **Assume good intent** - Contributors are trying to improve the project
2. **Be constructive** - Explain *why*, not just *what*
3. **Be thorough** - Review the entire change, not just the obvious parts
4. **Be timely** - Respect contributors' time with prompt reviews
5. **Be skeptical** - Verify claims; check edge cases; question assumptions

### The Reviewer's Role

Reviewers are **gatekeepers of quality**, not gatekeepers of the project. The goal is to:

- Help contributors succeed
- Maintain code quality and consistency
- Catch bugs before they reach users
- Share knowledge across the team
- Protect security and stability

### The Mutual Agreement

| Contributors Commit To | Reviewers Commit To |
|------------------------|---------------------|
| Following contribution guidelines | Reviewing within SLA timeframes |
| Responding to feedback promptly | Providing actionable, constructive feedback |
| Testing changes before submission | Explaining the reasoning behind requests |
| Keeping PRs focused and reviewable | Distinguishing blocking vs. non-blocking issues |
| Accepting valid criticism gracefully | Accepting valid pushback gracefully |

## Reviewer Responsibilities

### Response Time SLA

| PR Size | Lines Changed | Target Response |
|---------|---------------|-----------------|
| Small | < 100 | Within 24 hours |
| Medium | 100-500 | Within 48 hours |
| Large | 500+ | Within 72 hours |

Response means either:
- A complete review with feedback
- A comment acknowledging the PR and setting expectations

### What Reviewers Own

- **Quality** - Ensuring code meets project standards
- **Correctness** - Verifying the change works as intended
- **Security** - Catching vulnerabilities before merge
- **Consistency** - Maintaining architectural coherence
- **Documentation** - Ensuring changes are properly documented

### What Reviewers Do NOT Own

- **Perfection** - Minor style issues shouldn't block merges
- **Rewriting** - Suggesting alternatives, not dictating solutions
- **Timeline** - Contributors choose when to address feedback

## Review Process

### Step 1: Understand the Change

Before reviewing code, understand what it's trying to accomplish:

```bash
# Get PR metadata (use --repo for cross-repo reviews)
gh pr view <PR_NUMBER> --repo <OWNER>/<REPO> \
  --json number,title,body,headRefName,baseRefName,headRefOid,state,files,additions,deletions

# Example output tells you:
# - headRefOid: The commit SHA to review
# - files: List of changed files with additions/deletions
```

Read the PR description thoroughly. Check linked issues for context.

### Step 2: Get the Full Diff

Always review the **complete** diff. Truncating blinds you to issues.

```bash
# Get complete diff - DO NOT pipe through head/tail
gh pr diff <PR_NUMBER> --repo <OWNER>/<REPO>
```

**Important:** Always review the FULL diff. Never truncate with `head` or `tail` as this blinds you to issues.

### Step 3: Fetch Source Files

The diff shows changes, but you need full file context for accurate line numbers and surrounding code:

```bash
# Get the head SHA and fork info
gh api repos/OWNER/REPO/pulls/<PR_NUMBER> \
  --jq '.head.repo.full_name, .head.sha'

# Fetch specific files from the contributor's fork
gh api repos/<FORK_OWNER>/<REPO>/contents/<FILE_PATH>?ref=<COMMIT_SHA> \
  --jq '.content' | base64 -d
```

**Important:** PRs from forks require fetching from the fork repository, not the base repository.

### Step 4: Analyze Each File

For each changed file, use the appropriate checklist below.

### Step 5: Categorize Issues

Sort findings by severity to help contributors prioritize:

| Category | Criteria | Merge Blocked? |
|----------|----------|----------------|
| **Blocking** | Bugs, security issues, breaks build, data corruption | Yes |
| **Recommended** | Missing validation, inefficient code, edge cases | No |
| **Minor** | Style, naming, redundant code, stale comments | No |

### Step 6: Write the Review

Structure your review clearly:

```markdown
## Summary
Overall assessment (1-2 sentences)

## Blocking Issues
1. **Issue title** (file:line) - Description

## Recommended Improvements
1. **Issue title** (file:line) - Description

## Minor
1. **Issue title** (file:line) - Description
```

### Step 7: Submit with Suggestions

Use GitHub's suggestion syntax so contributors can accept with one click:

````markdown
**Blocking:** Description of the issue and why it matters.

```suggestion
corrected code here
```
````

## Technical Checklist

### Frontend (TypeScript/React)

| Check | What to Look For |
|-------|------------------|
| **Type Safety** | Explicit types, no `any`, proper null handling |
| **API Consistency** | Function names match interfaces, invoke strings match commands |
| **Imports** | All imports used, no circular dependencies |
| **React Patterns** | Proper keys, hooks rules, event cleanup |
| **Error Handling** | Errors caught and surfaced to users |
| **i18n** | User-facing strings use translation keys |

### Backend (Rust)

| Check | What to Look For |
|-------|------------------|
| **Input Validation** | User inputs validated before processing |
| **Error Handling** | Results properly propagated, meaningful error messages |
| **Side Effects** | Read operations don't write; writes are intentional |
| **Async Correctness** | async/await used appropriately, no blocking in async |
| **Command Registration** | New commands added to `lib.rs` |
| **Security** | No path traversal, injection, or credential exposure |

### Cross-Layer

| Check | What to Look For |
|-------|------------------|
| **API Contract** | Frontend invoke strings match Rust command names exactly |
| **Serialization** | Types serialize/deserialize correctly across IPC |
| **Naming** | Consistent naming across frontend and backend |
| **Feature Flags** | Incomplete features properly gated |

### Security

| Check | What to Look For |
|-------|------------------|
| **Secrets** | No hardcoded credentials, keys, or tokens |
| **Path Traversal** | User input not used directly in file paths |
| **Injection** | No command injection, SQL injection, XSS |
| **Logging** | Sensitive data masked in logs |
| **Permissions** | Principle of least privilege |

## Providing Feedback

### Constructive Feedback Format

**Instead of:**
> This is wrong.

**Write:**
> This will cause a runtime error because the Rust command is named `ensure_order_indexes` but this calls `insure_order_indexes`. The invoke string must match exactly.

### Feedback Templates

#### Typo in API Call
```markdown
**Blocking:** Typo in command name - `{wrong}` should be `{correct}`.
The Rust command is `{rust_name}`, and invoke strings must match exactly.

```suggestion
    {corrected_code}
```
```

#### Missing Type Definition
```markdown
**Blocking:** Missing `{method_name}` in interface. The function is
exported in the implementation but not defined in the type interface.

```suggestion
  {existing_method}: () => Result<Type>;
  {missing_method}: () => Result<Type>;
```
```

#### Missing Input Validation
```markdown
**Recommended:** Add validation to ensure inputs exist before persisting.
This prevents invalid data from being written if the frontend sends bad data.

```suggestion
pub async fn command_name(
    inputs: Vec<String>,
    manager: State<'_, Manager>,
) -> Result<(), String> {
    let existing = manager.get_all().await?;
    for input in &inputs {
        if !existing.contains(input) {
            return Err(format!("Unknown item: {}", input));
        }
    }
    // ... rest of function
```
```

#### Side Effect on Read Path
```markdown
**Recommended:** This causes a write operation on every read. Consider:
1. Using the read-only variant here
2. Moving the write to app initialization

```suggestion
        let data = self.read_data()?;
```
```

### Language Guidelines

| Do | Don't |
|----|-------|
| "This could cause..." | "This is wrong" |
| "Consider using..." | "You should use..." |
| "I notice that..." | "You forgot to..." |
| "What do you think about..." | "Change this to..." |

## Submitting Reviews

### Via GitHub UI

1. Go to the PR's "Files changed" tab
2. Add comments inline
3. Click "Review changes"
4. Select: Approve / Request changes / Comment
5. Submit

### Via GitHub API

For detailed reviews with multiple inline comments:

```bash
# Create review JSON
cat << 'EOF' > /tmp/review.json
{
  "commit_id": "<HEAD_SHA>",
  "event": "REQUEST_CHANGES",
  "body": "Review summary...",
  "comments": [
    {
      "path": "path/to/file.ts",
      "line": 26,
      "body": "**Blocking:** Issue description\n\n```suggestion\nfixed code\n```"
    }
  ]
}
EOF

# Submit review
gh api repos/OWNER/REPO/pulls/<PR_NUMBER>/reviews \
  --input /tmp/review.json
```

**Important Notes:**
- `commit_id` must be current - re-fetch before submitting
- `event` options: `APPROVE`, `REQUEST_CHANGES`, `COMMENT`
- Cannot use `APPROVE` on your own PRs
- Suggestion blocks must match the exact line content

### Review Events

| Event | When to Use |
|-------|-------------|
| `APPROVE` | No blocking issues, ready to merge |
| `REQUEST_CHANGES` | Blocking issues that must be fixed |
| `COMMENT` | Feedback without formal approval/rejection |

## Special Cases

### Large PRs (500+ lines)

1. Consider asking the contributor to split the PR
2. If not splittable, review in sections over multiple sessions
3. Focus on architecture first, then details
4. Document your review progress in comments

### First-Time Contributors

1. Be extra welcoming and encouraging
2. Explain project conventions that may not be obvious
3. Offer to pair or answer questions
4. Thank them for contributing

### Security-Sensitive Changes

1. Take extra time - security reviews shouldn't be rushed
2. Consider requesting a second reviewer
3. Check for OWASP Top 10 vulnerabilities
4. Verify secrets aren't exposed in logs or errors

### Breaking Changes

1. Verify the change is intentional and discussed
2. Check migration path for existing users
3. Ensure changelog/documentation is updated
4. Consider version implications

### Self-Reviews

When reviewing your own code (for practice or documentation):
- Use `COMMENT` event (can't approve your own PR)
- Be as critical as you would be of others' code
- Document findings for your own reference

## Becoming a Reviewer

### Path to Reviewer Status

1. **Contributor** - Make quality contributions to the project
2. **Consistent** - Demonstrate sustained engagement over time
3. **Knowledgeable** - Show understanding of the codebase and standards
4. **Communicative** - Interact constructively with others
5. **Invited** - Maintainers will invite active contributors to review

### Reviewer Expectations

Once a reviewer, you commit to:
- Meeting response time SLAs
- Following this review process
- Providing constructive feedback
- Staying current with project standards
- Mentoring new contributors

### Reviewer Recognition

Active reviewers are:
- Listed in project documentation
- Recognized in release notes
- Given write access as appropriate
- Invited to maintainer discussions

## Governance

Reviewer processes are governed by the milestone ceremony:

| Document | Purpose |
|----------|---------|
| [GOVERNANCE.md](./GOVERNANCE.md) | Root governance specification (SSOT) |
| [MILESTONES_CEREMONY.md](./MILESTONES_CEREMONY.md) | How governance works |
| [MILESTONES.main.md](./MILESTONES.main.md) | Propose process changes |
| [MILESTONES.latest.md](./MILESTONES.latest.md) | Current commitments |

To propose changes to the review process, add to [MILESTONES.main.md](./MILESTONES.main.md).

---

## Quick Reference

### Review Checklist

- [ ] Read PR description and linked issues
- [ ] Fetched full diff (not truncated)
- [ ] Fetched source files with correct commit SHA
- [ ] Checked all files against technical checklist
- [ ] Categorized issues (Blocking/Recommended/Minor)
- [ ] Used suggestion blocks for easy fixes
- [ ] Review is constructive and actionable

### Commands

```bash
# Get PR info (include --repo for cross-repo)
gh pr view <N> --repo OWNER/REPO --json title,body,headRefOid,files

# Get full diff (never truncate!)
gh pr diff <N> --repo OWNER/REPO

# Get fork info and commit SHA
gh api repos/OWNER/REPO/pulls/<N> --jq '.head.repo.full_name, .head.sha'

# Fetch file from contributor's fork
gh api repos/FORK/REPO/contents/PATH?ref=SHA --jq '.content' | base64 -d

# Submit review via API
gh api repos/OWNER/REPO/pulls/<N>/reviews --input review.json
```

### Validation Commands

After the contributor addresses feedback, verify the fixes:

```bash
# TypeScript (run pnpm install if dependencies changed)
pnpm install
pnpm run typecheck

# Rust
cargo check --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml

# Rust (with OpenSSL workaround for nix/direnv environments)
DIRENV_DIR= \
OPENSSL_DIR=/usr \
OPENSSL_LIB_DIR=/usr/lib/x86_64-linux-gnu \
OPENSSL_INCLUDE_DIR=/usr/include \
PATH=$HOME/.cargo/bin:/usr/local/bin:/usr/bin:/bin \
cargo check --manifest-path src-tauri/Cargo.toml

# Full build
pnpm run build
```

### Common Pitfalls

Watch for these frequently-missed issues:

1. **API name mismatches** - Frontend function name vs. backend command name
2. **Missing interface methods** - Function exported but not in type interface
3. **Typos in invoke strings** - JavaScript string literals don't get type-checked against Rust
4. **Read path side effects** - Functions that look like reads but write data
5. **Missing cleanup** - Delete operations that leave orphaned data
6. **Redundant React keys** - Nested components with duplicate keys
7. **Unused imports** - Imports added during development but no longer used
8. **Stale commit SHA** - Always re-fetch before submitting API reviews

---

*This document complements [CONTRIBUTORS.md](./CONTRIBUTORS.md). Together with [GOVERNANCE.md](./GOVERNANCE.md) and [MILESTONES_CEREMONY.md](./MILESTONES_CEREMONY.md), they form the governance framework for SpiritStream.*

