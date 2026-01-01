# Electron IPC Communication

## Overview

MagillaStream uses Electron's Inter-Process Communication (IPC) system to enable communication between the renderer process (frontend) and the main process (Node.js backend).

## Security Model

### Context Isolation
The renderer process runs in an isolated context, preventing direct access to Node.js APIs.

```typescript
// main.ts - Window creation with security settings
const mainWindow = new BrowserWindow({
  width: 1200,
  height: 800,
  webPreferences: {
    contextIsolation: true,      // Isolate renderer context
    sandbox: true,               // Enable sandbox mode
    nodeIntegration: false,      // Disable Node.js in renderer
    preload: path.join(__dirname, 'preload.js')
  }
});
```

### Preload Script

The preload script acts as a secure bridge between renderer and main process.

```typescript
// preload.ts
import { contextBridge, ipcRenderer } from 'electron';

contextBridge.exposeInMainWorld('electronAPI', {
  profileManager: {
    getAllProfileNames: () => ipcRenderer.invoke('profile:getAllProfileNames'),
    load: (name: string, password?: string) =>
      ipcRenderer.invoke('profile:load', name, password),
    save: (profile: ProfileDTO, password?: string) =>
      ipcRenderer.invoke('profile:save', profile, password),
    delete: (name: string) => ipcRenderer.invoke('profile:delete', name),
    getLastUsed: () => ipcRenderer.invoke('profile:getLastUsed'),
    saveLastUsed: (name: string) => ipcRenderer.invoke('profile:saveLastUsed', name)
  },
  ffmpegHandler: {
    test: () => ipcRenderer.invoke('ffmpeg:test'),
    start: (group: OutputGroupDTO, incomingUrl: string) =>
      ipcRenderer.invoke('ffmpeg:start', group, incomingUrl),
    stop: (groupId: string) => ipcRenderer.invoke('ffmpeg:stop', groupId),
    stopAll: () => ipcRenderer.invoke('ffmpeg:stopAll'),
    getAudioEncoders: () => ipcRenderer.invoke('ffmpeg:getAudioEncoders'),
    getVideoEncoders: () => ipcRenderer.invoke('ffmpeg:getVideoEncoders')
  },
  logger: {
    log: (level: string, message: string) =>
      ipcRenderer.invoke('logger:log', level, message)
  },
  getUserDataPath: () => ipcRenderer.invoke('app:getUserDataPath')
});
```

## IPC Handlers

### Handler Registration

All IPC handlers are registered in `ipcHandlers.ts`:

```typescript
// ipcHandlers.ts
import { ipcMain } from 'electron';
import { ProfileManager } from '../utils/profileManager';
import { FFmpegHandler } from '../utils/ffmpegHandler';

export function registerIpcHandlers(): void {
  // Profile handlers
  ipcMain.handle('profile:getAllProfileNames', async () => {
    return ProfileManager.getInstance().getAllProfileNames();
  });

  ipcMain.handle('profile:load', async (_, name: string, password?: string) => {
    return ProfileManager.getInstance().load(name, password);
  });

  // FFmpeg handlers
  ipcMain.handle('ffmpeg:start', async (_, group, incomingUrl) => {
    return FFmpegHandler.getInstance().start(group, incomingUrl);
  });

  // ... more handlers
}
```

### Handler Pattern

Each handler follows a consistent pattern:

```typescript
ipcMain.handle('channel:action', async (event, ...args) => {
  try {
    // 1. Validate input
    if (!args[0]) throw new Error('Missing required parameter');

    // 2. Call service
    const result = await SomeService.getInstance().action(...args);

    // 3. Return result (must be serializable)
    return result;
  } catch (error) {
    // 4. Handle errors
    Logger.getInstance().error(`Error in channel:action: ${error.message}`);
    throw error;
  }
});
```

## Channel Reference

### Profile Channels

| Channel | Direction | Parameters | Returns |
|---------|-----------|------------|---------|
| `profile:getAllProfileNames` | invoke | none | `string[]` |
| `profile:load` | invoke | `name: string, password?: string` | `ProfileDTO` |
| `profile:save` | invoke | `profile: ProfileDTO, password?: string` | `void` |
| `profile:delete` | invoke | `name: string` | `void` |
| `profile:getLastUsed` | invoke | none | `string \| null` |
| `profile:saveLastUsed` | invoke | `name: string` | `void` |

