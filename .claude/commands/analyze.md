---
description: Analyze a file or component in depth
allowed-tools:
  - Read
  - Grep
  - Glob
  - LSP
argument-hints: "file path or component name"
---

Perform a deep analysis of the specified file or component:

## Code Analysis
1. **Read the file** and understand its purpose
2. **Identify dependencies** - what it imports
3. **Identify dependents** - what imports it
4. **Document the API** - public methods/functions
5. **Note patterns used** - singleton, factory, etc.

## Quality Assessment
- Code complexity
- Error handling
- Type safety
- Documentation quality
- Test coverage needs

## Relationships
Use Grep to find:
- All imports of this file
- All usages of exported functions/classes
- Related configuration

## Output
Provide a structured analysis including:
1. Purpose and responsibility
2. Public API documentation
3. Dependencies graph
4. Potential improvements
5. Risk areas
