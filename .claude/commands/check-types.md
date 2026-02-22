---
description: Check TypeScript types without building
allowed-tools:
  - Bash
  - Read
  - Grep
---

Run type checking across the entire project:

## TypeScript (Frontend)
```bash
pnpm typecheck
```

## Rust (Backend)
```bash
cargo check --manifest-path server/Cargo.toml
```

## Tauri Desktop (if applicable)
```bash
cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml
```

If there are type errors:
1. List each error with file, line, and message
2. Explain what the type error means
3. Suggest the appropriate fix

Focus on:
- Missing type annotations
- Type mismatches between frontend types and backend models
- Serde serialization alignment (camelCase in TS, snake_case in Rust with `rename_all`)
- Missing properties
- Incorrect return types
