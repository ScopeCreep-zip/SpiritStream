---
description: Build the project and report any errors
allowed-tools:
  - Bash
  - Read
  - Grep
---

Run the full build process for SpiritStream:

1. Build the Rust backend:
   ```bash
   cargo build --manifest-path server/Cargo.toml
   ```

2. Build the frontend:
   ```bash
   pnpm build:web
   ```

3. Or build everything at once:
   ```bash
   pnpm build
   ```

If there are compilation errors:
- Read the error messages carefully
- Identify the file and line number
- Report the errors with suggested fixes

After successful build, confirm:
- Rust server binary compiled without errors
- Frontend built to `apps/web/dist/`
