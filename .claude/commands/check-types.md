---
description: Check TypeScript and Rust types without building
allowed-tools:
  - Bash
  - Read
  - Grep
---

Run type checking across the full stack:

1. **TypeScript** (frontend):
   ```bash
   pnpm typecheck
   ```

2. **Rust** (server):
   ```bash
   cargo check --manifest-path server/Cargo.toml
   ```

3. **Rust** (desktop launcher):
   ```bash
   cargo check --manifest-path apps/tauri/src-tauri/Cargo.toml
   ```

If there are type errors:
1. List each error with file, line, and message
2. Explain what the type error means
3. Suggest the appropriate fix

Focus on:
- Missing type annotations
- Type mismatches between frontend types and server models
- Missing properties
- Incorrect return types
