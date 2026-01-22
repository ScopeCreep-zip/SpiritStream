# Types Reference

[Documentation](../README.md) > [API Reference](./README.md) > Types Reference

---

This reference documents all TypeScript type definitions used in SpiritStream's frontend for type-safe communication with the Rust backend.

## Understanding the Type System

SpiritStream uses a **shared type contract** between the TypeScript frontend and Rust backend. Types defined here mirror Rust structs with automatic serialization via `serde`. This ensures compile-time type safety across the IPC boundary—if a type changes in Rust, the TypeScript definition must be updated to match, or the compiler will catch the mismatch.

### Type Hierarchy

The types follow a hierarchical structure that mirrors how streaming configurations work:

```
Profile (top-level container)
├── OutputGroup[] (encoding configurations)
│   ├── VideoSettings (video encoding params)
│   ├── AudioSettings (audio encoding params)
│   ├── ContainerSettings (output format)
│   └── StreamTarget[] (RTMP destinations)
```

A single **Profile** can contain multiple **OutputGroups**, allowing you to stream the same input to different platforms at different quality levels simultaneously. Each **OutputGroup** defines encoding settings and contains one or more **StreamTargets**—the actual RTMP endpoints.

---

## Core Models

### Profile

The Profile is the top-level configuration entity that users create, save, and load. It represents a complete streaming setup: where to receive video (the incoming RTMP URL) and how to distribute it (via output groups).

Profiles are persisted as JSON files in the user's data directory. When a password is provided during save/load, the entire profile is encrypted using AES-256-GCM—this protects stream keys at rest.

```typescript
interface Profile {
  id: string;              // UUID identifier
  name: string;            // User-friendly name
  incomingUrl: string;     // RTMP input URL
  outputGroups: OutputGroup[];
}
```

**Example:**

```json
{
  "id": "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
  "name": "Gaming Stream",
  "incomingUrl": "rtmp://localhost:1935/live/stream",
  "outputGroups": [...]
}
```

---

### OutputGroup

An OutputGroup bundles encoding settings with a list of destinations. The key insight is that **all targets in a group share the same encoding**—FFmpeg encodes once and pushes to multiple outputs. This is efficient but means you can't send 1080p to YouTube and 720p to Twitch in the same group; you'd need separate groups for different quality levels.

Why groups? Streaming platforms have different requirements. YouTube accepts up to 4K, Twitch recommends 1080p60 at 6000kbps, and mobile-focused platforms may want 720p. By organizing targets into groups by quality tier, you can serve each platform optimally while only encoding each quality level once.

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

**Example:**

```json
{
  "id": "group-1",
  "name": "Main Output",
  "video": {
    "codec": "libx264",
    "width": 1920,
    "height": 1080,
    "bitrate": 6000,
    "fps": 60
  },
  "audio": {
    "codec": "aac",
    "bitrate": 160,
    "sampleRate": 48000,
    "channels": 2
  },
  "container": {
    "format": "flv"
  },
  "streamTargets": [...]
}
```

---

### VideoSettings

VideoSettings control how the video stream is encoded. The most impactful decisions are **codec** and **bitrate**—they determine quality, compatibility, and CPU/GPU load.

The `codec: "copy"` option is special: it skips encoding entirely, passing through the original video unchanged. Use this when your input is already encoded appropriately (e.g., OBS outputs H.264 at 6000kbps and you want to relay that exact stream). Copy mode uses minimal CPU but means you can't change resolution or bitrate.

Optional fields like `preset` and `profile` fine-tune the encoder. Presets trade encoding speed for quality—`veryfast` uses less CPU but produces larger files than `medium`. Profiles affect decoder compatibility: `baseline` works on older devices, `high` enables better compression but requires modern decoders.

```typescript
interface VideoSettings {
  codec: string;           // FFmpeg codec (libx264, h264_nvenc, copy)
  width: number;           // Output width in pixels
  height: number;          // Output height in pixels
  bitrate: number;         // Bitrate in kbps
  fps: number;             // Frames per second
  preset?: string;         // Encoder preset (veryfast, medium, etc.)
  profile?: string;        // H.264 profile (baseline, main, high)
  keyframeInterval?: number; // Keyframe interval in seconds
}
```

**Codec Values:**

| Value | Description |
|-------|-------------|
| `copy` | Passthrough (no re-encoding) |
| `libx264` | Software x264 encoder |
| `h264_nvenc` | NVIDIA hardware encoder |
| `h264_qsv` | Intel QuickSync encoder |
| `h264_amf` | AMD hardware encoder |

