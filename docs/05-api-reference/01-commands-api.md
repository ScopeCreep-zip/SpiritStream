# Commands API

[Documentation](../README.md) > [API Reference](./README.md) > Commands API

---

This reference documents all Tauri commands available in SpiritStream. Commands are the primary way the frontend communicates with the Rust backend.

---

## Overview

Commands are invoked from the frontend using Tauri's `invoke` function. They're asynchronous—even simple queries return Promises because they cross the IPC boundary to Rust and back.

```typescript
import { invoke } from '@tauri-apps/api/core';

// Example: Load a profile
const profile = await invoke<Profile>('load_profile', {
  name: 'my-profile',
  password: 'optional-password'
});
```

### Common Workflows

**App startup:**
1. `get_settings()` → Load user preferences
2. `get_video_encoders()` → Cache available encoders
3. `get_all_profiles()` → Populate profile list
4. If `settings.lastProfile`: `load_profile()` → Restore last session

**Starting a stream:**
1. `is_profile_encrypted()` → Check if password needed
2. `load_profile()` → Get full profile data
3. For each output group: `start_stream()` → Launch FFmpeg processes
4. Listen for `stream_stats` events → Update UI with real-time data

**Saving changes:**
1. Update local state (React/Zustand)
2. `save_profile()` → Persist to disk
3. Show success/error feedback

---

## Profile Commands

Profile commands handle CRUD operations for streaming configurations. The typical flow is: list profiles → check encryption → load with password if needed → edit in UI → save back.

### get_all_profiles

Returns a list of all profile names.

**Signature:**
```rust
#[tauri::command]
pub async fn get_all_profiles(
    state: State<'_, ProfileManager>
) -> Result<Vec<String>, String>
```

**Parameters:** None

**Returns:** `Vec<String>` - List of profile names

**Frontend Usage:**
```typescript
const profiles = await invoke<string[]>('get_all_profiles');
// ["gaming", "podcast", "irl-stream"]
```

---

### load_profile

Loads a profile by name, optionally decrypting with password.

**Signature:**
```rust
#[tauri::command]
pub async fn load_profile(
    name: String,
    password: Option<String>,
    state: State<'_, ProfileManager>
) -> Result<Profile, String>
```

**Parameters:**
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `name` | `String` | Yes | Profile name |
| `password` | `Option<String>` | No | Decryption password |

**Returns:** `Profile` - The loaded profile

**Frontend Usage:**
```typescript
// Unencrypted profile
const profile = await invoke<Profile>('load_profile', { name: 'gaming' });

// Encrypted profile
const profile = await invoke<Profile>('load_profile', {
  name: 'private',
  password: 'my-password'
});
```

**Errors:**
- `"Profile not found"` - No profile with that name
- `"Decryption failed"` - Wrong password or corrupted file

---

### save_profile

Saves a profile, optionally encrypting with password.

**Signature:**
```rust
#[tauri::command]
pub async fn save_profile(
    profile: Profile,
    password: Option<String>,
    state: State<'_, ProfileManager>
) -> Result<(), String>
```

**Parameters:**
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `profile` | `Profile` | Yes | Profile to save |
| `password` | `Option<String>` | No | Encryption password |

**Returns:** `()` - Nothing on success

**Frontend Usage:**
```typescript
await invoke('save_profile', {
  profile: {
    id: 'uuid-here',
    name: 'my-profile',
    incomingUrl: 'rtmp://localhost:1935/live/stream',
    outputGroups: [...]
  },
  password: 'optional-password'
});
```

---

### delete_profile

Deletes a profile by name.

**Signature:**
```rust
#[tauri::command]
pub async fn delete_profile(
    name: String,
    state: State<'_, ProfileManager>
) -> Result<(), String>
```

**Parameters:**
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `name` | `String` | Yes | Profile name to delete |

**Returns:** `()` - Nothing on success

**Frontend Usage:**
```typescript
await invoke('delete_profile', { name: 'old-profile' });
```

---

### is_profile_encrypted

Checks if a profile is password-protected.

**Signature:**
```rust
#[tauri::command]
pub async fn is_profile_encrypted(
    name: String,
    state: State<'_, ProfileManager>
) -> Result<bool, String>
```

**Parameters:**
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `name` | `String` | Yes | Profile name |

**Returns:** `bool` - `true` if encrypted

**Frontend Usage:**
```typescript
const isEncrypted = await invoke<boolean>('is_profile_encrypted', {
  name: 'my-profile'
});
if (isEncrypted) {
  // Show password dialog
}
```

---

## Stream Commands

Stream commands control FFmpeg processes—starting, stopping, and querying their status. Each output group runs as a separate FFmpeg process, so you can start/stop groups independently.

**Important:** These commands launch external processes. Unlike profile commands that just read/write files, stream commands have real-time effects. Always handle errors gracefully and provide clear feedback when streams fail to start.

