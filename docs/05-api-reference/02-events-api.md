# Events API

[Documentation](../README.md) > [API Reference](./README.md) > Events API

---

This reference documents all Tauri events emitted by SpiritStream's backend for real-time communication with the frontend.

---

## Overview

Events provide real-time updates from the Rust backend to the React frontend. Unlike commands (request/response), events are pushed from the backend whenever state changes.

```typescript
import { listen, UnlistenFn } from '@tauri-apps/api/event';

// Listen for an event
const unlisten = await listen<StreamStats>('stream_stats', (event) => {
  console.log('Stats:', event.payload);
});

// Stop listening
unlisten();
```

---

## Stream Events

### stream_stats

Emitted every second during active streaming with encoding statistics.

**Payload:**

```typescript
interface StreamStats {
  groupId: string;      // Output group ID
  frame: number;        // Current frame number
  fps: number;          // Current frames per second
  bitrate: number;      // Current bitrate in kbps
  speed: number;        // Encoding speed (1.0 = real-time)
  size: number;         // Total bytes written
  time: number;         // Elapsed time in seconds
  droppedFrames: number; // Dropped frame count
  dupFrames: number;    // Duplicated frame count
}
```

**Example Payload:**

```json
{
  "groupId": "output-group-1",
  "frame": 3600,
  "fps": 59.94,
  "bitrate": 6012.5,
  "speed": 1.02,
  "size": 45000000,
  "time": 60.0,
  "droppedFrames": 0,
  "dupFrames": 0
}
```

**Frontend Usage:**

```typescript
import { listen } from '@tauri-apps/api/event';
import { useStreamStore } from '@/stores/streamStore';

function useStreamStats() {
  const updateStats = useStreamStore((s) => s.updateStats);

  useEffect(() => {
    let unlisten: (() => void) | undefined;

    listen<StreamStats>('stream_stats', (event) => {
      updateStats(event.payload);
    }).then((fn) => {
      unlisten = fn;
    });

    return () => unlisten?.();
  }, [updateStats]);
}
```

**Backend Emission:**

```rust
// Emitted from FFmpeg stderr parser
app_handle.emit("stream_stats", StreamStats {
    group_id: group_id.clone(),
    frame: parsed.frame,
    fps: parsed.fps,
    bitrate: parsed.bitrate,
    speed: parsed.speed,
    size: parsed.size,
    time: parsed.time,
    dropped_frames: parsed.drop,
    dup_frames: parsed.dup,
})?;
```

---

### stream_ended

Emitted when a stream stops (normally or due to error).

**Payload:**

```typescript
interface StreamEndedPayload {
  groupId: string;       // Output group ID
  exitCode: number;      // FFmpeg exit code (0 = normal)
  duration: number;      // Total stream duration in seconds
}
```

**Exit Codes:**

| Code | Meaning |
|------|---------|
| 0 | Normal termination |
| 1 | Generic error |
| 255 | Killed by user (SIGKILL) |

**Frontend Usage:**

```typescript
listen<StreamEndedPayload>('stream_ended', (event) => {
  const { groupId, exitCode } = event.payload;

  if (exitCode === 0) {
    toast.info(`Stream ${groupId} ended`);
  } else {
    toast.warning(`Stream ${groupId} ended unexpectedly`);
  }

  streamStore.getState().removeStream(groupId);
});
```

---

### stream_error

Emitted when a stream encounters an error.

**Payload:**

```typescript
interface StreamError {
  groupId: string;     // Output group ID
  message: string;     // Error message
  code?: string;       // Error code (optional)
  target?: string;     // Affected target (optional)
}
```

**Common Error Messages:**

| Message | Cause |
|---------|-------|
| `"Connection refused"` | Server unavailable |
| `"Connection timed out"` | Network timeout |
| `"Authentication failed"` | Invalid stream key |
| `"Server full"` | Ingest capacity exceeded |

**Frontend Usage:**

```typescript
listen<StreamError>('stream_error', (event) => {
  const { groupId, message, target } = event.payload;

  toast.error(`Stream error: ${message}`, {
    description: target ? `Target: ${target}` : undefined,
  });

  // Update stream status
  streamStore.getState().setStreamError(groupId, message);
});
```

---

### stream_target_status

Emitted when an individual target's status changes.

**Payload:**

```typescript
interface TargetStatus {
  groupId: string;
  targetId: string;
  status: 'connecting' | 'live' | 'error' | 'disconnected';
  message?: string;
}
```

**Frontend Usage:**

```typescript
listen<TargetStatus>('stream_target_status', (event) => {
  const { groupId, targetId, status } = event.payload;
  streamStore.getState().setTargetStatus(groupId, targetId, status);
});
```

---

## System Events

### ffmpeg_download_progress

Emitted during FFmpeg download operations.

**Payload:**

```typescript
interface DownloadProgress {
  downloaded: number;    // Bytes downloaded
  total: number;         // Total bytes
  percentage: number;    // 0-100
}
```

**Frontend Usage:**

```typescript
listen<DownloadProgress>('ffmpeg_download_progress', (event) => {
  setProgress(event.payload.percentage);
});
```

---

### ffmpeg_ready

