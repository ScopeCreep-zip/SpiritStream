---
description: Add a new IPC handler with full integration
allowed-tools:
  - Read
  - Edit
  - Write
  - Grep
argument-hints: "service:action (e.g., profile:export)"
---

Add a new IPC handler for the specified channel. This command will:

1. **Add the IPC handler** in `src/electron/ipcHandlers.ts`:
   ```typescript
   ipcMain.handle('service:action', async (_, ...args) => {
     return ServiceClass.getInstance().action(...args);
   });
   ```

2. **Add the preload bridge** in `src/electron/preload.ts`:
   ```typescript
   serviceName: {
     action: (...args) => ipcRenderer.invoke('service:action', ...args)
   }
   ```

3. **Add type definitions** in `src/types/preload.d.ts`:
   ```typescript
   interface ElectronAPI {
     serviceName: {
       action: (...args: ArgTypes[]) => Promise<ReturnType>;
     };
   }
   ```

Follow existing patterns in the codebase for consistency.