---

### AudioSettings

Audio encoding is simpler than video—AAC at 128-320kbps covers most streaming needs. Unlike video where hardware encoders provide huge speedups, audio encoding is lightweight on CPU, so software encoding is fine.

Like video, `codec: "copy"` passes audio through unchanged. This is useful when your input already has proper AAC audio. The `sampleRate` should typically be 44100 or 48000 Hz—most streaming platforms require 48000 for video content.

```typescript
interface AudioSettings {
  codec: string;           // FFmpeg codec (aac, copy)
  bitrate: number;         // Bitrate in kbps
  sampleRate: number;      // Sample rate in Hz
  channels: number;        // Channel count (1=mono, 2=stereo)
}
```

---

### ContainerSettings

The container format wraps video and audio streams for transport. For RTMP streaming, this is always **FLV** (Flash Video)—it's what the RTMP protocol expects. While FLV is an older format, it remains the standard for live streaming due to RTMP's ubiquity.

This type exists for future extensibility. If SpiritStream adds SRT or RIST protocol support, different container formats would be needed.

```typescript
interface ContainerSettings {
  format: string;          // Container format (flv for RTMP)
}
```

---

### StreamTarget

A StreamTarget represents a single RTMP destination—where your stream actually goes. The `url` is the RTMP server endpoint (e.g., `rtmp://live.twitch.tv/app`), and the `streamKey` is your authentication credential.

**Security note:** Stream keys are sensitive credentials. They're stored encrypted in profile files and should never be logged or exposed in the UI. The `service` field enables platform-specific behavior (URL building, log redaction) based on the platform registry.

```typescript
interface StreamTarget {
  id: string;              // UUID identifier
  service: Platform;       // Platform type (auto-generated from registry)
  name: string;            // Display name
  url: string;             // RTMP server URL
  streamKey: string;       // Stream key (sensitive)
}

// Platform is auto-generated from data/streaming-platforms.json
// Common values: 'youtube', 'twitch', 'kick', 'facebook_live', 'custom'
// 80+ platforms supported - see platform registry for full list
type Platform = string;  // Generated enum includes all registered platforms
```

**Example:**

```json
{
  "id": "target-1",
  "service": "twitch",
  "name": "Twitch Gaming",
  "url": "rtmp://live.twitch.tv/app",
  "streamKey": "live_xxxxxxxx_xxxxxxxxxx"
}
```

### StreamKeyPlacement

Platforms handle stream keys differently in their RTMP URLs. This enum (used on the Rust backend) determines how URLs are constructed:

```typescript
// Backend enum - frontend receives constructed URLs
enum StreamKeyPlacement {
  Append,        // Key appended: rtmp://server/app/{key}
  InUrlTemplate, // Key in template: rtmp://server/{stream_key}/live
}
```

---

## Settings Types

Application-level preferences that persist across sessions. Unlike Profile settings which configure streaming, these control the application itself.

### Settings

The Settings object holds user preferences for the application. These are stored separately from profiles in a dedicated settings file, so changing settings doesn't affect profile configurations.

The `ffmpegPath` field is optional—if not set, SpiritStream searches standard locations (PATH, bundled binary, known install directories). Users only need to set this if FFmpeg is installed in a non-standard location.

```typescript
interface Settings {
  language: string;        // UI language (en, es, de, etc.)
  theme: Theme;            // Theme preference
  startMinimized: boolean;
  showNotifications: boolean;
  ffmpegPath?: string;     // Custom FFmpeg path
  autoDownloadFfmpeg: boolean;
  lastProfile?: string;    // Last used profile name
}

type Theme = 'light' | 'dark' | 'system';
```

---

## Encoder Types

Encoder types represent the video/audio codecs available on the user's system. These are detected at runtime by querying FFmpeg, since available encoders depend on installed hardware (NVIDIA GPU, Intel iGPU, etc.) and FFmpeg's build configuration.

### EncoderInfo

EncoderInfo describes a single available encoder. The `encoderType` distinction matters for UI presentation—hardware encoders are generally preferred when available because they offload work from the CPU, enabling higher quality settings or freeing CPU for other tasks.

```typescript
interface EncoderInfo {
  name: string;            // FFmpeg encoder name
  displayName: string;     // User-friendly name
  encoderType: EncoderType;
}

type EncoderType = 'software' | 'hardware';
```

**Example Response:**

```json
[
  { "name": "libx264", "displayName": "x264 (CPU)", "encoderType": "software" },
  { "name": "h264_nvenc", "displayName": "NVENC (NVIDIA)", "encoderType": "hardware" }
]
```

