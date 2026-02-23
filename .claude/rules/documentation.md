# Documentation Guidelines

All Claude-generated documentation MUST be placed in `.claude/claudedocs/`.

## Directory Structure

```
.claude/claudedocs/
├── index.md                    # Master index (update when adding docs)
├── research/                   # Research & reference materials
│   └── *.md / *.html          # Analysis, mockups, design specs
└── scratch/                    # Temporary working documents
    └── *.md                    # Draft docs, notes, explorations
```

## Rules

1. **Never create docs in project root** — use `.claude/claudedocs/` exclusively
2. **Update index.md** — add new documents to the index with descriptions
3. **Use `scratch/` for temporary work** — draft analysis, exploration notes, temporary plans
4. **Use `research/` for reference materials** — mockups, external research, design specs
5. **Promote scratch to root when finalized** — move completed docs from `scratch/` to `claudedocs/`

## When to Create Documentation

| Scenario | Location | Filename Pattern |
|----------|----------|------------------|
| Planning a feature | `scratch/` | `feature-name-plan.md` |
| Analyzing code | `scratch/` | `analysis-topic.md` |
| API documentation | `claudedocs/` | `api-name.md` |
| Component specs | `claudedocs/` | `component-name.md` |
| Research/mockups | `research/` | Descriptive name |
| Architecture decisions | `claudedocs/` | `adr-NNN-title.md` |
