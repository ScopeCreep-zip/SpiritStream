# Electron Development Patterns

These patterns should be followed for all Electron-related code.

## Security First

### BrowserWindow Creation
Always use these security settings:
```typescript
new BrowserWindow({
  webPreferences: {
    contextIsolation: true,
    sandbox: true,
    nodeIntegration: false,
    preload: path.join(__dirname, 'preload.js')
  }
});
```

### Never Do
- Set `nodeIntegration: true`
- Set `contextIsolation: false`
- Set `webSecurity: false`
- Use `remote` module
- Expose unnecessary APIs in preload

## IPC Communication

### Handler Pattern
```typescript
ipcMain.handle('channel:action', async (event, arg1, arg2) => {
  // 1. Validate inputs
  if (!arg1 || typeof arg1 !== 'string') {
    throw new Error('Invalid argument');
  }

  // 2. Perform operation
  const result = await service.action(arg1, arg2);

  // 3. Return serializable result
  return result;
});
```

### Preload Bridge Pattern
```typescript
contextBridge.exposeInMainWorld('electronAPI', {
  serviceName: {
    action: (arg: string) => ipcRenderer.invoke('channel:action', arg)
  }
});
```

### Type Definitions
Always add types in preload.d.ts:
```typescript
interface ElectronAPI {
  serviceName: {
    action: (arg: string) => Promise<ResultType>;
  };
}
```

## Data Serialization

### What Can Cross IPC
- Primitives (string, number, boolean)
- Plain objects (no class instances)
- Arrays of the above
- null and undefined

### What Cannot Cross IPC
- Class instances
- Functions
- Symbols
- Circular references
- DOM elements

### Solution: Use DTOs
```typescript
// Model (main process)
class Profile {
  toDTO(): ProfileDTO {
    return { id: this._id, name: this._name };
  }
}

// DTO (shared)
interface ProfileDTO {
  id: string;
  name: string;
}
```

## Process Communication

### Main to Renderer
```typescript
// Main process
mainWindow.webContents.send('event:name', data);

// Preload
ipcRenderer.on('event:name', (event, data) => {
  callback(data);
});
```

### Renderer to Main (Request/Response)
```typescript
// Preload
action: () => ipcRenderer.invoke('channel:action')

// Main
ipcMain.handle('channel:action', async () => {
  return result;
});
```

## File Paths

### Use app.getPath()
```typescript
const userDataPath = app.getPath('userData');
const logsPath = path.join(userDataPath, 'logs');
const profilesPath = path.join(userDataPath, 'profiles');
```

### Development vs Production
```typescript
const isDev = !app.isPackaged;

const resourcePath = isDev
  ? path.join(__dirname, '..', 'resources')
  : path.join(process.resourcesPath, 'resources');
```

## Window Management

### Single Window Pattern
```typescript
let mainWindow: BrowserWindow | null = null;

function createWindow(): void {
  mainWindow = new BrowserWindow({...});

  mainWindow.on('closed', () => {
    mainWindow = null;
  });
}

app.whenReady().then(createWindow);

app.on('activate', () => {
  if (mainWindow === null) {
    createWindow();
  }
});
```

### Clean Shutdown
```typescript
app.on('window-all-closed', () => {
  if (process.platform !== 'darwin') {
    app.quit();
  }
});

app.on('before-quit', async () => {
  // Clean up resources
  await ffmpegHandler.stopAll();
  logger.close();
});
```
