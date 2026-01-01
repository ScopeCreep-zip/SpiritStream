# MagillaStream Coding Standards

These rules apply to all code modifications in this project.

## TypeScript Conventions

### Naming
- **Classes/Interfaces/Types**: PascalCase (`ProfileManager`, `OutputGroupDTO`)
- **Variables/Functions/Methods**: camelCase (`loadProfile`, `streamTargets`)
- **Constants**: UPPER_SNAKE_CASE (`MAX_RETRY_COUNT`)
- **Private members**: underscore prefix (`_id`, `_name`)
- **Files**: camelCase for utilities (`profileManager.ts`), PascalCase for models (`Profile.ts`)

### Types
- Always use explicit return types for public methods
- Use `interface` for object shapes, `type` for unions/aliases
- Prefer `readonly` for properties that shouldn't change
- Use strict null checks - handle `undefined` and `null` explicitly

### Classes
- Use private constructor + getInstance() for singletons
- Implement toDTO() for models that need serialization
- Use static fromDTO() factory methods for deserialization

## Electron Patterns

### IPC Handlers
- Channel naming: `service:action` (e.g., `profile:load`)
- Always validate inputs in handlers
- Return serializable data only (DTOs, primitives)
- Handle errors with try/catch and logging

### Security
- Never disable context isolation
- Never enable node integration in renderer
- Sanitize all paths to prevent traversal attacks
- Mask sensitive data (stream keys) in logs

## Error Handling

### Main Process
```typescript
try {
  const result = await operation();
  return result;
} catch (error) {
  Logger.getInstance().error(`Operation failed: ${error.message}`);
  throw new Error(`User-friendly message: ${error.message}`);
}
```

### Frontend
```typescript
try {
  const result = await window.electronAPI.service.action();
  updateUI(result);
} catch (error) {
  showError(`Failed to perform action: ${error.message}`);
}
```

## File Organization

- One class per file for models
- Group related utilities in single files
- Keep IPC handlers in ipcHandlers.ts
- Shared interfaces in shared/interfaces.ts

## Comments

- Don't add comments for obvious code
- Do add comments for complex logic or non-obvious decisions
- Use JSDoc for public APIs
- Keep comments up to date when code changes