### start_stream

Starts streaming for an output group. This spawns an FFmpeg process that runs until `stop_stream` is called or an error occurs.

**Signature:**
```rust
#[tauri::command]
pub async fn start_stream(
    group: OutputGroup,
    incoming_url: String,
    state: State<'_, FFmpegHandler>,
    app: AppHandle
) -> Result<u32, String>
```

**Parameters:**
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `group` | `OutputGroup` | Yes | Output group config |
| `incoming_url` | `String` | Yes | RTMP input URL |

**Returns:** `u32` - Process ID of FFmpeg

**Frontend Usage:**
```typescript
const pid = await invoke<number>('start_stream', {
  group: outputGroup,
  incomingUrl: 'rtmp://localhost:1935/live/stream'
});
```

**Events Emitted:**
- `stream_stats` - Real-time statistics
- `stream_ended` - When stream stops
- `stream_error` - On error

---

### stop_stream

Stops a specific output group.

**Signature:**
```rust
#[tauri::command]
pub async fn stop_stream(
    group_id: String,
    state: State<'_, FFmpegHandler>
) -> Result<(), String>
```

**Parameters:**
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `group_id` | `String` | Yes | Output group ID |

**Returns:** `()` - Nothing on success

**Frontend Usage:**
```typescript
await invoke('stop_stream', { groupId: 'output-group-1' });
```

---

### stop_all_streams

Stops all active streams.

**Signature:**
```rust
#[tauri::command]
pub async fn stop_all_streams(
    state: State<'_, FFmpegHandler>
) -> Result<(), String>
```

**Parameters:** None

**Returns:** `()` - Nothing on success

**Frontend Usage:**
```typescript
await invoke('stop_all_streams');
```

---

### get_active_stream_count

Returns number of active streams.

**Signature:**
```rust
#[tauri::command]
pub fn get_active_stream_count(
    state: State<'_, FFmpegHandler>
) -> usize
```

**Parameters:** None

**Returns:** `usize` - Count of active streams

**Frontend Usage:**
```typescript
const count = await invoke<number>('get_active_stream_count');
```

---

### is_group_streaming

Checks if an output group is currently streaming.

**Signature:**
```rust
#[tauri::command]
pub fn is_group_streaming(
    group_id: String,
    state: State<'_, FFmpegHandler>
) -> bool
```

**Parameters:**
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `group_id` | `String` | Yes | Output group ID |

**Returns:** `bool` - `true` if streaming

**Frontend Usage:**
```typescript
const isLive = await invoke<boolean>('is_group_streaming', {
  groupId: 'main-output'
});
```

---

### toggle_stream_target

Enables or disables a stream target and restarts the group.

**Signature:**
```rust
#[tauri::command]
pub async fn toggle_stream_target(
    target_id: String,
    enabled: bool,
    group: OutputGroup,
    incoming_url: String,
    state: State<'_, FFmpegHandler>,
    app: AppHandle
) -> Result<u32, String>
```

**Parameters:**
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `target_id` | `String` | Yes | Target to toggle |
| `enabled` | `bool` | Yes | Enable or disable |
| `group` | `OutputGroup` | Yes | Parent output group |
| `incoming_url` | `String` | Yes | RTMP input URL |

**Returns:** `u32` - New process ID

**Frontend Usage:**
```typescript
const pid = await invoke<number>('toggle_stream_target', {
  targetId: 'twitch-target',
  enabled: false,
  group: outputGroup,
  incomingUrl: 'rtmp://localhost:1935/live/stream'
});
```

---

## System Commands

System commands query hardware capabilities and manage FFmpeg installation. Call these once at startup and cache the results—encoder detection involves spawning FFmpeg to probe the system, so it's not instant.

### get_video_encoders

Returns available video encoders. Call this once when the app starts and use the result to populate encoder dropdowns. The list depends on installed hardware (NVIDIA GPU for NVENC, Intel CPU for QSV, etc.).

**Signature:**
```rust
#[tauri::command]
pub async fn get_video_encoders(
    state: State<'_, FFmpegHandler>
) -> Result<Vec<EncoderInfo>, String>
```

**Returns:** `Vec<EncoderInfo>` - Available encoders

**Frontend Usage:**
```typescript
interface EncoderInfo {
  name: string;
  displayName: string;
  type: 'software' | 'hardware';
}

const encoders = await invoke<EncoderInfo[]>('get_video_encoders');
// [
//   { name: "libx264", displayName: "x264 (CPU)", type: "software" },
//   { name: "h264_nvenc", displayName: "NVENC (NVIDIA)", type: "hardware" }
// ]
```

---

### get_audio_encoders

Returns available audio encoders.

**Signature:**
```rust
#[tauri::command]
pub async fn get_audio_encoders(
    state: State<'_, FFmpegHandler>
) -> Result<Vec<EncoderInfo>, String>
```

