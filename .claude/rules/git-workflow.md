# Git Workflow Rules

Follow these conventions for all Git operations.

## Branch Naming

| Type | Pattern | Example |
|------|---------|---------|
| Feature | `feature/<description>` | `feature/add-preset-support` |
| Bug fix | `fix/<description>` | `fix/stream-reconnection` |
| Refactor | `refactor/<description>` | `refactor/ipc-handlers` |
| Documentation | `docs/<description>` | `docs/api-reference` |
| Hotfix | `hotfix/<description>` | `hotfix/critical-crash` |

## Commit Message Format

```
<type>(<scope>): <description>

[optional body]

[optional footer]
```

### Types
- `feat`: New feature
- `fix`: Bug fix
- `docs`: Documentation changes
- `style`: Code style (formatting, no logic change)
- `refactor`: Code restructuring
- `test`: Adding/modifying tests
- `chore`: Maintenance tasks

### Scopes
- `electron`: Main process changes
- `frontend`: UI changes
- `models`: Domain model changes
- `services`: Service layer changes
- `ipc`: IPC handler changes
- `build`: Build system changes

### Examples
```
feat(frontend): add dark mode toggle

Adds a toggle switch in settings to enable dark mode.
Updates CSS variables for dark theme support.

Closes #42
```

```
fix(services): prevent stream key exposure in logs

Masks stream keys with asterisks when logging
stream operations to prevent credential leakage.
```

## Before Committing

1. **Check for type errors**
   ```bash
   npx tsc --noEmit
   ```

2. **Verify build succeeds**
   ```bash
   npm run build
   ```

3. **Review changes**
   ```bash
   git diff --staged
   ```

4. **No sensitive data**
   - Check for hardcoded keys
   - Check for personal paths
   - Check for debug code

## Pull Request Guidelines

### Title Format
Same as commit message: `type(scope): description`

### Description Template
```markdown
## Summary
Brief description of changes

## Changes
- Change 1
- Change 2

## Testing
How this was tested

## Screenshots (if UI changes)
[Include screenshots]
```

### Checklist
- [ ] Code compiles without errors
- [ ] Code follows project conventions
- [ ] No sensitive data committed
- [ ] Documentation updated if needed
