# Contributing to SpiritStream

**Authority**: Delegated from [GOVERNANCE.md](./GOVERNANCE.md)
**Domain**: Contribution Process

Welcome! We're excited that you're interested in contributing to SpiritStream. This document explains how to participate in the project effectively.

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Getting Started](#getting-started)
- [Development Setup](#development-setup)
- [Making Contributions](#making-contributions)
- [Pull Request Process](#pull-request-process)
- [What to Expect from Review](#what-to-expect-from-review)
- [Communication](#communication)
- [Recognition](#recognition)

## Code of Conduct

This project follows the [Contributor Covenant Code of Conduct](https://www.contributor-covenant.org/version/2/1/code_of_conduct/). By participating, you are expected to uphold this code. Report unacceptable behavior to the maintainers.

## Getting Started

### Types of Contributions

We welcome many types of contributions:

| Type | Description |
|------|-------------|
| **Bug Fixes** | Fixes for issues in the tracker |
| **Features** | New functionality (discuss first in an issue) |
| **Documentation** | README, guides, inline docs, examples |
| **Tests** | Unit tests, integration tests, E2E tests |
| **Refactoring** | Code quality improvements (no behavior change) |
| **Translations** | i18n support for additional languages |
| **Bug Reports** | Well-documented issues with reproduction steps |
| **Design** | UI/UX improvements, mockups, accessibility |

### First-Time Contributors

Look for issues labeled:
- `good first issue` - Simple, well-scoped tasks
- `help wanted` - We'd appreciate community help
- `documentation` - Non-code contributions

## Development Setup

### Prerequisites

| Tool | Version | Purpose |
|------|---------|---------|
| Node.js | 18+ | Frontend tooling |
| Rust | 1.70+ | Backend development |
| pnpm/npm | Latest | Package management |
| FFmpeg | 6.0+ | Stream processing |

### Quick Start

```bash
# Clone the repository
git clone https://github.com/ScopeCreep-zip/SpiritStream.git
cd SpiritStream

# Run setup (installs dependencies, checks prerequisites)
./setup.sh        # Linux/macOS
./setup.ps1       # Windows PowerShell

# Start development server
npm run dev
```

### Nix Users

```bash
# With direnv (recommended)
direnv allow

# Or manually
nix develop
```

See `setup.sh` for detailed prerequisite checks and troubleshooting.

## Making Contributions

### Branch Naming

```
<type>/<description>

Types: feature, fix, docs, refactor, test, chore
Examples:
  feature/drag-drop-profiles
  fix/stream-reconnection
  docs/api-reference
```

### Commit Messages

We follow [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>(<scope>): <description>

[optional body]

[optional footer]
```

**Types:** `feat`, `fix`, `docs`, `style`, `refactor`, `test`, `chore`

**Scopes:** `frontend`, `backend`, `build`, `ci`, `i18n`

**Examples:**
```
feat(frontend): add drag-and-drop profile ordering

Implements dnd-kit for reordering profiles in the sidebar.
Order persists to disk via new Tauri commands.

Closes #42
```

```
fix(backend): prevent stream key exposure in logs

Masks stream keys with asterisks when logging stream
operations to prevent credential leakage in log files.
```

### Code Standards

#### TypeScript (Frontend)
- Strict mode enabled (`"strict": true`)
- Explicit return types for public functions
- `interface` for object shapes, `type` for unions
- Functional components with hooks

#### Rust (Backend)
- `cargo clippy` must pass without warnings
- `cargo fmt` for formatting
- Use `Result<T, E>` for error handling
- Document public APIs with `///`

#### General
- No hardcoded secrets or credentials
- Prefer editing existing files over creating new ones
- Keep changes focused - one concern per PR
- Include tests for new functionality

### Pre-Submit Checklist

Before opening a PR, verify:

```bash
# TypeScript type checking
npm run typecheck

# Rust checks
cargo check --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml

# Linting
npm run lint

# Format check
npm run format:check
cargo fmt --check --manifest-path src-tauri/Cargo.toml

# Build (catches integration issues)
npm run build
```

## Pull Request Process

### Opening a PR

1. **Create from a feature branch** - Never commit directly to `main`

2. **Fill out the PR template:**
   ```markdown
   ## Summary
   Brief description of changes (1-3 sentences)

   ## Changes
   - Specific change 1
   - Specific change 2

   ## Test Plan
   How you verified this works

   ## Screenshots (if UI changes)
   Before/after screenshots
   ```

3. **Link related issues** - Use `Closes #123` or `Fixes #456`

4. **Keep PRs focused** - One feature or fix per PR

5. **Ensure CI passes** - All checks must be green

### PR Size Guidelines

| Size | Lines Changed | Review Time |
|------|---------------|-------------|
| Small | < 100 | Same day |
| Medium | 100-500 | 1-3 days |
| Large | 500+ | Consider splitting |

Large PRs are harder to review thoroughly. When possible, break work into smaller, reviewable chunks.

### Responding to Feedback

- **Be responsive** - Try to address feedback within 48 hours
- **Ask questions** - If feedback is unclear, ask for clarification
- **Accept suggestions** - GitHub suggestion blocks can be accepted with one click
- **Explain decisions** - If you disagree, explain your reasoning respectfully
- **Push updates** - After addressing feedback, push changes and re-request review

## What to Expect from Review

Our review process is documented in [REVIEWERS.md](./REVIEWERS.md). Here's what you can expect:

### Review Timeline

| PR Size | Target Response |
|---------|-----------------|
| Small | Within 24 hours |
| Medium | Within 48 hours |
| Large | Within 72 hours |

### Feedback Categories

Reviewers categorize feedback to help you prioritize:

| Category | Meaning |
|----------|---------|
| **Blocking** | Must fix before merge (bugs, security, breaks build) |
| **Recommended** | Should fix, but not blocking (validation, edge cases) |
| **Minor** | Nice to have (style, naming, comments) |

### What Reviewers Check

- **Correctness** - Does the code work as intended?
- **API Consistency** - Do names match across layers?
- **Security** - No injection, path traversal, or data leaks?
- **Tests** - Is new functionality tested?
- **Documentation** - Are changes documented where needed?

### Your Rights as a Contributor

- Receive respectful, constructive feedback
- Ask for clarification on any review comment
- Disagree with feedback (with explanation)
- Request a different reviewer if needed
- Have your PR reviewed in a timely manner

## Communication

### Where to Ask Questions

| Channel | Purpose |
|---------|---------|
| GitHub Issues | Bug reports, feature requests |
| GitHub Discussions | Questions, ideas, help |
| Pull Request Comments | PR-specific discussion |

### Getting Help

- **Stuck on setup?** - Check `setup.sh` output and open a discussion
- **Unclear requirements?** - Comment on the issue before starting work
- **Review taking too long?** - Ping maintainers politely in the PR

## Recognition

### Contributors File

All contributors are recognized in the repository. Significant contributions are highlighted in release notes.

### Becoming a Reviewer

Active contributors who demonstrate:
- Consistent, quality contributions
- Understanding of the codebase
- Constructive communication

May be invited to become reviewers. See [REVIEWERS.md](./REVIEWERS.md) for reviewer responsibilities.

## Governance

Project governance follows a structured milestone ceremony:

| Document | Purpose |
|----------|---------|
| [GOVERNANCE.md](./GOVERNANCE.md) | Root governance specification (SSOT) |
| [MILESTONES_CEREMONY.md](./MILESTONES_CEREMONY.md) | How governance works |
| [MILESTONES.main.md](./MILESTONES.main.md) | Propose governance changes |
| [MILESTONES.latest.md](./MILESTONES.latest.md) | Current commitments |

To propose governance changes (scope, process, domains), add to [MILESTONES.main.md](./MILESTONES.main.md).

---

## Quick Reference

### Commands

```bash
npm run dev          # Development server
npm run build        # Production build
npm run typecheck    # TypeScript checks
npm run lint         # Lint code
npm run format       # Format code
cargo check          # Rust type check
cargo clippy         # Rust linting
cargo test           # Rust tests
```

### PR Checklist

- [ ] Branch follows naming convention
- [ ] Commits follow conventional commits
- [ ] All CI checks pass
- [ ] PR description filled out
- [ ] Related issues linked
- [ ] Self-reviewed the diff
- [ ] No secrets or credentials committed

---

*This document complements [REVIEWERS.md](./REVIEWERS.md). Together with [GOVERNANCE.md](./GOVERNANCE.md) and [MILESTONES_CEREMONY.md](./MILESTONES_CEREMONY.md), they form the governance framework for SpiritStream.*

---

Thank you for contributing to SpiritStream!