**Returns:** `Vec<EncoderInfo>` - Available audio encoders

**Frontend Usage:**
```typescript
const encoders = await invoke<EncoderInfo[]>('get_audio_encoders');
```

---

### get_bundled_ffmpeg_path

Returns path to bundled FFmpeg if available.

**Signature:**
```rust
#[tauri::command]
pub fn get_bundled_ffmpeg_path(
    app: AppHandle
) -> Option<String>
```

**Returns:** `Option<String>` - Path or null

**Frontend Usage:**
```typescript
const path = await invoke<string | null>('get_bundled_ffmpeg_path');
```

---

### download_ffmpeg

Downloads FFmpeg to the app directory.

**Signature:**
```rust
#[tauri::command]
pub async fn download_ffmpeg(
    app: AppHandle
) -> Result<String, String>
```

**Returns:** `String` - Path to downloaded FFmpeg

**Events Emitted:**
- `ffmpeg_download_progress` - Download progress updates

**Frontend Usage:**
```typescript
const path = await invoke<string>('download_ffmpeg');
```

---

## Settings Commands

Settings commands manage app-wide preferences that persist across profiles. These are separate from profile data—changing your theme doesn't require re-saving your streaming configuration.

### get_settings

Returns application settings. Call this early in app initialization to restore the user's preferences (theme, language, etc.).

**Signature:**
```rust
#[tauri::command]
pub async fn get_settings(
    state: State<'_, SettingsManager>
) -> Result<Settings, String>
```

**Returns:** `Settings` - Current settings

**Frontend Usage:**
```typescript
const settings = await invoke<Settings>('get_settings');
```

---

### save_settings

Saves application settings.

**Signature:**
```rust
#[tauri::command]
pub async fn save_settings(
    settings: Settings,
    state: State<'_, SettingsManager>
) -> Result<(), String>
```

**Parameters:**
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `settings` | `Settings` | Yes | Settings to save |

**Frontend Usage:**
```typescript
await invoke('save_settings', { settings: newSettings });
```

---

### get_theme

Returns current theme.

**Signature:**
```rust
#[tauri::command]
pub fn get_theme(
    state: State<'_, ThemeManager>
) -> String
```

**Returns:** `String` - "light", "dark", or "system"

---

### set_theme

Sets the theme.

**Signature:**
```rust
#[tauri::command]
pub async fn set_theme(
    theme: String,
    state: State<'_, ThemeManager>,
    app: AppHandle
) -> Result<(), String>
```

**Parameters:**
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `theme` | `String` | Yes | "light", "dark", or "system" |

---

## Error Handling

All commands return `Result<T, String>` where errors are string messages. The Tauri IPC layer converts Rust `Err` values into rejected Promises on the JavaScript side.

**Best practice:** Wrap `invoke` calls in try/catch and show user-friendly feedback. The error strings are designed to be displayable directly:

```typescript
try {
  const profile = await invoke<Profile>('load_profile', { name: 'test' });
} catch (error) {
  // error is a string message
  console.error('Failed to load profile:', error);
}
```

### Common Errors

These errors are returned as strings. The frontend should display them to the user and offer appropriate recovery actions:

| Error | Cause | Recovery |
|-------|-------|----------|
| `"Profile not found"` | Profile file doesn't exist | Show profile picker, user may have deleted file manually |
| `"Decryption failed"` | Wrong password or corrupted file | Prompt for password retry; after 3 attempts, offer password reset |
| `"FFmpeg not found"` | FFmpeg not installed/bundled | Trigger `download_ffmpeg()` or show manual install instructions |
| `"Stream already running"` | Group already has active FFmpeg | Call `stop_stream()` first, or show "already live" status |
| `"Invalid configuration"` | Profile data is malformed | Validate profile before saving; offer to reset to defaults |
| `"No stream targets"` | Output group has no destinations | Prompt user to add at least one stream target |

---

## Type Definitions

These TypeScript interfaces match the Rust structs. The `camelCase` naming on the TypeScript side maps to `snake_case` on the Rust side automatically via serde's `rename_all` attribute.

### Profile

```typescript
interface Profile {
  id: string;
  name: string;
  incomingUrl: string;
  outputGroups: OutputGroup[];
}
```

### OutputGroup

```typescript
interface OutputGroup {
  id: string;
  name: string;
  video: VideoSettings;
  audio: AudioSettings;
  container: ContainerSettings;
  streamTargets: StreamTarget[];
}
```

### StreamTarget

```typescript
interface StreamTarget {
  id: string;
  platform: string;
  name: string;
  url: string;
  streamKey: string;
}
```

See [Types Reference](./03-types-reference.md) for complete type definitions.

---

**Related:** [Events API](./02-events-api.md) | [Types Reference](./03-types-reference.md) | [Error Handling](./04-error-handling.md)

