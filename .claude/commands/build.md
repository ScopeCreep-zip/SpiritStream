---
description: Build the project and report any errors
allowed-tools:
  - Bash
  - Read
  - Grep
---

Run the full build process for SpiritStream:

1. First run `npm run clean` to clean previous builds
2. Then run `npm run compile` to compile TypeScript
3. Finally run `npm run copy-resources` to copy static files

If there are any TypeScript compilation errors:
- Read the error messages carefully
- Identify the file and line number
- Report the errors with suggested fixes

After successful build, confirm:
- dist/electron/main.js exists
- dist/config/encoders.conf exists
- dist/frontend/index/index.html exists
