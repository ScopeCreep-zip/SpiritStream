# Passthrough Mode Implementation

**Date**: 2026-01-04
**Status**: Completed

## Summary

Changed default `OutputGroup` behavior to use FFmpeg in **passthrough/relay mode** instead of re-encoding. This makes FFmpeg function purely as an RTMP server by default, accepting incoming streams and forwarding them to targets without any transcoding.

## Motivation

The original defaults forced users to specify encoding settings (resolution, bitrate, FPS, etc.) even when they just wanted FFmpeg to relay the stream as-is. This was inefficient and created unnecessary configuration overhead.

By defaulting to passthrough mode:
- FFmpeg acts as a pure RTMP relay server
- No CPU/GPU encoding overhead
- Original stream quality is preserved
- Simpler default configuration
- Users can still configure encoding when needed

## Changes Made

### 1. Backend (Rust)

**File**: `src-tauri/src/models/output_group.rs`

- Changed `VideoSettings::default()`:
  - `codec`: `"libx264"` → `"copy"`
  - `width`, `height`, `fps`: Removed defaults (set to `0`)
  - `bitrate`: `"6000k"` → `"0k"`
  - `preset`, `profile`: Changed to `None`

- Changed `AudioSettings::default()`:
  - `codec`: `"aac"` → `"copy"`
  - `bitrate`: `"160k"` → `"0k"`
  - `channels`: `2` → `0`
  - `sample_rate`: `48000` → `0`

- Added documentation clarifying passthrough mode

**File**: `src-tauri/src/models/profile.rs`

- Updated `Profile::to_summary()` to detect copy mode
- Shows `"Passthrough"` instead of `"0p0"` for copy mode profiles
- Shows `"None"` when no output groups exist

**File**: `src-tauri/src/services/ffmpeg_handler.rs`

- Added clarifying comments about passthrough mode behavior
- No logic changes needed (copy mode already supported)

**File**: `src-tauri/src/commands/system.rs`

- Removed unused `regex::Regex` import (cleanup)

### 2. Frontend (TypeScript)

**File**: `src-frontend/types/profile.ts`

- Updated `createDefaultVideoSettings()`:
  - `codec`: `'libx264'` → `'copy'`
  - `width`, `height`, `fps`: Set to `0`
  - `bitrate`: `'6000k'` → `'0k'`
  - `preset`, `profile`: Changed to `undefined`

- Updated `createDefaultAudioSettings()`:
  - `codec`: `'aac'` → `'copy'`
  - `bitrate`: `'160k'` → `'0k'`
  - `channels`: `2` → `0`
  - `sampleRate`: `48000` → `0`

- Updated `formatResolution()` helper:
  - Detects copy mode (`codec === 'copy'` or `height === 0`)
  - Returns `'Passthrough'` string instead of resolution

## How It Works

### FFmpeg Command Generation

The `FFmpegHandler::build_args()` method already had logic to detect copy mode:

```rust
let use_stream_copy = group.video.codec == "copy" && group.audio.codec == "copy";

if use_stream_copy {
    args.push("-c:v".to_string()); args.push("copy".to_string());
    args.push("-c:a".to_string()); args.push("copy".to_string());
} else {
    // ... encoding settings
}
```

### Example Command (Passthrough Mode)

```bash
ffmpeg -i rtmp://0.0.0.0:1935/live \
  -c:v copy \
  -c:a copy \
  -map 0:v \
  -map 0:a \
  -progress pipe:2 \
  -stats \
  -f flv rtmp://live.twitch.tv/app/{stream_key}
```

### Example Command (Encoding Mode)

```bash
ffmpeg -i rtmp://0.0.0.0:1935/live \
  -c:v libx264 \
  -s 1920x1080 \
  -b:v 6000k \
  -r 60 \
  -c:a aac \
  -b:a 160k \
  -ac 2 \
  -ar 48000 \
  -preset veryfast \
  -profile:v high \
  -map 0:v \
  -map 0:a \
  -progress pipe:2 \
  -stats \
  -f flv rtmp://live.twitch.tv/app/{stream_key}
```

## User-Facing Impact

### Profile Display

- **Before**: Profiles showed `"1080p60"` with bitrate even for passthrough
- **After**: Profiles show `"Passthrough"` when using copy mode

### Default Workflow

1. User creates a new profile
2. Output group is created with `codec: "copy"` by default
3. User adds stream targets
4. FFmpeg relays the incoming stream to all targets without re-encoding
5. If user wants to re-encode (different resolution/bitrate), they can change codec from "copy" to a real encoder

## Testing

- ✅ Rust compilation: `cargo check` - Success
- ✅ TypeScript types: `npm run typecheck` - Success
- ✅ No logic changes to FFmpeg handler (already supported copy mode)

## Future Considerations

1. **UI Updates**: Consider updating the encoder settings UI to show "Passthrough Mode" toggle
2. **Validation**: May want to prevent users from setting resolution/bitrate when codec is "copy"
3. **Documentation**: Update user-facing docs to explain passthrough vs encoding modes
4. **Profile Migration**: Existing profiles with encoding settings will continue to work as-is

## Related Files

- Backend models: `src-tauri/src/models/output_group.rs`, `profile.rs`
- Frontend types: `src-frontend/types/profile.ts`
- FFmpeg handler: `src-tauri/src/services/ffmpeg_handler.rs`
- Architecture docs: `.claude/claudedocs/alignment/ARCHITECTURE.md`
