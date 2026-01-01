---
description: Create a new domain model with DTO
allowed-tools:
  - Read
  - Write
  - Edit
argument-hints: "ModelName (e.g., Preset)"
---

Create a new domain model following the project's patterns:

1. **Create model file** at `src/models/{ModelName}.ts`:
   - Private properties with underscore prefix
   - Getters and setters
   - `toDTO()` method
   - Static `fromDTO()` factory method
   - Constructor with sensible defaults

2. **Add DTO interface** in `src/shared/interfaces.ts`:
   - All properties from the model
   - Use primitive types only (no class instances)

3. **Follow existing patterns** from:
   - Profile.ts
   - OutputGroup.ts
   - StreamTarget.ts

Example structure:
```typescript
export class ModelName {
  private _id: string;
  private _name: string;

  constructor(name: string = '') {
    this._id = generateUUID();
    this._name = name;
  }

  // Getters/Setters
  get id(): string { return this._id; }
  get name(): string { return this._name; }
  set name(value: string) { this._name = value; }

  // Serialization
  toDTO(): ModelNameDTO {
    return { id: this._id, name: this._name };
  }

  static fromDTO(dto: ModelNameDTO): ModelName {
    const instance = new ModelName(dto.name);
    instance._id = dto.id;
    return instance;
  }
}
```
