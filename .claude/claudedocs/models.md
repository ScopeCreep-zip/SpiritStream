# Domain Models

## Overview

MagillaStream's domain layer consists of four core models that represent the business entities of the streaming application.

## Model Hierarchy

```
Profile
├── id: string
├── name: string
├── incomingUrl: string
├── theme?: Theme
└── outputGroups: OutputGroup[]
          ├── id: string
          ├── videoEncoder: string
          ├── resolution: string
          ├── videoBitrate: number
          ├── fps: number
          ├── audioCodec: string
          ├── audioBitrate: number
          ├── generatePts: boolean
          └── streamTargets: StreamTarget[]
                    ├── id: string
                    ├── url: string
                    ├── streamKey: string
                    └── port: number
```

## Profile

**Location**: `src/models/Profile.ts`

The Profile model is the top-level entity representing a complete streaming configuration.

### Properties

| Property | Type | Description |
|----------|------|-------------|
| `_id` | `string` | Unique identifier (UUID) |
| `_name` | `string` | User-friendly profile name |
| `_incomingUrl` | `string` | RTMP URL where incoming stream is received |
| `_outputGroups` | `OutputGroup[]` | Array of output encoding configurations |
| `_theme` | `Theme \| undefined` | Optional UI theme customization |

### Methods

```typescript
class Profile {
  // Getters/Setters
  get id(): string;
  get name(): string;
  set name(value: string);
  get incomingUrl(): string;
  set incomingUrl(value: string);
  get outputGroups(): OutputGroup[];
  get theme(): Theme | undefined;
  set theme(value: Theme | undefined);

  // Group management
  addOutputGroup(group: OutputGroup): void;
  removeOutputGroup(groupId: string): void;
  getOutputGroup(groupId: string): OutputGroup | undefined;

  // Serialization
  toDTO(): ProfileDTO;
  export(): ExportedProfile;

  // Factory
  static fromDTO(dto: ProfileDTO): Profile;
}
```

### Example Usage

```typescript
const profile = new Profile('My Stream Config');
profile.incomingUrl = 'rtmp://localhost:1935/live';

const group = new OutputGroup();
group.videoEncoder = 'libx264';
group.resolution = '1920x1080';
group.videoBitrate = 6000;

profile.addOutputGroup(group);

// Save to file
const dto = profile.toDTO();
await profileManager.save(dto);
```

## OutputGroup

**Location**: `src/models/OutputGroup.ts`

An OutputGroup represents a specific encoding configuration that can output to multiple streaming targets.

### Properties

| Property | Type | Default | Description |
|----------|------|---------|-------------|
| `_id` | `string` | UUID | Unique identifier |
| `_videoEncoder` | `string` | 'libx264' | FFmpeg video encoder |
| `_resolution` | `string` | '1920x1080' | Output resolution |
| `_videoBitrate` | `number` | 6000 | Video bitrate in kbps |
| `_fps` | `number` | 30 | Frames per second |
| `_audioCodec` | `string` | 'aac' | FFmpeg audio codec |
| `_audioBitrate` | `number` | 128 | Audio bitrate in kbps |
| `_generatePts` | `boolean` | false | Generate PTS timestamps |
| `_streamTargets` | `StreamTarget[]` | [] | Output destinations |

### Methods

```typescript
class OutputGroup {
  // Getters/Setters for all properties
  get videoEncoder(): string;
  set videoEncoder(value: string);
  // ... etc

  // Target management
  addStreamTarget(target: StreamTarget): void;
  removeStreamTarget(targetId: string): void;
  getStreamTarget(targetId: string): StreamTarget | undefined;

  // Serialization
  toDTO(): OutputGroupDTO;

  // Factory
  static fromDTO(dto: OutputGroupDTO): OutputGroup;
}
```

### FFmpeg Command Generation

The OutputGroup properties map directly to FFmpeg arguments:

```bash
ffmpeg -i <incomingUrl> \
  -c:v <videoEncoder> \
  -s <resolution> \
  -b:v <videoBitrate>k \
  -r <fps> \
  -c:a <audioCodec> \
  -b:a <audioBitrate>k \
  [-fflags +genpts] \           # if generatePts is true
  -f flv <streamTarget.normalizedPath>
```

## StreamTarget

**Location**: `src/models/StreamTarget.ts`

A StreamTarget represents a single RTMP destination where the encoded stream will be sent.

### Properties

| Property | Type | Default | Description |
|----------|------|---------|-------------|
| `_id` | `string` | UUID | Unique identifier |
| `_url` | `string` | '' | RTMP server URL (e.g., 'rtmp://a.rtmp.youtube.com/live2') |
| `_streamKey` | `string` | '' | Stream key for authentication |
| `_port` | `number` | 1935 | RTMP port |

### Computed Properties

```typescript
get normalizedPath(): string {
  // Constructs the full RTMP path
  // Returns: rtmp://server:port/path/streamkey
  return `${this.url}/${this.streamKey}`;
}
```

### Methods

```typescript
class StreamTarget {
  // Getters/Setters
  get url(): string;
  set url(value: string);
  get streamKey(): string;
  set streamKey(value: string);
  get port(): number;
  set port(value: number);
  get normalizedPath(): string;

  // Serialization
  toDTO(): StreamTargetDTO;
  export(): ExportedStreamTarget;

  // Factory
  static fromDTO(dto: StreamTargetDTO): StreamTarget;
}
```

### Example Targets

| Platform | URL | Port |
|----------|-----|------|
| YouTube | rtmp://a.rtmp.youtube.com/live2 | 1935 |
| Twitch | rtmp://live.twitch.tv/app | 1935 |
| Facebook | rtmps://live-api-s.facebook.com:443/rtmp | 443 |
| Custom | rtmp://your-server.com/live | 1935 |

## Theme

**Location**: `src/models/Theme.ts`

The Theme model represents UI customization options.

### Properties

| Property | Type | Default | Description |
|----------|------|---------|-------------|
| `primaryColor` | `string` | '#007bff' | Primary UI color |
| `secondaryColor` | `string` | '#6c757d' | Secondary UI color |
| `backgroundColor` | `string` | '#ffffff' | Background color |
| `textColor` | `string` | '#212529' | Text color |
| `darkMode` | `boolean` | false | Enable dark mode |

### Methods

```typescript
class Theme {
  toDTO(): ThemeDTO;
  static fromDTO(dto: ThemeDTO): Theme;
}
```

## DTO Interfaces

**Location**: `src/shared/interfaces.ts`

All models have corresponding DTO interfaces for serialization:

```typescript
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

export interface ThemeDTO {
  primaryColor: string;
  secondaryColor: string;
  backgroundColor: string;
  textColor: string;
  darkMode: boolean;
}
```

## Model Conversion Utilities

**Location**: `src/utils/dtoUtils.ts`

The `dtoUtils` module provides conversion functions between DTOs and model instances:

```typescript
// Convert DTO to Model
function profileFromDTO(dto: ProfileDTO): Profile;
function outputGroupFromDTO(dto: OutputGroupDTO): OutputGroup;
function streamTargetFromDTO(dto: StreamTargetDTO): StreamTarget;

// Convert Model to DTO
function profileToDTO(profile: Profile): ProfileDTO;
// Note: Models have their own toDTO() methods as well
```

## Best Practices

1. **Always use DTOs for IPC**: Never send model instances through IPC
2. **Immutable IDs**: Never modify the ID after creation
3. **Validate before save**: Ensure required fields are populated
4. **Use factory methods**: Prefer `fromDTO()` over manual construction when loading
5. **Handle optional theme**: Check for undefined before accessing theme properties
