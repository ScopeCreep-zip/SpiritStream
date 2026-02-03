# SpiritStream Coding Standards

These rules apply to all code modifications in this project.

## TypeScript Conventions (Frontend)

### Naming
- **Classes/Interfaces/Types**: PascalCase (`ProfileManager`, `OutputGroupDTO`)
- **Variables/Functions/Methods**: camelCase (`loadProfile`, `streamTargets`)
- **Constants**: UPPER_SNAKE_CASE (`MAX_RETRY_COUNT`)
- **Files**: camelCase for utilities (`profileManager.ts`), PascalCase for components (`StreamStatus.tsx`)

### Types
- Always use explicit return types for public functions
- Use `interface` for object shapes, `type` for unions/aliases
- Prefer `readonly` for properties that shouldn't change
- Use strict null checks - handle `undefined` and `null` explicitly

### React Components
- Functional components with hooks (no class components)
- Props interface defined above component
- One component per file
- Memoize expensive computations with `useMemo`/`useCallback`

## Rust Conventions (Backend)

See `rust-patterns.md` for detailed Rust guidelines.

### Naming
- **Structs/Enums/Traits**: PascalCase
- **Functions/Methods**: snake_case
- **Constants**: UPPER_SNAKE_CASE
- **Modules**: snake_case

### Error Handling
- Use `Result<T, E>` for fallible operations
- Return descriptive error messages
- Avoid `unwrap()` in production code

## Backend/Frontend Communication

### HTTP API Pattern
```typescript
// Frontend calls backend via HTTP abstraction
const profile = await backend.invoke('load_profile', { name: 'default' });
```

### JSON Serialization
- Rust uses `#[serde(rename_all = "camelCase")]`
- TypeScript interfaces match camelCase field names
- Dates as ISO strings, not timestamps

### Security
- Never log stream keys or tokens
- Sanitize all paths to prevent traversal attacks
- Validate inputs on backend, trust nothing from frontend

## Error Handling

### Backend (Rust)
```rust
pub fn load_profile(&self, name: &str) -> Result<Profile, String> {
    fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read profile: {e}"))?;
    Ok(profile)
}
```

### Frontend (TypeScript)
```typescript
try {
  const result = await backend.invoke('load_profile', { name });
  setProfile(result);
} catch (error) {
  showError(`Failed to load profile: ${error}`);
}
```

## File Organization

### Frontend (`apps/web/src/`)
```
components/     # React components
  ui/           # Base UI components (Button, Card, etc.)
  stream/       # Streaming-related components
  modals/       # Modal dialogs
hooks/          # Custom React hooks
stores/         # Zustand state stores
lib/
  backend/      # Backend abstraction layer
types/          # TypeScript type definitions
views/          # Page-level components
```

### Backend (`server/src/`)
```
commands/       # HTTP command handlers
models/         # Data structures, DTOs
services/       # Business logic
```

## Comments

- Don't add comments for obvious code
- Do add comments for complex logic or non-obvious decisions
- Use `///` doc comments for public Rust APIs
- Use JSDoc for exported TypeScript functions
- Keep comments up to date when code changes
