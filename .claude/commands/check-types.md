---
description: Check TypeScript types without building
allowed-tools:
  - Bash
  - Read
  - Grep
---

Run TypeScript type checking without emitting files:

```bash
npx tsc --noEmit
```

If there are type errors:
1. List each error with file, line, and message
2. Explain what the type error means
3. Suggest the appropriate fix

Focus on:
- Missing type annotations
- Type mismatches
- Missing properties
- Incorrect return types