---

## Stream Types

Stream types capture the runtime state of active streams. These are updated continuously while streaming and are used to display real-time statistics in the UI.

### StreamStats

StreamStats provides real-time metrics from FFmpeg. These are parsed from FFmpeg's stderr output, which emits progress lines like `frame=1000 fps=60.0 bitrate=6000kbps`. The values help users monitor stream health:

- **fps** below target indicates the encoder can't keep up (CPU/GPU overload)
- **droppedFrames** above zero suggests network congestion or encoding lag
- **speed** below 1.0 means encoding is slower than real-time—the stream will fall behind

```typescript
interface StreamStats {
  groupId: string;         // Output group ID
  frame: number;           // Current frame number
  fps: number;             // Current FPS
  bitrate: number;         // Current bitrate (kbps)
  speed: number;           // Encoding speed (1.0 = real-time)
  size: number;            // Total bytes written
  time: number;            // Elapsed time (seconds)
  droppedFrames: number;   // Dropped frame count
  dupFrames: number;       // Duplicated frame count
}
```

---

### StreamStatus

A simple state machine for stream lifecycle. The transitions are: `offline` → `connecting` → `live` → `offline` (or `error` at any point). The UI uses this to show appropriate status badges and enable/disable controls.

```typescript
type StreamStatus = 'offline' | 'connecting' | 'live' | 'error';
```

---

### StreamError

When FFmpeg encounters an error—connection refused, invalid stream key, network timeout—it exits with a non-zero code and error message. StreamError captures this information for display to the user. The optional `target` field identifies which specific RTMP destination failed when a group has multiple targets.

```typescript
interface StreamError {
  groupId: string;
  message: string;
  code?: string;
  target?: string;         // Affected target ID
}
```

---

## Event Types

### StreamEndedPayload

Stream termination event.

```typescript
interface StreamEndedPayload {
  groupId: string;
  exitCode: number;        // Process exit code
  duration: number;        // Total duration (seconds)
}
```

---

### TargetStatus

Individual target status update.

```typescript
interface TargetStatus {
  groupId: string;
  targetId: string;
  status: StreamStatus;
  message?: string;
}
```

---

### DownloadProgress

FFmpeg download progress.

```typescript
interface DownloadProgress {
  downloaded: number;      // Bytes downloaded
  total: number;           // Total bytes
  percentage: number;      // 0-100
}
```

---

### LogEntry

Application log entry.

```typescript
interface LogEntry {
  timestamp: string;       // ISO timestamp
  level: LogLevel;
  message: string;
  source?: string;         // Component source
}

type LogLevel = 'debug' | 'info' | 'warn' | 'error';
```

---

## Store Types

Store types define the shape of Zustand stores—the frontend's state management layer. Each store combines state (data) with actions (functions to modify that data). The stores communicate with the Rust backend via Tauri commands and update the UI reactively when state changes.

### ProfileState

The ProfileState store manages profile CRUD operations and tracks the currently loaded profile. The `loading` and `saving` flags enable UI feedback during async operations. Importantly, `profiles` only contains names (strings), not full Profile objects—full profiles are loaded on-demand to avoid memory overhead.

```typescript
interface ProfileState {
  // State
  profiles: string[];      // Profile names
  current: Profile | null; // Loaded profile
  loading: boolean;
  saving: boolean;
  error: string | null;

  // Actions
  loadProfiles: () => Promise<void>;
  loadProfile: (name: string, password?: string) => Promise<void>;
  saveProfile: (password?: string) => Promise<void>;
  deleteProfile: (name: string) => Promise<void>;
  updateProfile: (updates: Partial<Profile>) => void;
}
```

---

### StreamState

The StreamState store manages active streaming sessions. It tracks which output groups are currently streaming (via `activeStreams` mapping group IDs to process IDs), real-time statistics per group, and any errors that occur. The `isStreaming` convenience boolean is true when any stream is active.

```typescript
interface StreamState {
  // State
  activeStreams: Map<string, number>;  // groupId -> pid
  stats: Map<string, StreamStats>;
  errors: Map<string, StreamError>;
  isStreaming: boolean;

  // Actions
  startStream: (group: OutputGroup, incomingUrl: string) => Promise<void>;
  stopStream: (groupId: string) => Promise<void>;
  stopAllStreams: () => Promise<void>;
  updateStats: (stats: StreamStats) => void;
  setStreamError: (groupId: string, error: StreamError) => void;
}
```

---

### ThemeState

