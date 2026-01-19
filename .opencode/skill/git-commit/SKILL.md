---
name: git-commit
description: Create conventional commits following conventionalcommits.org v1.0.0 specification
---

## Conventional Commits Specification (v1.0.0)

This skill implements the [Conventional Commits](https://conventionalcommits.org/en/v1.0.0/)
specification for machine-parseable, SemVer-compatible commit messages.

### Commit Message Structure

```
<type>[optional scope][optional !]: <description>

[optional body]

[optional footer(s)]
```

## CRITICAL MANDATES - NON-NEGOTIABLE

These rules are absolute. Violation is unacceptable under any circumstances.

### 1. MUST READ FULL DIFF BEFORE EVERY COMMIT

**YOU MUST NEVER COMMIT A SINGLE LINE OF CODE THAT YOU DID NOT READ.**

Before EVERY `git commit`, you MUST:
1. Run `git --no-pager diff --staged` (or `git -C <path> --no-pager diff --staged`)
2. Read the COMPLETE output - every single line
3. Only then execute the commit

### 2. NEVER TRUNCATE OR BYPASS DIFF OUTPUT

**ABSOLUTELY FORBIDDEN:**
```bash
# NEVER use these patterns - they hide code you're committing
git diff --staged | head           # FORBIDDEN - truncates output
git diff --staged | tail           # FORBIDDEN - truncates output
git diff --staged > file.txt       # FORBIDDEN - hides output
git diff --staged 2>&1 | head      # FORBIDDEN - truncates output
git diff --stat                    # INSUFFICIENT - shows only filenames
git diff --staged --stat           # INSUFFICIENT - shows only filenames
```

**ALWAYS use:**
```bash
git --no-pager diff --staged       # Shows FULL diff, no paging
```

If a diff is too large to read, the commit is TOO LARGE. Split it.

### 3. KEEP COMMITS SMALL ENOUGH TO READ

If you cannot read the full `git --no-pager diff --staged` output, the commit
scope is too large. You MUST:

1. Unstage files: `git reset HEAD <file>`
2. Stage a smaller logical group
3. Commit that group
4. Repeat until all changes are committed

**Maximum commit size guidance:**
- Aim for diffs under 500 lines when possible
- Schema/generated files may be larger but MUST still be read
- If a single file diff is >1000 lines, consider if the change can be split

### 4. NEVER COMMIT FILES THAT DON'T BELONG

Before staging ANY file, verify it belongs in git history.

**NEVER COMMIT (unless explicitly instructed):**
| Pattern | Reason |
|---------|--------|
| `.env`, `.env.*` | Contains secrets |
| `*.secret`, `*-secret.*` | Contains secrets |
| `credentials.json`, `*credentials*` | Contains secrets |
| `*.pem`, `*.key`, `*.p12` | Private keys |
| `kubeconfig`, `*kubeconfig*` | Cluster credentials |
| `token`, `*token*`, `*.token` | Auth tokens |
| `password`, `*password*` | Passwords |
| `node_modules/`, `vendor/` | Dependencies (use lockfiles) |
| `*.log`, `logs/` | Runtime logs |
| `*.pyc`, `__pycache__/` | Python bytecode |
| `.DS_Store`, `Thumbs.db` | OS metadata |
| `*.swp`, `*.swo`, `*~` | Editor temp files |
| `dist/`, `build/`, `out/` | Build artifacts |
| Large binary files (>10MB) | Should use Git LFS |

**ALWAYS CHECK before staging:**
```bash
git status                          # Review ALL untracked files
git diff --staged --stat            # Review filenames before full diff
```

If you see a suspicious filename, ASK the user before staging.

### 5. WORKING DIRECTORY MATTERS

When committing in a subdirectory (like `infrastructure/`), ALWAYS use `git -C`:

```bash
git -C infrastructure status
git -C infrastructure --no-pager diff --staged
git -C infrastructure commit -m "..."
```

NEVER use `cd <dir> && git ...` patterns.

## Commit Ceremony

Execute these steps in order. Do NOT skip steps.

### Step 1: Gather State

Run ALL of these commands in parallel:

```bash
git status                      # Working tree state
git --no-pager diff             # Unstaged changes (MUST use --no-pager)
git --no-pager diff --staged    # Staged changes (MUST use --no-pager)
git log -5                      # Recent commits with full messages
```

**NEVER use `--oneline`** - always show full commit messages to understand
the project's commit style including body formatting.

**NEVER truncate output** - if output is large, that's information you need.

### Step 2: Analyze Changes

Before staging, determine:

1. **What changed?** - List modified files and their purpose
2. **Why did it change?** - The motivation (fix bug, add feature, refactor)
3. **What is the impact?** - Does it break existing behavior?
4. **Does this file belong in git?** - Check against forbidden patterns above
5. **Is this too large for one commit?** - If >20 files, consider splitting

### Step 2.5: Pre-Stage File Review

**CRITICAL: Before staging, review each file for:**

1. **Secrets/credentials** - Scan for API keys, tokens, passwords
2. **Generated files** - Schema files, lock files (may be large)
3. **Binary files** - Images, compiled assets (should use LFS if large)
4. **Temporary files** - Editor backups, OS metadata

```bash
# Check file sizes before staging
git status --porcelain | while read status file; do
  ls -lh "$file" 2>/dev/null
done

# Or use git to show sizes
git diff --stat  # Shows line counts per file
```

If a file looks suspicious, ASK before staging.

### Step 3: Stage Logical Groups

MUST stage related changes together. MUST NOT mix unrelated changes.

**Stage in small, reviewable chunks:**

| Scope | Files | Example |
|-------|-------|---------|
| `neovim` | `src/programs/neovim/*` | keymaps, plugins, options |
| `flake` | `flake.nix`, `src/devshells/*` | nix config, shells |
| `opencode` | `opencode.json`, `.opencode/*` | ai config, skills |
| `qcow2` | `src/qcow2/*` | vm image config |
| `docs` | `README.md`, `docs/*` | documentation |
| `deps` | `flake.lock` alone | dependency updates |

```bash
git add <files>
```

### Step 3.5: MANDATORY - Read Full Staged Diff

**THIS STEP IS NON-NEGOTIABLE. NEVER SKIP THIS.**

```bash
git --no-pager diff --staged     # Read EVERY line of this output
```

**You MUST:**
1. Execute this command
2. Read the COMPLETE output
3. Understand what you are committing
4. Verify no secrets, credentials, or unwanted files

**If the diff is too large to read:**
1. STOP - do not commit
2. Unstage some files: `git reset HEAD <file>`
3. Stage a smaller group
4. Return to this step

**FORBIDDEN patterns that bypass reading:**
```bash
# ALL OF THESE ARE FORBIDDEN
git diff --staged | head
git diff --staged | tail  
git diff --staged > /tmp/diff.txt
git diff --staged --stat  # Only shows filenames, not content
```

### Step 4: Determine Commit Type

Select ONE type based on the primary change:

| Type | SemVer | Use When |
|------|--------|----------|
| `feat` | MINOR | Adding new functionality |
| `fix` | PATCH | Correcting a bug |
| `docs` | - | Documentation only changes |
| `style` | - | Formatting, whitespace (no code change) |
| `refactor` | - | Code restructuring (no behavior change) |
| `perf` | PATCH | Performance improvement |
| `test` | - | Adding or fixing tests |
| `build` | - | Build system or dependencies |
| `ci` | - | CI/CD configuration |
| `chore` | - | Maintenance tasks |
| `revert` | varies | Reverting previous commit |

### Step 5: Check for Breaking Changes

A breaking change:
- Removes or renames public API
- Changes behavior that consumers depend on
- Requires consumers to modify their code

If breaking change exists:
- Add `!` before the colon: `feat(api)!: remove deprecated endpoint`
- OR add footer: `BREAKING CHANGE: description of what breaks`

### Step 6: Write Description

The description MUST:
- Immediately follow the colon and space
- Be lowercase (except proper nouns)
- Use imperative mood ("add" not "added" or "adds")
- Not end with a period
- Be under 72 characters
- Summarize WHAT changed, not HOW

### Step 7: Decide on Body

Include a body when ANY of these apply:

| Condition | Action |
|-----------|--------|
| Multiple logical changes in one commit | MUST include body |
| Change requires explanation of WHY | SHOULD include body |
| Commit touches 3+ files | SHOULD include body |
| Breaking change needs detail | MUST include body |
| Non-obvious implementation | SHOULD include body |
| Simple single-file change | MAY omit body |
| Description fully explains change | MAY omit body |

Body format:
- Blank line after description
- Wrap at 72 characters
- Explain WHAT and WHY, not HOW
- Use bullet points for multiple items
- Free-form paragraphs allowed

### Step 8: Add Footers (if needed)

Footer format: `Token: value` or `Token #value`

| Footer | Use When |
|--------|----------|
| `BREAKING CHANGE:` | API/behavior breaking (MUST be uppercase) |
| `Fixes #123` | Closes an issue |
| `Refs #456` | References related issue |
| `Reviewed-by:` | Code review attribution |

NEVER include:
- `Co-authored-by:` with AI names
- `Signed-off-by:` with AI identities  
- Any PII (emails, usernames) not already public in git config

### Step 9: Execute Commit

**STOP! Have you read the full staged diff? If not, go back to Step 3.5.**

For subject-only commits:
```bash
git commit -m "type(scope): description"
```

For commits with body:
```bash
git commit -m "type(scope): description

- first change explanation
- second change explanation
- third change explanation"
```

For commits with body and footer:
```bash
git commit -m "type(scope): description

Explanation of the change and its motivation.

BREAKING CHANGE: description of what breaks"
```

### Step 10: Verify Success

MUST run after every commit:

```bash
git status                      # Confirm state
git log -1 --format=fuller      # Show FULL commit (not oneline)
```

Report to user:
- Commit hash (short)
- Full commit message (type, scope, description, body if present)
- Files changed count
- Remaining unstaged changes (if any)

### Step 11: Continue or Complete

If unstaged changes remain:
1. Count remaining files
2. Ask: "N files remain unstaged. Continue with another commit?"
3. If yes, return to Step 3

## Quality Checklist

Before executing commit, verify:

- [ ] **DIFF READ** - I ran `git --no-pager diff --staged` and read EVERY line
- [ ] **NO TRUNCATION** - I did NOT use head/tail/redirect to limit diff output
- [ ] **NO SECRETS** - No API keys, tokens, passwords, or credentials in diff
- [ ] **NO FORBIDDEN FILES** - No .env, kubeconfig, *.key, *.pem files staged
- [ ] Type matches the primary change purpose
- [ ] Scope matches the affected codebase area  
- [ ] Description is lowercase imperative under 72 chars
- [ ] Body included if multiple changes or non-obvious
- [ ] No AI attribution in footers
- [ ] No PII beyond git config
- [ ] Breaking changes marked with `!` or `BREAKING CHANGE:`

## Examples

### Feature with body (correct)

```
feat(opencode): add git-commit skill for conventional commits

- implement conventionalcommits.org v1.0.0 specification
- add decision tree for body inclusion
- add breaking change detection guidance
- include quality checklist for verification
```

### Bug fix without body (correct)

```
fix(starship): silence error on dumb terminals
```

### Breaking change with footer (correct)

```
feat(api)!: change authentication to OAuth2

Migrate from API key authentication to OAuth2 flow.
Existing API keys will stop working after v2.0.0.

BREAKING CHANGE: API key authentication removed, use OAuth2 tokens
Refs #142
```

### Dependency update (correct)

```
chore(deps): update flake inputs

- nixpkgs: 89dbf01 -> 30a3c51
- nixvim: cae79c4 -> 983751b
- rust-overlay: 03c6e38 -> 056ce5b
```

### Multiple scope refactor (correct)

```
refactor(neovim): reorganize ai keymaps and fix checkhealth

keymaps:
- change ai prefix from <leader>v to <leader>a
- flatten hierarchy for direct tool access

checkhealth:
- switch nixpkgs-fmt to nixfmt-rfc-style
- disable mercurial in diffview
- disable latex in render-markdown
```

## Anti-Patterns (NEVER do these)

### Diff Truncation (CRITICAL VIOLATION)

```bash
# FORBIDDEN - These hide code you're about to commit
git diff --staged | head -100      # Truncates to 100 lines
git diff --staged | tail -50       # Shows only last 50 lines
git diff --staged > diff.txt       # Hides output in file
git diff --staged 2>&1 | head      # Truncates stderr too
git diff --stat                    # Only shows filenames
git diff --staged --shortstat      # Only shows counts

# REQUIRED - Always use this
git --no-pager diff --staged       # Full output, no paging
```

### Committing Without Reading

```bash
# FORBIDDEN - Committing without seeing the diff
git add . && git commit -m "..."   # Never saw what was staged

# REQUIRED - Always read between add and commit
git add <files>
git --no-pager diff --staged       # READ THIS
git commit -m "..."
```

### Committing Secrets

```bash
# FORBIDDEN - These files should NEVER be committed
git add .env                       # Contains secrets
git add kubeconfig                 # Cluster credentials
git add credentials.json           # API credentials
git add *.pem                      # Private keys
git add *secret*                   # Anything with "secret" in name

# REQUIRED - Check filenames before staging
git status                         # Review untracked files first
```

### Commit Message Anti-Patterns

```
# Using --oneline (hides commit body, prevents learning style)
git log --oneline

# Vague description
fix: stuff

# Capitalized type or scope  
Fix(Flake): update config

# Past tense
feat(api): added new endpoint

# AI attribution
feat(neovim): add keymaps

Co-authored-by: Claude <claude@anthropic.com>

# Implementation chatter
fix(starship): I noticed the prompt was showing errors so I added a check...

# Missing body when needed (multiple files, non-obvious change)
refactor(neovim): reorganize ai keymaps and fix checkhealth warnings
```

### Oversized Commits

```bash
# FORBIDDEN - Too many files to review
git add .                          # Adds everything - can't review
git add src/                       # Adds entire directory blindly

# REQUIRED - Stage files you can review
git add src/specific-file.py       # Stage specific files
git --no-pager diff --staged       # Verify you can read entire diff
```

## Activation

Load this skill when user:
- Says "commit", "stage and commit", "create commit"
- Asks to "prepare for PR" or "save changes"  
- Requests conventional commit format
- After completing work and asking to persist changes

## Summary: The Golden Rule

**READ BEFORE YOU COMMIT.**

Every commit follows this pattern:
1. `git add <specific files>`
2. `git --no-pager diff --staged` ← **READ EVERY LINE**
3. `git commit -m "..."`

If you cannot read the full diff, the commit is too large. Split it.

Never truncate. Never skip. Never assume. READ.
