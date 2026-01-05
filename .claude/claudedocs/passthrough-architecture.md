# Passthrough Architecture

**Date**: 2026-01-04
**Status**: Implemented

## Overview

SpiritStream uses a **passthrough-first architecture** where FFmpeg acts as an RTMP relay server by default, with optional re-encoding via custom output groups.

## Concept

```
┌─────────────────────────────────────────────────────────────────┐
│                     OBS (External Encoder)                      │
│                                                                 │
│  User configures: 1080p30 H.264 @ 240000K                      │
│                          ↓                                      │
│              RTMP stream to SpiritStream                        │
└─────────────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────────────┐
│                      SpiritStream                               │
│                                                                 │
│  Profile: Defines RTMP server listen settings                  │
│  ├─ Bind Address: 0.0.0.0                                       │
│  ├─ Port: 1935                                                  │
│  └─ Application: live                                           │
│                                                                 │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │ Default Output Group (Passthrough)                       │  │
│  │ ├─ ID: "default"                                         │  │
│  │ ├─ isDefault: true (IMMUTABLE)                           │  │
│  │ ├─ video.codec: "copy"                                   │  │
│  │ ├─ audio.codec: "copy"                                   │  │
│  │ └─ Stream Targets: [YouTube]                             │  │
│  │                                                          │  │
│  │ → Forwards 1080p30 @ 240000K to YouTube                 │  │
│  └──────────────────────────────────────────────────────────┘  │
│                                                                 │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │ Custom Output Group (Re-encode)                          │  │
│  │ ├─ ID: random UUID                                       │  │
│  │ ├─ isDefault: false                                      │  │
│  │ ├─ video.codec: "libx264"                                │  │
│  │ ├─ video.bitrate: "6000k"                                │  │
│  │ ├─ Resolution: 1080p30                                   │  │
│  │ └─ Stream Targets: [Twitch, YouTube Backup]              │  │
│  │                                                          │  │
│  │ → Re-encodes 240000K → 6000K for Twitch                 │  │
│  └──────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
```

## Architecture Decisions

### 1. Default Passthrough Group

Every profile includes an **immutable default output group**:

- **ID**: Fixed as `"default"`
- **Name**: "Passthrough (Default)"
- **Flag**: `isDefault: true`
- **Video codec**: `"copy"` (no re-encoding)
- **Audio codec**: `"copy"` (no re-encoding)
- **Cannot be edited**: Modal refuses to open
- **Cannot be deleted**: Store silently refuses
- **Can add/remove targets**: Still configurable

### 2. Custom Output Groups

Users create custom groups for re-encoding:

- **ID**: Random UUID
- **Flag**: `isDefault: false`
- **Configurable**: Full encoding settings
- **Can be edited/duplicated/deleted**: Full control
- **Purpose**: Transcode to different quality levels

## Implementation

### Backend (Rust)

**`src-tauri/src/models/output_group.rs`**

```rust
pub struct OutputGroup {
    pub id: String,
    pub name: String,
    pub is_default: bool,  // Marks immutable default
    pub video: VideoSettings,
    pub audio: AudioSettings,
    pub container: ContainerSettings,
    pub stream_targets: Vec<StreamTarget>,
}

impl OutputGroup {
    /// Create new custom output group
    pub fn new() -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name: "New Output Group".to_string(),
            is_default: false,
            video: VideoSettings::default(), // copy mode
            audio: AudioSettings::default(), // copy mode
            // ...
        }
    }

    /// Create the default passthrough group
    pub fn new_default() -> Self {
        Self {
            id: "default".to_string(),
            name: "Passthrough (Default)".to_string(),
            is_default: true,
            video: VideoSettings::default(), // codec: "copy"
            audio: AudioSettings::default(), // codec: "copy"
            // ...
        }
    }
}
```

**VideoSettings defaults**:
```rust
impl Default for VideoSettings {
    fn default() -> Self {
        Self {
            codec: "copy".to_string(),
            width: 0,
            height: 0,
            fps: 0,
            bitrate: "0k".to_string(),
            preset: None,
            profile: None,
        }
    }
}
```

### Frontend (TypeScript)

**`src-frontend/types/profile.ts`**

```typescript
export interface OutputGroup {
  id: string;
  name: string;
  isDefault?: boolean;  // Marks immutable default
  video: VideoSettings;
  audio: AudioSettings;
  container: ContainerSettings;
  streamTargets: StreamTarget[];
}

// Factory for default passthrough group
export const createPassthroughOutputGroup = (): OutputGroup => ({
  id: 'default',
  name: 'Passthrough (Default)',
  isDefault: true,
  video: { codec: 'copy', width: 0, height: 0, fps: 0, bitrate: '0k' },
  audio: { codec: 'copy', bitrate: '0k', channels: 0, sampleRate: 0 },
  container: { format: 'flv' },
  streamTargets: [],
});

// Always include default group when creating profiles
export const createDefaultProfile = (name: string = 'New Profile'): Profile => ({
  id: crypto.randomUUID(),
  name,
  encrypted: false,
  input: createDefaultRtmpInput(),
  outputGroups: [createPassthroughOutputGroup()],
});
```

### FFmpeg Command Generation

**`src-tauri/src/services/ffmpeg_handler.rs`**