### FFmpeg Channels

| Channel | Direction | Parameters | Returns |
|---------|-----------|------------|---------|
| `ffmpeg:test` | invoke | none | `boolean` |
| `ffmpeg:start` | invoke | `group: OutputGroupDTO, url: string` | `ProcessInfo` |
| `ffmpeg:stop` | invoke | `groupId: string` | `void` |
| `ffmpeg:stopAll` | invoke | none | `void` |
| `ffmpeg:getAudioEncoders` | invoke | none | `EncoderInfo[]` |
| `ffmpeg:getVideoEncoders` | invoke | none | `EncoderInfo[]` |

### Logger Channels

| Channel | Direction | Parameters | Returns |
|---------|-----------|------------|---------|
| `logger:log` | invoke | `level: string, message: string` | `void` |

### App Channels

| Channel | Direction | Parameters | Returns |
|---------|-----------|------------|---------|
| `app:getUserDataPath` | invoke | none | `string` |

## Data Transfer Objects (DTOs)

All data passed through IPC must be serializable. The project uses DTOs defined in `shared/interfaces.ts`:

```typescript
// shared/interfaces.ts
export interface ProfileDTO {
  id: string;
  name: string;
  incomingUrl: string;
  outputGroups: OutputGroupDTO[];
  theme?: ThemeDTO;
}

export interface OutputGroupDTO {
  id: string;
  videoEncoder: string;
  resolution: string;
  videoBitrate: number;
  fps: number;
  audioCodec: string;
  audioBitrate: number;
  generatePts: boolean;
  streamTargets: StreamTargetDTO[];
}

export interface StreamTargetDTO {
  id: string;
  url: string;
  streamKey: string;
  port: number;
}
```

## Error Handling

### Main Process
```typescript
ipcMain.handle('profile:load', async (_, name, password) => {
  try {
    return await ProfileManager.getInstance().load(name, password);
  } catch (error) {
    // Log error in main process
    Logger.getInstance().error(`Failed to load profile: ${error.message}`);
    // Re-throw to propagate to renderer
    throw new Error(`Failed to load profile: ${error.message}`);
  }
});
```

### Renderer Process
```typescript
async function loadProfile(name) {
  try {
    const profile = await window.electronAPI.profileManager.load(name);
    updateUI(profile);
  } catch (error) {
    showErrorModal(`Could not load profile: ${error.message}`);
  }
}
```

## Best Practices

1. **Always use `invoke`/`handle`**: Use invoke pattern for request-response communication
2. **Keep payloads small**: Only send necessary data
3. **Use DTOs**: Don't send class instances, convert to plain objects
4. **Validate inputs**: Always validate parameters in handlers
5. **Handle errors**: Wrap handlers in try-catch
6. **Log operations**: Log significant IPC operations for debugging
7. **Avoid synchronous IPC**: Never use `sendSync` or `invokeSync`

## Type Safety

The preload script exports are typed via declaration file:

```typescript
// types/preload.d.ts
interface ElectronAPI {
  profileManager: {
    getAllProfileNames: () => Promise<string[]>;
    load: (name: string, password?: string) => Promise<ProfileDTO>;
    save: (profile: ProfileDTO, password?: string) => Promise<void>;
    delete: (name: string) => Promise<void>;
    getLastUsed: () => Promise<string | null>;
    saveLastUsed: (name: string) => Promise<void>;
  };
  ffmpegHandler: {
    test: () => Promise<boolean>;
    start: (group: OutputGroupDTO, url: string) => Promise<ProcessInfo>;
    stop: (groupId: string) => Promise<void>;
    stopAll: () => Promise<void>;
    getAudioEncoders: () => Promise<EncoderInfo[]>;
    getVideoEncoders: () => Promise<EncoderInfo[]>;
  };
  logger: {
    log: (level: string, message: string) => Promise<void>;
  };
  getUserDataPath: () => Promise<string>;
}

declare global {
  interface Window {
    electronAPI: ElectronAPI;
  }
}
```
