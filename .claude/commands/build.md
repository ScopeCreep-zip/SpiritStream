---
description: Build the project and report any errors
allowed-tools:
  - Bash
  - Read
  - Grep
---

Run the full build process for SpiritStream:

1. Run `pnpm build` to build all workspaces via Turbo
2. If frontend-only: `pnpm build:web`
3. If desktop: `pnpm build:desktop`
4. If server-only: `cargo build --manifest-path server/Cargo.toml --release`

If there are build errors:
- Read the error messages carefully
- Identify the file and line number
- Report the errors with suggested fixes

After successful build, confirm:
- `apps/web/dist/` exists (frontend assets)
- `server/target/release/spiritstream-server` exists (server binary, if built)
