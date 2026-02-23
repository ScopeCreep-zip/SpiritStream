---
description: Create a new domain model
allowed-tools:
  - Read
  - Write
  - Edit
  - Grep
argument-hints: "ModelName (e.g., Preset)"
---

Create a new domain model following the project's patterns:

1. **Create Rust model** at `server/src/models/{model_name}.rs`:
   ```rust
   use serde::{Deserialize, Serialize};

   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct ModelName {
       pub id: String,
       pub name: String,
   }
   ```

2. **Register in models module** — add `pub mod model_name;` to `server/src/models/mod.rs`

3. **Create TypeScript type** at `apps/web/src/types/{modelName}.ts` (or add to existing types file):
   ```typescript
   export interface ModelName {
     id: string;
     name: string;
   }
   ```

4. **Follow existing patterns** from:
   - Rust: `server/src/models/` (existing model files)
   - TypeScript: `apps/web/src/types/` (existing type files)

Ensure field names match between Rust (snake_case with `#[serde(rename)]` if needed) and TypeScript (camelCase).
