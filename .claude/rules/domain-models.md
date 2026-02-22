# Domain Models

## Core Models

### Profile
Top-level configuration containing all scenes, sources, and settings.

| Field | Type | Description |
|-------|------|-------------|
| `id` | `string` | UUID |
| `name` | `string` | User-friendly name |
| `scenes` | `Scene[]` | Scene compositions |
| `sources` | `Source[]` | All sources (profile-level, placed in scenes via layers) |
| `outputGroups` | `OutputGroup[]` | Encoding and streaming targets |

### Scene
A composable canvas with positioned sources.

| Field | Type | Description |
|-------|------|-------------|
| `id` | `string` | UUID |
| `name` | `string` | Display name |
| `layers` | `Layer[]` | Positioned source instances |
| `layerGroups` | `LayerGroup[]` | Organizational grouping |
| `defaultTransition` | `TransitionConfig` | Scene-specific transition |

### Layer
A source instance positioned on a scene canvas.

| Field | Type | Description |
|-------|------|-------------|
| `id` | `string` | UUID |
| `sourceId` | `string` | Reference to source |
| `position` | `{ x, y }` | Canvas position |
| `size` | `{ width, height }` | Display size |
| `rotation` | `number` | Degrees |
| `crop` | `{ top, right, bottom, left }` | Pixel crop |
| `visible` | `boolean` | Layer visibility |
| `locked` | `boolean` | Prevent editing |
| `filters` | `VideoFilter[]` | Applied video filters |

### Source
A reusable input that can be placed in multiple scenes.

| Field | Type | Description |
|-------|------|-------------|
| `id` | `string` | UUID |
| `name` | `string` | Display name |
| `type` | `SourceType` | One of 14 source types |
| `config` | `SourceConfig` | Type-specific configuration |
| `audioConfig` | `AudioConfig?` | Volume, mute, filters |
| `videoFilters` | `VideoFilter[]?` | Default filters |

### OutputGroup
Encoding profile for stream targets.

| Field | Type | Description |
|-------|------|-------------|
| `videoEncoder` | `string` | FFmpeg video codec |
| `resolution` | `string` | Output resolution |
| `videoBitrate` | `number` | Video bitrate (kbps) |
| `fps` | `number` | Frame rate |
| `audioCodec` | `string` | FFmpeg audio codec |
| `audioBitrate` | `number` | Audio bitrate (kbps) |
| `streamTargets` | `StreamTarget[]` | Output destinations |

### StreamTarget
RTMP destination.

| Field | Type | Description |
|-------|------|-------------|
| `url` | `string` | RTMP server URL |
| `streamKey` | `string` | Authentication key |
| `port` | `number` | RTMP port (default: 1935) |

## Source Types

| Type | Description | Key Properties |
|------|-------------|----------------|
| `rtmp` | Network stream input | `url` |
| `camera` | Webcam/USB camera | `deviceId`, `resolution`, `fps` |
| `screen` | Display capture | `displayId`, `captureCursor` |
| `window` | Application window | `windowId`, `captureCursor` |
| `game` | Game capture | `windowId`, `captureMode`, `allowTransparency` |
| `captureCard` | HDMI/SDI input | `deviceId`, `resolution`, `fps` |
| `ndi` | NDI network source | `sourceName`, `bandwidth`, `lowLatency` |
| `mediaFile` | Video/audio file | `filePath`, `loop`, `restartOnActivate` |
| `mediaPlaylist` | File playlist | `items[]`, `shuffleMode`, `loop` |
| `text` | Text overlay | `text`, `font`, `color`, `outline` |
| `browser` | Web page | `url`, `width`, `height`, `css` |
| `colorFill` | Solid color | `color` |
| `nestedScene` | Scene within scene | `sceneId` |
| `audioDevice` | Audio-only input | `deviceId`, `channels` |

## Key Model Files

- **Rust**: `server/src/models/` — `profile.rs`, `scene.rs`, `source.rs`, `output_group.rs`, `stream_target.rs`, `settings.rs`, `theme.rs`, `encoders.rs`, `stream_stats.rs`
- **TypeScript**: `apps/web/src/types/source.ts` — source type definitions
- **Stores**: `apps/web/src/stores/profileStore.ts`, `sceneStore.ts`, `sourceStore.ts`
