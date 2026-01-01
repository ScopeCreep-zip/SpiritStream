# Service Layer

## Overview

MagillaStream's service layer contains singleton services that handle core business logic and system operations.

## Service Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                        Service Layer                             │
├────────────────┬────────────────┬────────────────┬──────────────┤
│ ProfileManager │ FFmpegHandler  │    Logger      │  Encryption  │
│                │                │                │              │
│ - Persistence  │ - Process Mgmt │ - File Logging │ - AES-256    │
│ - Encryption   │ - Cmd Building │ - Levels       │ - PBKDF2     │
│ - Caching      │ - Encoder Det. │ - Formatting   │ - Secure     │
└────────────────┴────────────────┴────────────────┴──────────────┘
         │                │                │              │
         v                v                v              v
┌─────────────────────────────────────────────────────────────────┐
│                     File System / OS                             │
│   profiles/*.json   │   Child Processes   │   logs/*.log        │
└─────────────────────────────────────────────────────────────────┘
```

## ProfileManager

**Location**: `src/utils/profileManager.ts`

Manages profile persistence, encryption, and caching.

### Singleton Pattern

```typescript
class ProfileManager {
  private static instance: ProfileManager;

  private constructor() {
    this.profilesDir = path.join(app.getPath('userData'), 'profiles');
    this.ensureProfilesDir();
  }

  public static getInstance(): ProfileManager {
    if (!ProfileManager.instance) {
      ProfileManager.instance = new ProfileManager();
    }
    return ProfileManager.instance;
  }
}
```

### Methods

| Method | Parameters | Returns | Description |
|--------|------------|---------|-------------|
| `getAllProfileNames()` | none | `string[]` | List all saved profile names |
| `load(name, password?)` | `string, string?` | `ProfileDTO` | Load profile, decrypt if needed |
| `save(profile, password?)` | `ProfileDTO, string?` | `void` | Save profile, encrypt if password provided |
| `delete(name)` | `string` | `void` | Delete profile file |
| `getLastUsed()` | none | `string \| null` | Get last used profile name |
| `saveLastUsed(name)` | `string` | `void` | Save last used profile name |

### File Storage

Profiles are stored as JSON files:

```
{userData}/
  profiles/
    MyProfile.json
    StreamConfig.json
    ...
```

### Encryption Detection

```typescript
private isEncrypted(content: string): boolean {
  // Encrypted files start with specific marker
  // or are Base64 encoded with salt prefix
  try {
    const parsed = JSON.parse(content);
    return false; // Valid JSON = not encrypted
  } catch {
    return true; // Not valid JSON = encrypted
  }
}
```

### Save Flow

```typescript
async save(profile: ProfileDTO, password?: string): Promise<void> {
  const filePath = this.getProfilePath(profile.name);
  let content = JSON.stringify(profile, null, 2);

  if (password) {
    content = Encryption.getInstance().encrypt(content, password);
  }

  await fs.writeFile(filePath, content, 'utf-8');
}
```

## FFmpegHandler

**Location**: `src/utils/ffmpegHandler.ts`

Manages FFmpeg process spawning, monitoring, and termination.

### Properties

| Property | Type | Description |
|----------|------|-------------|
| `ffmpegPath` | `string` | Path to FFmpeg binary |
| `runningProcesses` | `Map<string, ChildProcess>` | Active FFmpeg processes by group ID |

### Methods

| Method | Parameters | Returns | Description |
|--------|------------|---------|-------------|
| `test()` | none | `boolean` | Verify FFmpeg is available |
| `start(group, url)` | `OutputGroupDTO, string` | `ProcessInfo` | Start encoding |
| `stop(groupId)` | `string` | `void` | Stop specific stream |
| `stopAll()` | none | `void` | Stop all streams |
| `getVideoEncoders()` | none | `EncoderInfo[]` | List available video encoders |
| `getAudioEncoders()` | none | `EncoderInfo[]` | List available audio encoders |

### FFmpeg Path Resolution

```typescript
private resolveFfmpegPath(): string {
  // Development: Use bundled FFmpeg
  const devPath = path.join(__dirname, '../../resources/ffmpeg/bin/ffmpeg');

  // Production: Use app resources
  const prodPath = path.join(process.resourcesPath, 'ffmpeg/bin/ffmpeg');

  // Add .exe for Windows
  const ext = process.platform === 'win32' ? '.exe' : '';

  return fs.existsSync(devPath + ext) ? devPath + ext : prodPath + ext;
}
```

### Command Building

```typescript
private buildCommand(group: OutputGroupDTO, incomingUrl: string): string[] {
  const args = [
    '-i', incomingUrl,
    '-c:v', group.videoEncoder,
    '-s', group.resolution,
    '-b:v', `${group.videoBitrate}k`,
    '-r', String(group.fps),
    '-c:a', group.audioCodec,
    '-b:a', `${group.audioBitrate}k`,
  ];

  if (group.generatePts) {
    args.push('-fflags', '+genpts');
  }

  // Add output targets
  for (const target of group.streamTargets) {
    args.push('-f', 'flv', target.normalizedPath);
  }

  return args;
}
```

### Process Management

```typescript
async start(group: OutputGroupDTO, incomingUrl: string): Promise<ProcessInfo> {
  const args = this.buildCommand(group, incomingUrl);

  const process = spawn(this.ffmpegPath, args);

  process.stderr.on('data', (data) => {
    Logger.getInstance().ffmpeg(data.toString());
  });

  process.on('exit', (code) => {
    this.runningProcesses.delete(group.id);
    Logger.getInstance().log(`FFmpeg exited with code ${code}`);
  });

  this.runningProcesses.set(group.id, process);

  return { pid: process.pid, groupId: group.id };
}
```

## EncoderDetection

**Location**: `src/utils/encoderDetection.ts`

Detects available FFmpeg encoders and filters against whitelist.

### Methods

| Method | Parameters | Returns | Description |
|--------|------------|---------|-------------|
| `getAvailableVideoEncoders()` | none | `EncoderInfo[]` | Whitelisted video encoders |
| `getAvailableAudioEncoders()` | none | `EncoderInfo[]` | Whitelisted audio encoders |

### Detection Flow

```typescript
async getAvailableVideoEncoders(): Promise<EncoderInfo[]> {
  // 1. Run ffmpeg -encoders
  const output = await this.runFfmpegEncoders();

  // 2. Parse output for video encoders (V...)
  const detected = this.parseVideoEncoders(output);

  // 3. Load whitelist from encoders.conf
  const whitelist = this.loadWhitelist();

  // 4. Filter to only whitelisted encoders
  return detected.filter(e => whitelist.video.includes(e.name));
}
```

### Encoder Whitelist

**Location**: `config/encoders.conf`

```json
{
  "video": [
    "libx264",
    "libx265",
    "h264_nvenc",
    "hevc_nvenc",
    "h264_qsv",
    "hevc_qsv",
    "libaom-av1",
    "libsvtav1",
    "libvpx",
    "libvpx-vp9",
    "h264_vulkan"
  ],
  "audio": [
    "aac",
    "libopus",
    "libmp3lame"
  ]
}
```

## Logger

**Location**: `src/utils/logger.ts`

File-based logging with multiple log levels and outputs.

### Log Levels

| Level | Priority | Usage |
|-------|----------|-------|
| ERROR | 0 | Errors requiring attention |
| WARN | 1 | Potential issues |
| INFO | 2 | General information |
| DEBUG | 3 | Detailed debugging |

### Log Files

```
{userData}/
  logs/
    app.log      # Application logs
    ffmpeg.log   # FFmpeg process output
    frontend.log # Frontend/renderer logs
```

### Methods

```typescript
class Logger {
  log(message: string): void;
  error(message: string): void;
  warn(message: string): void;
  info(message: string): void;
  debug(message: string): void;
  ffmpeg(message: string): void;  // Separate FFmpeg log
}
```

### Log Format

```
[2024-01-15T10:30:45.123-05:00] [INFO] Application started
[2024-01-15T10:30:46.456-05:00] [DEBUG] Loading profile: MyStream
[2024-01-15T10:30:47.789-05:00] [ERROR] Failed to connect: Connection refused
```

## RendererLogger

**Location**: `src/utils/rendererLogger.ts`

Bridge for frontend logging through IPC.

```typescript
// Frontend usage
window.electronAPI.logger.log('info', 'Button clicked');
window.electronAPI.logger.log('error', 'Failed to update UI');
```

## Encryption

**Location**: `src/utils/encryption.ts`

AES-256-GCM encryption for profile security.

### Algorithm Details

| Component | Value |
|-----------|-------|
| Algorithm | AES-256-GCM |
| Key Derivation | PBKDF2 |
| Iterations | 100,000 |
| Salt Length | 16 bytes |
| IV Length | 12 bytes |
| Auth Tag | 16 bytes |

### Methods

```typescript
class Encryption {
  encrypt(plaintext: string, password: string): string;
  decrypt(ciphertext: string, password: string): string;
}
```

### Encrypted Data Format

```
Base64(salt + iv + authTag + encryptedData)
```

### Encryption Flow

```typescript
encrypt(plaintext: string, password: string): string {
  // 1. Generate random salt
  const salt = crypto.randomBytes(16);

  // 2. Derive key using PBKDF2
  const key = crypto.pbkdf2Sync(password, salt, 100000, 32, 'sha256');

  // 3. Generate random IV
  const iv = crypto.randomBytes(12);

  // 4. Create cipher and encrypt
  const cipher = crypto.createCipheriv('aes-256-gcm', key, iv);
  const encrypted = Buffer.concat([
    cipher.update(plaintext, 'utf8'),
    cipher.final()
  ]);

  // 5. Get auth tag
  const authTag = cipher.getAuthTag();

  // 6. Combine and encode
  const combined = Buffer.concat([salt, iv, authTag, encrypted]);
  return combined.toString('base64');
}
```

## Service Initialization

Services are initialized in the main process:

```typescript
// main.ts
app.whenReady().then(async () => {
  // Initialize services
  const logger = Logger.getInstance();
  logger.info('Application starting...');

  const profileManager = ProfileManager.getInstance();
  const ffmpegHandler = FFmpegHandler.getInstance();

  // Test FFmpeg availability
  const ffmpegOk = await ffmpegHandler.test();
  if (!ffmpegOk) {
    logger.error('FFmpeg not found!');
  }

  // Register IPC handlers
  registerIpcHandlers();

  // Create window
  createMainWindow();
});
```
