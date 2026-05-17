# SpiritStream Coding Standards

These rules apply to all code modifications in this project.

## TypeScript Conventions

### Naming

- **Classes / Interfaces / Types**: PascalCase (`ProfileService`, `OutputGroup`)
- **Variables / Functions / Methods**: camelCase (`loadProfile`, `streamTargets`)
- **Constants**: UPPER_SNAKE_CASE (`MAX_RETRY_COUNT`)
- **Files**: camelCase for utilities (`profileStore.ts`), PascalCase for components (`StreamStatus.tsx`)

### Types

- Explicit return types on every public function/method.
- `interface` for object shapes, `type` for unions/aliases.
- `readonly` on properties that shouldn't change.
- Strict null checks — handle `undefined` and `null` explicitly.
- Import domain types only from `@spiritstream/types`. Never re-declare a Rust-side type in TypeScript.

## Rust Conventions

### Service pattern

- Services live in `crates/core/src/services/` and are constructed once at startup, held in `ServiceRegistry`, and shared as `Arc<...>` across all active transports.
- Service methods return `Result<T, CoreError>` using the structured enum at `crates/core/src/errors.rs`. **No `Result<T, String>` in new code.**
- HTTP handlers wrap errors via `ApiError(CoreError)`'s single `IntoResponse` impl in `crates/transport-http`. CLI commands map `CoreError` → exit code in `crates/transport-cli`. Mapping lives in the transport, not in core.
- Core compiles without `axum`, `tower`, `hyper`, `tauri`, or `clap`. Adding a transport-specific dependency to `crates/core/Cargo.toml` is a bug.

### Sensitive data

- Use `mask_sensitive()` / `redact_payload()` (in `crates/transport-http/src/lib.rs`) before logging any request or response that may contain stream keys, OAuth tokens, or session cookies. The `tracing` redaction layer enforces this at the boundary.
- Encrypt at-rest secrets through the `SecretStore` trait — either `KeyringSecretStore` or `EncryptedFileSecretStore`. The choice is made once at startup in `build_secret_store(app_data_dir, override_kind)`; never call platform keyring or file APIs directly.
- New encrypted payloads use AES-256-GCM-SIV (the V2 envelope). The V1 AES-GCM envelope is read-only and re-saves auto-upgrade.
- Path inputs validated via `validate_path_within_any()` to prevent traversal.
- Audit-log entries go through `AuditLogService::record` — never write log files directly. The HMAC chain depends on the service owning every append.

### Naming

- **Structs / Enums / Traits**: PascalCase (`ProfileService`, `CoreError`)
- **Functions / Methods**: snake_case (`get_all_profiles`, `start_stream`)
- **Constants**: UPPER_SNAKE_CASE (`DEFAULT_PORT`)
- **Modules**: snake_case (`profile_service`, `audit_log`)

### DTOs

- Write transport DTOs with `#[serde(rename_all = "camelCase")]` from the first draft. Don't reactively add it after test failures.
- ts-rs `#[derive(TS)]` + `#[ts(export, export_to = "../../packages/types/src/")]` on every domain type that crosses the transport boundary.

## Frontend Patterns

### Backend abstraction

- All API calls go through the `api` client from `@spiritstream/api-client`. Never call `fetch()` directly from a component.
- The HTTP client owns retry logic, cookie-based auth, and CSRF token attachment. Components only see typed methods returning typed results.

### State management

- Zustand stores in `apps/web/src/stores/`, one store per domain (`profileStore`, `settingsStore`, …).
- Stores hold **UI state only**. Validation, orchestration, side-effect decisions all live in `crates/core`.
- Stores export hooks: `useProfileStore`, `useSettingsStore`, etc.

### Error handling

```typescript
try {
  const result = await api.profile.load(name);
  // update state
} catch (error) {
  showError(`Failed to load profile: ${error.message}`);
}
```

### Components

- Functional components only, one per file.
- Props interface defined above the component.
- Use `forwardRef` when exposing refs.
- Memoize expensive computations.
- Subscribe to server events via the `useEvents` hook; every effect that subscribes must return a cleanup function — no leaked listeners.

## CSS / Tailwind

- Tailwind v4 with design tokens via `var(--token)`.
- Semantic class names for custom CSS.
- Mobile-first responsive design.
- Dark mode via `data-theme="dark"`.
- No `style={{}}` attributes, no hardcoded hex colors, no magic z-indices. Use CSS custom properties from `apps/web/src/styles/tokens.css`.

## Comments

- Default to writing no comments. Well-named identifiers carry the WHAT.
- Add a comment only when the WHY is non-obvious — a hidden constraint, a subtle invariant, a workaround for a specific upstream bug.
- Use JSDoc for public TS APIs, `///` for public Rust APIs.
- Keep comments up to date when code changes.
- **No TODO / FIXME / XXX markers in source.** Future work tracks in plans, GitHub issues, or `crates/transport-veilid/BLOCKERS.md` — never in code.
- **No `#[allow(dead_code)]`.** Every type, field, and function must have a live caller in the same change.