Emitted when FFmpeg is available and validated.

**Payload:**

```typescript
interface FFmpegReady {
  path: string;         // Path to FFmpeg binary
  version: string;      // FFmpeg version string
}
```

---

### settings_changed

Emitted when settings are modified.

**Payload:**

```typescript
interface SettingsChanged {
  key: string;          // Setting key that changed
  value: unknown;       // New value
}
```

**Frontend Usage:**

```typescript
listen<SettingsChanged>('settings_changed', (event) => {
  if (event.payload.key === 'theme') {
    themeStore.getState().setTheme(event.payload.value as string);
  }
});
```

---

## Log Events

### log_entry

Emitted for each new log entry.

**Payload:**

```typescript
interface LogEntry {
  timestamp: string;    // ISO timestamp
  level: 'debug' | 'info' | 'warn' | 'error';
  message: string;
  source?: string;      // Component source
}
```

**Frontend Usage:**

```typescript
listen<LogEntry>('log_entry', (event) => {
  logStore.getState().addEntry(event.payload);
});
```

---

## Event Patterns

### React Hook Pattern

```typescript
// hooks/useTauriEvent.ts
import { useEffect, useCallback } from 'react';
import { listen, UnlistenFn } from '@tauri-apps/api/event';

export function useTauriEvent<T>(
  event: string,
  handler: (payload: T) => void
) {
  const stableHandler = useCallback(handler, [handler]);

  useEffect(() => {
    let unlisten: UnlistenFn | undefined;

    listen<T>(event, (e) => stableHandler(e.payload))
      .then((fn) => {
        unlisten = fn;
      });

    return () => {
      unlisten?.();
    };
  }, [event, stableHandler]);
}
```

**Usage:**

```typescript
function StreamMonitor() {
  const updateStats = useStreamStore((s) => s.updateStats);

  useTauriEvent<StreamStats>('stream_stats', updateStats);

  return <StatsDisplay />;
}
```

### Store Integration Pattern

```typescript
// stores/streamStore.ts
export const useStreamStore = create<StreamState>((set, get) => ({
  // ... state ...

  initEventListeners: async () => {
    const unlistenStats = await listen<StreamStats>('stream_stats', (e) => {
      get().updateStats(e.payload);
    });

    const unlistenEnded = await listen<StreamEndedPayload>('stream_ended', (e) => {
      get().handleStreamEnded(e.payload);
    });

    const unlistenError = await listen<StreamError>('stream_error', (e) => {
      get().handleStreamError(e.payload);
    });

    return () => {
      unlistenStats();
      unlistenEnded();
      unlistenError();
    };
  },
}));
```

### App-Level Initialization

```typescript
// App.tsx
function App() {
  useEffect(() => {
    let cleanup: (() => void) | undefined;

    useStreamStore.getState().initEventListeners()
      .then((fn) => {
        cleanup = fn;
      });

    return () => cleanup?.();
  }, []);

  return <AppShell />;
}
```

---

## Backend Event Emission

### From Rust

```rust
use tauri::AppHandle;

// Emit to all windows
fn emit_stats(app: &AppHandle, stats: StreamStats) -> Result<(), tauri::Error> {
    app.emit("stream_stats", stats)
}

// Emit to specific window
fn emit_to_window(app: &AppHandle, stats: StreamStats) -> Result<(), tauri::Error> {
    if let Some(window) = app.get_webview_window("main") {
        window.emit("stream_stats", stats)?;
    }
    Ok(())
}
```

### Event Names Convention

| Pattern | Example | Purpose |
|---------|---------|---------|
| `{noun}_{action}` | `stream_stats` | State updates |
| `{noun}_{past_verb}` | `stream_ended` | Completed actions |
| `{noun}_error` | `stream_error` | Error notifications |

---

## Type Definitions

```typescript
// types/events.ts
export interface StreamStats {
  groupId: string;
  frame: number;
  fps: number;
  bitrate: number;
  speed: number;
  size: number;
  time: number;
  droppedFrames: number;
  dupFrames: number;
}

export interface StreamEndedPayload {
  groupId: string;
  exitCode: number;
  duration: number;
}

export interface StreamError {
  groupId: string;
  message: string;
  code?: string;
  target?: string;
}

export interface TargetStatus {
  groupId: string;
  targetId: string;
  status: 'connecting' | 'live' | 'error' | 'disconnected';
  message?: string;
}

export interface DownloadProgress {
  downloaded: number;
  total: number;
  percentage: number;
}

export interface LogEntry {
  timestamp: string;
  level: 'debug' | 'info' | 'warn' | 'error';
  message: string;
  source?: string;
}
```

---

## Best Practices

### Do

1. Clean up listeners in useEffect cleanup
2. Use typed event handlers
3. Centralize event listeners in stores
4. Handle missing/malformed payloads
5. Throttle high-frequency events in UI

### Don't

1. Create listeners without cleanup
2. Listen to same event multiple times
3. Block UI on event processing
4. Assume event order
5. Store event listeners as component state

---

**Related:** [Commands API](./01-commands-api.md) | [Types Reference](./03-types-reference.md) | [Tauri Integration](../03-frontend/04-tauri-integration.md)

