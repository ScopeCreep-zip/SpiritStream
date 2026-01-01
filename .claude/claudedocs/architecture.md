# MagillaStream Architecture

## Overview

MagillaStream follows a layered architecture typical of Electron applications, with clear separation between the main process, renderer process, and shared code.

## Layer Diagram

```
┌─────────────────────────────────────────────────────────────────────┐
│                          Presentation Layer                          │
│  ┌─────────────────────────────────────────────────────────────────┐ │
│  │                    Frontend (Vanilla JS)                         │ │
│  │                   src/frontend/index/                            │ │
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐           │ │
│  │  │   Sidebar    │  │  Main Area   │  │   Modals     │           │ │
│  │  │  - Profiles  │  │  - Groups    │  │  - Add/Edit  │           │ │
│  │  │  - Controls  │  │  - Targets   │  │  - Confirm   │           │ │
│  │  └──────────────┘  └──────────────┘  └──────────────┘           │ │
│  └─────────────────────────────────────────────────────────────────┘ │
├─────────────────────────────────────────────────────────────────────┤
│                          Bridge Layer                                │
│  ┌─────────────────────────────────────────────────────────────────┐ │
│  │                    Preload Script                                │ │
│  │                 src/electron/preload.ts                          │ │
│  │                                                                  │ │
│  │  contextBridge.exposeInMainWorld('electronAPI', {               │ │
│  │    profileManager: { ... },                                     │ │
│  │    ffmpegHandler: { ... },                                      │ │
│  │    logger: { ... }                                              │ │
│  │  })                                                             │ │
│  └─────────────────────────────────────────────────────────────────┘ │
├─────────────────────────────────────────────────────────────────────┤
│                          IPC Layer                                   │
│  ┌─────────────────────────────────────────────────────────────────┐ │
│  │                    IPC Handlers                                  │ │
│  │                src/electron/ipcHandlers.ts                       │ │
│  │                                                                  │ │
│  │  ipcMain.handle('profile:load', ...)                            │ │
│  │  ipcMain.handle('ffmpeg:start', ...)                            │ │
│  │  ipcMain.handle('logger:write', ...)                            │ │
│  └─────────────────────────────────────────────────────────────────┘ │
├─────────────────────────────────────────────────────────────────────┤
│                          Service Layer                               │
│  ┌───────────────┐  ┌───────────────┐  ┌───────────────┐            │
│  │ProfileManager │  │ FFmpegHandler │  │    Logger     │            │
│  │               │  │               │  │               │            │
│  │ - save()      │  │ - start()     │  │ - log()       │            │
│  │ - load()      │  │ - stop()      │  │ - error()     │            │
│  │ - delete()    │  │ - getEncoders │  │ - warn()      │            │
│  └───────────────┘  └───────────────┘  └───────────────┘            │
│  ┌───────────────┐  ┌───────────────┐                               │
│  │  Encryption   │  │EncoderDetect  │                               │
│  │               │  │               │                               │
│  │ - encrypt()   │  │ - detect()    │                               │
│  │ - decrypt()   │  │ - filter()    │                               │
│  └───────────────┘  └───────────────┘                               │
├─────────────────────────────────────────────────────────────────────┤
│                          Domain Layer                                │
│  ┌───────────────┐  ┌───────────────┐  ┌───────────────┐            │
│  │    Profile    │  │  OutputGroup  │  │ StreamTarget  │            │
│  │               │  │               │  │               │            │
│  │ - id          │  │ - encoder     │  │ - url         │            │
│  │ - name        │  │ - resolution  │  │ - streamKey   │            │
│  │ - incomingUrl │  │ - bitrate     │  │ - port        │            │
│  │ - groups[]    │  │ - targets[]   │  │               │            │
│  └───────────────┘  └───────────────┘  └───────────────┘            │
│  ┌───────────────┐                                                  │
│  │     Theme     │                                                  │
│  │               │                                                  │
│  │ - colors      │                                                  │
│  │ - darkMode    │                                                  │
│  └───────────────┘                                                  │
├─────────────────────────────────────────────────────────────────────┤
│                       Infrastructure Layer                           │
│  ┌───────────────────────────────────────────────────────────────┐  │
│  │  File System          │  Child Processes    │  Configuration  │  │
│  │  - profiles/*.json    │  - FFmpeg           │  - encoders.conf│  │
│  │  - logs/*.log         │                     │                 │  │
│  └───────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────┘
```

## Data Flow

### Profile Load Flow

```
Frontend                Preload              IPC Handler           ProfileManager
   │                       │                      │                      │
   │ loadProfile(name)     │                      │                      │
   ├──────────────────────>│                      │                      │
   │                       │ invoke('profile:load')                      │
   │                       ├─────────────────────>│                      │
   │                       │                      │ load(name, password) │
   │                       │                      ├─────────────────────>│
   │                       │                      │                      │ read file
   │                       │                      │                      │ decrypt if needed
   │                       │                      │                      │ parse JSON
   │                       │                      │      ProfileDTO      │
   │                       │                      │<─────────────────────┤
   │                       │      ProfileDTO      │                      │
   │                       │<─────────────────────┤                      │
   │      ProfileDTO       │                      │                      │
   │<──────────────────────┤                      │                      │
   │ updateUI(profile)     │                      │                      │
```

### Stream Start Flow

```
Frontend                Preload              IPC Handler           FFmpegHandler
   │                       │                      │                      │
   │ startStream(groups)   │                      │                      │
   ├──────────────────────>│                      │                      │
   │                       │ invoke('ffmpeg:start')                      │
   │                       ├─────────────────────>│                      │
   │                       │                      │ start(group, url)    │
   │                       │                      ├─────────────────────>│
   │                       │                      │                      │ build command
   │                       │                      │                      │ spawn process
   │                       │                      │                      │ store in map
   │                       │                      │      { pid, status } │
   │                       │                      │<─────────────────────┤
   │                       │      { pid, status } │                      │
   │                       │<─────────────────────┤                      │
   │      { pid, status }  │                      │                      │
   │<──────────────────────┤                      │                      │
```

## Process Communication

### IPC Channel Naming Convention

All IPC channels follow the pattern: `{service}:{action}`

| Service | Actions |
|---------|---------|
| profile | getAllProfileNames, load, save, delete, getLastUsed, saveLastUsed |
| ffmpeg | test, start, stop, stopAll, getAudioEncoders, getVideoEncoders |
| logger | log, error, warn, info, debug |

### Security Considerations

1. **Context Isolation**: The renderer cannot directly access Node.js APIs
2. **Sandbox Mode**: Additional security layer enabled
3. **Preload Script**: Only exposed APIs are available to renderer
4. **Input Validation**: All IPC handlers validate incoming data

## Component Dependencies

```
main.ts
  └── ipcHandlers.ts
        ├── profileManager.ts
        │     ├── encryption.ts
        │     └── models/Profile.ts
        │           ├── models/OutputGroup.ts
        │           │     └── models/StreamTarget.ts
        │           └── models/Theme.ts
        ├── ffmpegHandler.ts
        │     └── encoderDetection.ts
        └── logger.ts
              └── rendererLogger.ts (bridge)
```

## State Management

### Main Process State
- ProfileManager: Cached profile list
- FFmpegHandler: Running process map
- Logger: Active log file handles

### Renderer State
- `outputGroups[]`: Current profile configuration
- Modal state: Open/closed flags
- UI state: Selected profile, active streams

### Persistence
- Profiles: `{userData}/profiles/*.json`
- Logs: `{userData}/logs/*.log`
- Last used: localStorage (renderer) + file (main)