The handler detects copy mode and generates appropriate commands:

**Passthrough mode** (`codec: "copy"`):
```bash
ffmpeg -listen 1 -i rtmp://0.0.0.0:1935/live \
  -c:v copy \
  -c:a copy \
  -map 0:v \
  -map 0:a \
  -f flv rtmp://live.twitch.tv/app/{stream_key}
```

**Re-encode mode** (custom codec):
```bash
ffmpeg -listen 1 -i rtmp://0.0.0.0:1935/live \
  -c:v libx264 \
  -s 1920x1080 \
  -b:v 6000k \
  -r 30 \
  -preset veryfast \
  -c:a aac \
  -b:a 160k \
  -map 0:v \
  -map 0:a \
  -f flv rtmp://live.twitch.tv/app/{stream_key}
```

## UI Behavior

### Profile Modal

**Removed encoding settings**:
- No resolution selector
- No FPS selector
- No bitrate input

**Clarification text**:
> "Configure your streaming software (OBS, etc.) to send to this RTMP URL. Encoding settings are configured in your streaming software, not in the profile. Use output groups to re-encode to different settings for different platforms."

### EncoderCard (Default Group)

```
┌─────────────────────────────────────────────────┐
│ 🖥️ Passthrough (Default) (Read-only)      [●] │
│    Passthrough                                   │
│                                                  │
│ ┌────────┬────────┬────────┬────────┐           │
│ │ Source │ Source │ Source │   —    │           │
│ │  Res   │ Bitrat │  FPS   │ Preset │           │
│ └────────┴────────┴────────┴────────┘           │
│                                                  │
│ 🔊 Audio: Source                                 │
│ Default RTMP relay - cannot be edited or deleted│
└─────────────────────────────────────────────────┘
```

**Features**:
- Shows "Source" for all encoding parameters
- No Edit/Duplicate/Delete buttons
- Clear "(Read-only)" indicator
- Explanation text at bottom

### EncoderCard (Custom Group)

```
┌─────────────────────────────────────────────────┐
│ 🎬 High Quality Stream                    [●]   │
│    NVENC (Hardware)                              │
│                                                  │
│ ┌─────────┬────────┬────────┬─────────┐         │
│ │1920×1080│ 6000k  │ 30 fps │ Balanced│         │
│ │   Res   │ Bitrat │  FPS   │  Preset │         │
│ └─────────┴────────┴────────┴─────────┘         │
│                                                  │
│ 🔊 Audio: AAC @ 160k • Profile: HIGH             │
│ [✏️ Edit] [📋 Copy] [🗑️ Delete]                  │
└─────────────────────────────────────────────────┘
```

**Features**:
- Shows actual encoding parameters
- Full Edit/Duplicate/Delete functionality
- Hardware encoder indicator

### OutputGroupModal Protection

```typescript
export function OutputGroupModal({ open, onClose, mode, group }: Props) {
  const isDefaultGroup = mode === 'edit' && group?.isDefault === true;

  // Refuse to open modal for default group
  if (isDefaultGroup && open) {
    setTimeout(() => onClose(), 0);
    return null;
  }

  // ... rest of modal
}
```

## User Workflows

### Basic Workflow (Passthrough Only)

1. User creates profile in SpiritStream
2. Default passthrough group created automatically
3. User adds YouTube target to default group
4. User configures OBS to stream to `rtmp://localhost:1935/live`
5. User starts streaming from OBS
6. SpiritStream relays stream to YouTube without re-encoding

### Advanced Workflow (Mixed Passthrough + Re-encode)

1. User creates profile in SpiritStream
2. Default passthrough group created automatically
3. User adds YouTube Primary target to default group
4. User clicks "Add Encoder" → creates custom group
5. User configures custom group: 1080p30 H.264 @ 6000k
6. User adds Twitch and YouTube Backup to custom group
7. User configures OBS to stream at 1080p30 H.264 @ 240000k
8. User starts streaming from OBS
9. SpiritStream:
   - Relays 240000k stream to YouTube Primary (no re-encode)
   - Re-encodes to 6000k for Twitch and YouTube Backup

## Benefits

### 1. Clear Mental Model
- **Default = relay**: Just forwards the stream
- **Custom = transcode**: Re-encodes to different settings
- No confusion about what each group does

### 2. Prevents Misconfiguration
- Can't accidentally break the default relay
- Default always works as simple RTMP forwarder
- Forces intentional creation of custom encoding

### 3. Performance Optimization
- Default passthrough uses zero CPU/GPU for encoding
- Only re-encode when explicitly needed
- Multiple targets can share the same passthrough stream

### 4. Simplified Onboarding
- New users get working passthrough immediately
- No need to understand encoding settings initially
- Can add re-encoding later when needed

### 5. Flexibility
- Can mix passthrough and re-encode targets
- Different platforms can get different quality levels
- Original quality available for primary destinations

## Related Documentation

- [immutable-default-group.md](.claude/claudedocs/scratch/immutable-default-group.md)
- [passthrough-mode-changes.md](.claude/claudedocs/scratch/passthrough-mode-changes.md)
- [profile-encoding-removal.md](.claude/claudedocs/scratch/profile-encoding-removal.md)

---

**Architecture implemented**: 2026-01-04
