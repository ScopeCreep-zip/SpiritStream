---
description: Create a new domain model with DTO
allowed-tools:
  - Read
  - Write
  - Edit
argument-hints: "ModelName (e.g., Preset)"
---

Create a new domain model following the project's patterns:

1. **Create Rust model** at `server/src/models/{model_name}.rs`:
   - Derive `Debug, Clone, Serialize, Deserialize`
   - Use `#[serde(rename_all = "camelCase")]` for JSON interop
   - Use `Result<T, String>` for fallible operations
   - Add to `server/src/models/mod.rs` exports

   ```rust
   #[derive(Debug, Clone, Serialize, Deserialize)]
   #[serde(rename_all = "camelCase")]
   pub struct ModelName {
       pub id: String,
       pub name: String,
   }

   impl ModelName {
       pub fn new(name: String) -> Self {
           Self {
               id: uuid::Uuid::new_v4().to_string(),
               name,
           }
       }
   }
   ```

2. **Create TypeScript interface** in `apps/web/src/types/`:
   - Match camelCase field names from Rust serde output
   - Use `interface` for object shapes

   ```typescript
   export interface ModelName {
       id: string;
       name: string;
   }
   ```

3. **Follow existing patterns** from:
   - `server/src/models/profile.rs` + `apps/web/src/types/source.ts`
   - `server/src/models/output_group.rs`
   - `server/src/models/stream_target.rs`

4. **If the model needs a Zustand store**, create at `apps/web/src/stores/{modelName}Store.ts` following patterns in existing stores.