Theme management with system preference support. The `theme` field is the user's preference (including 'system'), while `resolved` is the actual applied theme after resolving system preference. This separation allows the UI to show "System" in settings while applying the correct light/dark theme.

```typescript
interface ThemeState {
  theme: Theme;
  resolved: 'light' | 'dark';  // Actual applied theme
  setTheme: (theme: Theme) => Promise<void>;
  toggleTheme: () => void;
  initTheme: () => Promise<void>;
}
```

---

## Utility Types

Helper types that make the codebase more maintainable and type-safe.

### Result Types

Tauri commands return values directly on success and throw string errors on failure. This differs from Rust's `Result<T, E>` pattern—the Tauri bridge converts `Err(e)` to a thrown exception. Frontend code uses try/catch to handle errors.

```typescript
type CommandResult<T> = T;  // Success
// Errors throw as strings
```

---

### Form Types

Form types represent the data collected from UI forms before conversion to full model types. They often use simpler formats—for example, resolution as a string `"1920x1080"` rather than separate width/height fields—because that matches how users input data.

```typescript
interface ProfileFormData {
  name: string;
  incomingUrl: string;
  resolution: string;      // "1920x1080"
  fps: number;
  bitrate: number;
}

interface TargetFormData {
  platform: Platform;
  name: string;
  url: string;
  streamKey: string;
}
```

---

### ID Types

For type safety on identifiers.

```typescript
type ProfileId = string;
type OutputGroupId = string;
type StreamTargetId = string;
```

---

## Rust Equivalents

### Type Mapping

| TypeScript | Rust |
|------------|------|
| `string` | `String` |
| `number` | `u32`, `f64`, `usize` |
| `boolean` | `bool` |
| `T \| null` | `Option<T>` |
| `T[]` | `Vec<T>` |
| `Record<K, V>` | `HashMap<K, V>` |

### Naming Convention

| TypeScript | Rust |
|------------|------|
| `camelCase` | `snake_case` |
| `incomingUrl` | `incoming_url` |
| `streamTargets` | `stream_targets` |

Serde handles conversion automatically with `#[serde(rename_all = "camelCase")]`.

---

## Type Guards

Type guards are runtime checks that narrow TypeScript types. They're essential when working with data from external sources (user input, API responses) where TypeScript can't guarantee the type at compile time. These guards validate data before it's used, preventing runtime errors.

### Platform Check

```typescript
function isPlatform(value: string): value is Platform {
  return ['youtube', 'twitch', 'kick', 'facebook', 'custom'].includes(value);
}
```

### Status Check

```typescript
function isLive(status: StreamStatus): boolean {
  return status === 'live';
}

function isError(status: StreamStatus): boolean {
  return status === 'error';
}
```

---

## Default Values

Default values serve two purposes: providing sensible starting points for new entities and ensuring the application has valid state even before user configuration. These defaults are chosen to work "out of the box" with minimal setup.

### Default Profile

The default profile uses `localhost:1935` as the incoming URL (standard RTMP port) and `copy` codecs to minimize CPU usage. Users typically customize these after creation.

```typescript
const defaultProfile: Profile = {
  id: crypto.randomUUID(),
  name: 'New Profile',
  incomingUrl: 'rtmp://localhost:1935/live/stream',
  outputGroups: [defaultOutputGroup],
};
```

### Default Output Group

```typescript
const defaultOutputGroup: OutputGroup = {
  id: crypto.randomUUID(),
  name: 'Main Output',
  video: {
    codec: 'copy',
    width: 1920,
    height: 1080,
    bitrate: 6000,
    fps: 60,
  },
  audio: {
    codec: 'copy',
    bitrate: 160,
    sampleRate: 48000,
    channels: 2,
  },
  container: {
    format: 'flv',
  },
  streamTargets: [],
};
```

### Default Settings

```typescript
const defaultSettings: Settings = {
  language: 'en',
  theme: 'system',
  startMinimized: false,
  showNotifications: true,
  autoDownloadFfmpeg: true,
};
```

---

## File Organization

```
apps/web/src/types/
├── index.ts           # Re-exports
├── profile.ts         # Profile, OutputGroup, StreamTarget
├── settings.ts        # Settings, Theme
├── stream.ts          # StreamStats, StreamStatus, errors
├── encoder.ts         # EncoderInfo, EncoderType
├── events.ts          # Event payloads
└── stores.ts          # Store state interfaces
```

---

**Related:** [Commands API](./01-commands-api.md) | [Events API](./02-events-api.md) | [Models Reference](../02-backend/03-models-reference.md)

