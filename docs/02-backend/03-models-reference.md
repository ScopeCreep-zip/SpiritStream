# Models Reference

[Documentation](../README.md) > [Backend](./README.md) > Models Reference

---

This reference documents all data models used in SpiritStream's Rust backend. Models are defined as Rust structs with serde serialization for IPC communication.

## Model Hierarchy

Understanding how models relate to each other is key to working with SpiritStream's data layer:

```
Profile
├── id, name, incoming_url
└── output_groups: Vec<OutputGroup>
    ├── id, name
    ├── video: VideoSettings
    │   └── codec, resolution, bitrate, fps, preset...
    ├── audio: AudioSettings
    │   └── codec, bitrate, sample_rate, channels
    ├── container: ContainerSettings
    │   └── format
    └── stream_targets: Vec<StreamTarget>
        └── id, platform, name, url, stream_key
```

The hierarchy reflects how streaming actually works: a **Profile** captures your complete streaming setup (where video comes from, how it's encoded, where it goes). Each **OutputGroup** represents one FFmpeg process—all targets in a group share the same encoding, which is efficient but means different quality levels require separate groups.

---

## Core Models

### Profile

The top-level configuration entity representing a streaming setup. A user typically has multiple profiles for different scenarios: "Gaming Stream" for high-action content at 60fps, "Just Chatting" for lower bitrate talking-head streams, "Recording Only" for local capture without RTMP output.

Profiles are stored as individual JSON files in the app data directory, optionally encrypted with a password. The `incoming_url` is where SpiritStream receives video—usually from OBS via a local RTMP server or a capture card.

```rust
// apps/desktop/src-tauri/src/models/profile.rs
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub incoming_url: String,
    pub output_groups: Vec<OutputGroup>,
}
```

| Field | Type | Description |
|-------|------|-------------|
| `id` | `String` | UUID identifier |
| `name` | `String` | User-friendly profile name |
| `incoming_url` | `String` | RTMP ingest URL (e.g., `rtmp://localhost:1935/live/stream`) |
| `output_groups` | `Vec<OutputGroup>` | List of output configurations |

**TypeScript Equivalent:**
```typescript
interface Profile {
  id: string;
  name: string;
  incomingUrl: string;
  outputGroups: OutputGroup[];
}
```

---

### OutputGroup

An output configuration specifying encoding settings and stream targets. The key design decision here is that **all targets in a group share identical encoding**. FFmpeg encodes once and pushes the same stream to multiple destinations, which is CPU-efficient but inflexible.

**When to use multiple output groups:**
- Different quality levels (1080p for YouTube, 720p for Twitch)
- Different frame rates (60fps for gaming, 30fps for mobile viewers)
- Platform-specific encoding (one uses NVENC, another uses x264)

**When to use one output group with multiple targets:**
- Same quality to all platforms
- Simulcasting the exact same stream everywhere

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutputGroup {
    pub id: String,
    pub name: String,
    pub video: VideoSettings,
    pub audio: AudioSettings,
    pub container: ContainerSettings,
    pub stream_targets: Vec<StreamTarget>,
}
```

| Field | Type | Description |
|-------|------|-------------|
| `id` | `String` | UUID identifier |
| `name` | `String` | Group name (e.g., "Main Output") |
| `video` | `VideoSettings` | Video encoding settings |
| `audio` | `AudioSettings` | Audio encoding settings |
| `container` | `ContainerSettings` | Container format settings |
| `stream_targets` | `Vec<StreamTarget>` | RTMP destinations |

**TypeScript Equivalent:**
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

---

### VideoSettings

Video encoding configuration. These settings map directly to FFmpeg arguments—understanding them means understanding FFmpeg's encoding pipeline.

The `codec` field is the most important choice. Hardware encoders (`h264_nvenc`, `h264_qsv`, `h264_amf`) offload work to your GPU, while software encoders (`libx264`) use CPU. The `"copy"` codec passes video through unchanged—useful when your input is already encoded correctly and you just want to relay it.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoSettings {
    pub codec: String,
    pub width: u32,
    pub height: u32,
    pub bitrate: u32,
    pub fps: u32,
    pub preset: Option<String>,
    pub profile: Option<String>,
    pub keyframe_interval: Option<u32>,
}
```

| Field | Type | Description |
|-------|------|-------------|
| `codec` | `String` | FFmpeg codec name (e.g., `libx264`, `h264_nvenc`, `copy`) |
| `width` | `u32` | Output width in pixels |
| `height` | `u32` | Output height in pixels |
| `bitrate` | `u32` | Video bitrate in kbps |
| `fps` | `u32` | Frames per second |
| `preset` | `Option<String>` | Encoder preset (e.g., `veryfast`) |
| `profile` | `Option<String>` | H.264 profile (e.g., `high`) |
| `keyframe_interval` | `Option<u32>` | Keyframe interval in seconds |

**Passthrough Mode:**
When `codec` is `"copy"`, video is passed through without re-encoding.

---

### AudioSettings

Audio encoding configuration. Audio is simpler than video—AAC is the universal choice for RTMP streaming, and the settings rarely need adjustment beyond bitrate. The `"copy"` codec works well when your source is already AAC-encoded (common from OBS).

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioSettings {
    pub codec: String,
    pub bitrate: u32,
    pub sample_rate: u32,
    pub channels: u32,
}
```

| Field | Type | Description |
|-------|------|-------------|
| `codec` | `String` | FFmpeg audio codec (e.g., `aac`, `copy`) |
| `bitrate` | `u32` | Audio bitrate in kbps |
| `sample_rate` | `u32` | Sample rate in Hz (e.g., `48000`) |
| `channels` | `u32` | Number of channels (1=mono, 2=stereo) |

---

### ContainerSettings

Output container format settings. For RTMP streaming, this is almost always `"flv"`—it's the only container format the RTMP protocol supports. This model exists for future extensibility (recording to MP4, HLS output, etc.) rather than current flexibility.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContainerSettings {
    pub format: String,
}
```

| Field | Type | Description |
|-------|------|-------------|
| `format` | `String` | Container format (typically `flv` for RTMP) |

---

### StreamTarget

RTMP streaming destination. Each target represents one place your stream goes—a Twitch channel, a YouTube broadcast, a custom RTMP server. The `service` field references a `Platform` enum variant that determines platform-specific behavior (URL handling, stream key placement, log redaction).

**Security note:** Stream keys are sensitive credentials. They're encrypted at rest when profiles are password-protected, and the `stream_key` field supports environment variable references (`${MY_STREAM_KEY}`) for users who prefer not to store keys in profile files at all.

```rust
// apps/desktop/src-tauri/src/models/stream_target.rs

// Platform enum auto-generated from data/streaming-platforms.json at build time
include!(concat!(env!("OUT_DIR"), "/generated_platforms.rs"));

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamTarget {
    pub id: String,
    #[serde(default)]
    pub service: Platform,
    #[serde(default)]
    pub name: String,
    pub url: String,
    pub stream_key: String,
}
```

| Field | Type | Description |
|-------|------|-------------|
| `id` | `String` | UUID identifier |
| `service` | `Platform` | Auto-generated enum from platform registry (80+ platforms) |
| `name` | `String` | Display name (e.g., "My Twitch Channel") |
| `url` | `String` | RTMP server URL |
| `stream_key` | `String` | Stream key (can be env var reference: `${VAR_NAME}`) |

**Platform Enum:**

The `Platform` enum is auto-generated at build time from `data/streaming-platforms.json`. This means you won't find the enum definition in source code—it's created by `build.rs` and included via the `include!` macro. Common values include:

- `Platform::Youtube` - YouTube Live
- `Platform::Twitch` - Twitch
- `Platform::Kick` - Kick
- `Platform::FacebookLive` - Facebook Live
- `Platform::Custom` - Custom RTMP server
- Plus 75+ additional platforms from OBS's rtmp-services

See [Platform Registry](./06-platform-registry.md) for the full list and how to add new platforms.

---

## Statistics Models

Statistics models capture runtime data that doesn't persist—they're populated from FFmpeg's progress output and pushed to the frontend for live monitoring.

### StreamStats

Real-time streaming statistics parsed from FFmpeg's stderr output. These values update multiple times per second during an active stream and help users detect problems: dropping frames indicates encoding overload, bitrate fluctuation suggests network issues, and speed below 1.0 means encoding can't keep up with real-time.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamStats {
    pub group_id: String,
    pub frame: u64,
    pub fps: f64,
    pub bitrate: f64,
    pub speed: f64,
    pub size: u64,
    pub time: f64,
    pub dropped_frames: u64,
    pub dup_frames: u64,
}
```

| Field | Type | Description |
|-------|------|-------------|
| `group_id` | `String` | Output group identifier |
| `frame` | `u64` | Current frame number |
| `fps` | `f64` | Current frames per second |
| `bitrate` | `f64` | Current bitrate in kbps |
| `speed` | `f64` | Encoding speed (1.0 = real-time) |
| `size` | `u64` | Total bytes written |
| `time` | `f64` | Elapsed time in seconds |
| `dropped_frames` | `u64` | Number of dropped frames |
| `dup_frames` | `u64` | Number of duplicated frames |

---

## Settings Models

Settings models store user preferences that apply across all profiles. They're persisted separately from profiles so changing your theme doesn't require re-saving your streaming configuration.

### Settings

Application-wide settings stored in a single JSON file. The `last_profile` field enables "remember what I was doing" behavior on app launch.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub language: String,
    pub theme: String,
    pub start_minimized: bool,
    pub show_notifications: bool,
    pub ffmpeg_path: Option<String>,
    pub auto_download_ffmpeg: bool,
    pub last_profile: Option<String>,
}
```

| Field | Type | Description |
|-------|------|-------------|
| `language` | `String` | UI language code (e.g., `en`, `es`) |
| `theme` | `String` | Theme setting (`light`, `dark`, `system`) |
| `start_minimized` | `bool` | Start app minimized |
| `show_notifications` | `bool` | Show desktop notifications |
| `ffmpeg_path` | `Option<String>` | Custom FFmpeg path |
| `auto_download_ffmpeg` | `bool` | Auto-download FFmpeg if missing |
| `last_profile` | `Option<String>` | Last used profile name |

---

## Encoder Models

Encoder models describe what's available on the user's system. They're populated at runtime by querying FFmpeg, since available encoders depend on hardware (NVIDIA GPU for NVENC, Intel CPU for QSV, etc.).

### EncoderInfo

Information about an available encoder. The `encoder_type` distinction matters for UI—hardware encoders are generally preferred for streaming because they don't compete with games for CPU resources.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EncoderInfo {
    pub name: String,
    pub display_name: String,
    pub encoder_type: EncoderType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EncoderType {
    Software,
    Hardware,
}
```

| Field | Type | Description |
|-------|------|-------------|
| `name` | `String` | FFmpeg encoder name |
| `display_name` | `String` | User-friendly name |
| `encoder_type` | `EncoderType` | Software or hardware |

---

## Platform Models

Platform models provide metadata about streaming services. They're used for UI conveniences (auto-filling URLs, displaying the right icon) and security (intelligently redacting stream keys in logs based on each platform's URL structure).

### StreamKeyPlacement

Different platforms embed stream keys in URLs differently. This enum captures the two strategies SpiritStream needs to handle:

```rust
// apps/desktop/src-tauri/src/services/platform_registry.rs

pub enum StreamKeyPlacement {
    /// Append stream key to URL (e.g., rtmp://server/app/{key})
    Append,
    /// Replace {stream_key} template in URL (e.g., rtmp://server/{stream_key}/live)
    InUrlTemplate,
}
```

| Variant | Example | Used By |
|---------|---------|---------|
| `Append` | `rtmp://live.twitch.tv/app/` + `KEY` | Twitch, YouTube, most platforms |
| `InUrlTemplate` | `rtmp://server/{stream_key}/live` | Restream, some custom setups |

### PlatformConfig

Platform-specific configuration loaded from the JSON registry. Each platform has a `PlatformConfig` that determines how URLs are built, normalized, and how stream keys are masked in logs.

```rust
pub struct PlatformConfig {
    /// Display name
    pub name: &'static str,

    /// Default RTMP server URL (may contain {stream_key} template)
    pub default_server: &'static str,

    /// Stream key placement strategy
    pub placement: StreamKeyPlacement,

    /// Default app path for URL normalization (e.g., "app", "live2")
    pub default_app_path: Option<&'static str>,

    /// Stream key position in URL path for redaction (0 = don't mask)
    pub stream_key_position: usize,
}
```

| Field | Type | Description |
|-------|------|-------------|
| `name` | `&'static str` | Human-readable platform name |
| `default_server` | `&'static str` | Default RTMP URL, may include `{stream_key}` placeholder |
| `placement` | `StreamKeyPlacement` | How stream key is added to URL |
| `default_app_path` | `Option<&'static str>` | Path segment for URL normalization |
| `stream_key_position` | `usize` | Which path segment contains the key (for log redaction) |

**Key Methods:**

- `normalize_url()` - Fixes URLs missing required path segments (e.g., Kick requires `/app`)
- `redact_url()` - Masks stream keys in URLs for safe logging
- `build_url_with_key()` - Constructs final RTMP URL with stream key

See [Platform Registry](./06-platform-registry.md) for implementation details and how to add new platforms.

---

## Default Values

Default values are chosen to work out-of-the-box for common streaming scenarios. They prioritize compatibility (1080p60 works everywhere) and safety (copy mode avoids CPU load until the user explicitly chooses an encoder).

### Default Profile

New profiles start with a local RTMP URL and one empty output group. The `localhost:1935` URL assumes users are streaming from OBS to SpiritStream on the same machine—the most common setup.

```rust
impl Default for Profile {
    fn default() -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name: "New Profile".to_string(),
            incoming_url: "rtmp://localhost:1935/live/stream".to_string(),
            output_groups: vec![OutputGroup::default()],
        }
    }
}
```

### Default Output Group

```rust
impl Default for OutputGroup {
    fn default() -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name: "Main Output".to_string(),
            video: VideoSettings::default(),
            audio: AudioSettings::default(),
            container: ContainerSettings::default(),
            stream_targets: vec![],
        }
    }
}
```

### Default Video Settings

The `"copy"` codec default means new output groups pass video through without re-encoding—zero CPU impact until you explicitly choose an encoder. The other defaults (1080p60, 6000kbps, 2-second keyframes) match Twitch's recommended settings and work well on most platforms.

```rust
impl Default for VideoSettings {
    fn default() -> Self {
        Self {
            codec: "copy".to_string(),
            width: 1920,
            height: 1080,
            bitrate: 6000,
            fps: 60,
            preset: Some("veryfast".to_string()),  // Fast encoding, good quality
            profile: Some("high".to_string()),      // H.264 High profile for quality
            keyframe_interval: Some(2),             // 2 seconds for platform compatibility
        }
    }
}
```

### Default Audio Settings

Audio defaults to copy mode (passthrough) with standard streaming settings. 48kHz stereo at 160kbps is the sweet spot for voice and music—higher bitrates offer diminishing returns for streaming audio.

```rust
impl Default for AudioSettings {
    fn default() -> Self {
        Self {
            codec: "copy".to_string(),
            bitrate: 160,
            sample_rate: 48000,   // Professional audio standard
            channels: 2,          // Stereo
        }
    }
}
```

---

## Serialization

All models use `serde` for JSON serialization. This serves two purposes: persisting profiles to disk and sending data across the Tauri IPC boundary to the React frontend. The same struct definition works for both—no separate DTOs needed.

```rust
// Serialize to JSON
let json = serde_json::to_string_pretty(&profile)?;

// Deserialize from JSON
let profile: Profile = serde_json::from_str(&json)?;
```

### camelCase Conversion

Rust uses `snake_case` for fields; JavaScript uses `camelCase`. The `#[serde(rename_all = "camelCase")]` attribute handles this automatically—write idiomatic Rust, and the frontend receives idiomatic JavaScript:

| Rust | JSON |
|------|------|
| `incoming_url` | `incomingUrl` |
| `stream_targets` | `streamTargets` |
| `sample_rate` | `sampleRate` |

---

## Validation

Models can implement validation methods that check business rules before the data reaches services. This catches problems early—a profile with no output groups is invalid regardless of what operation you're trying to perform on it.

```rust
impl Profile {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.name.is_empty() {
            return Err(ValidationError::EmptyName);
        }

        if self.output_groups.is_empty() {
            return Err(ValidationError::NoOutputGroups);
        }

        for group in &self.output_groups {
            group.validate()?;
        }

        Ok(())
    }
}
```

---

**Related:** [Rust Overview](./01-rust-overview.md) | [Services Layer](./02-services-layer.md) | [Tauri Commands](./04-tauri-commands.md)

