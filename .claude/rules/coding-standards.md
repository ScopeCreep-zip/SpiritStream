# SpiritStream Coding Standards

These rules apply to all code modifications in this project.

## TypeScript Conventions

### Naming
- **Classes/Interfaces/Types**: PascalCase (`ProfileManager`, `OutputGroup`)
- **Variables/Functions/Methods**: camelCase (`loadProfile`, `streamTargets`)
- **Constants**: UPPER_SNAKE_CASE (`MAX_RETRY_COUNT`)
- **Files**: camelCase for utilities (`profileStore.ts`), PascalCase for components (`StreamStatus.tsx`)

### Types
- Always use explicit return types for public methods
- Use `interface` for object shapes, `type` for unions/aliases
- Prefer `readonly` for properties that shouldn't change
- Use strict null checks — handle `undefined` and `null` explicitly

## Rust Conventions

### Service Pattern
- Services are `Arc<ServiceManager>` fields in `AppState`, shared across routes
- Service methods return `Result<T, String>` — the `invoke()` handler wraps results in `InvokeResponse { ok, data, error }`
- Error handling: `log::error!()` + return `Err(string)`; let `invoke()` handle serialization

### Sensitive Data
- Use `mask_sensitive()` / `redact_payload()` before logging any request or response that may contain stream keys or tokens
- Path inputs validated via `validate_path_within_any()` to prevent traversal

### Naming
- **Structs/Enums/Traits**: PascalCase (`ProfileManager`, `InvokeResponse`)
- **Functions/Methods**: snake_case (`get_all_profiles`, `start_stream`)
- **Constants**: UPPER_SNAKE_CASE (`DEFAULT_PORT`)
- **Modules**: snake_case (`ffmpeg_handler`, `profile_manager`)

## Frontend Patterns

### Backend Abstraction
- All API calls go through `api.*` from `lib/backend/api.ts` — never call `fetch()` directly
- `api.ts` selects `httpApi` (default) or `tauriApi` (legacy) based on detected mode
- HTTP calls use `safeFetch()` with retry logic and cookie-based auth

### State Management
- Zustand stores in `stores/`, one store per domain (e.g., `profileStore`, `settingsStore`)
- Stores export hooks: `useProfileStore`, `useSettingsStore`, etc.

### Error Handling
```typescript
try {
  const result = await api.profile.load(name);
  // update state
} catch (error) {
  showError(`Failed to load profile: ${error.message}`);
}
```

### Components
- Functional components only, one per file
- Props interface defined above the component
- Use `forwardRef` when exposing refs
- Memoize expensive computations

## CSS / Tailwind
- Use design tokens via `var(--token)`
- Semantic class names for custom CSS
- Mobile-first responsive design
- Dark mode via `data-theme="dark"`

## Comments
- Don't add comments for obvious code
- Do add comments for complex logic or non-obvious decisions
- Use JSDoc for public TS APIs, `///` for public Rust APIs
- Keep comments up to date when code changes
