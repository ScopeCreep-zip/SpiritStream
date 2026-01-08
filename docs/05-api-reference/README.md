# API Reference

[Documentation](../README.md) > API Reference

---

## Overview

This section provides comprehensive API documentation for SpiritStream, including Tauri commands, events, type definitions, and error handling.

## Documents

| Document | Description | Audience |
|----------|-------------|----------|
| [01. Commands API](./01-commands-api.md) | Complete Tauri command reference | All levels |
| [02. Events API](./02-events-api.md) | Event system documentation | Intermediate+ |
| [03. Types Reference](./03-types-reference.md) | TypeScript and Rust type definitions | All levels |
| [04. Error Handling](./04-error-handling.md) | Error codes and recovery patterns | Intermediate+ |

## Command Categories

| Category | Commands | Purpose |
|----------|----------|---------|
| Profile | 7 | Profile CRUD operations |
| Stream | 8 | Stream control and status |
| System | 3 | Encoder detection, FFmpeg test |
| Settings | 5 | App settings management |
| FFmpeg | 4 | Download and version check |
| Theme | 4 | Theme management |

## Quick Reference

### Invoke a Command

```typescript
import { invoke } from '@tauri-apps/api/core';

const profiles = await invoke<string[]>('get_all_profiles');
```

### Listen to Events

```typescript
import { listen } from '@tauri-apps/api/event';

await listen<StreamStats>('stream_stats', (event) => {
  console.log('Stats:', event.payload);
});
```

---

*Section: 05-api-reference*
