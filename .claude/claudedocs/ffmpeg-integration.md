# FFmpeg Integration

## Overview

MagillaStream uses FFmpeg for all stream encoding operations. The application bundles FFmpeg and manages child processes for stream encoding.

## FFmpeg Location

### Development Mode

```
resources/
└── ffmpeg/
    └── bin/
        ├── ffmpeg        (Linux/macOS)
        ├── ffmpeg.exe    (Windows)
        ├── ffprobe       (Linux/macOS)
        └── ffprobe.exe   (Windows)
```

### Production Mode

After packaging, FFmpeg is located in the app resources:

```
{app}/
└── resources/
    └── resources/
        └── ffmpeg/
            └── bin/
                └── ffmpeg[.exe]
```

### Path Resolution

```typescript
private resolveFfmpegPath(): string {
  const isProduction = app.isPackaged;

  if (isProduction) {
    // Production: resources are in process.resourcesPath
    return path.join(
      process.resourcesPath,
      'resources',
      'ffmpeg',
      'bin',
      process.platform === 'win32' ? 'ffmpeg.exe' : 'ffmpeg'
    );
  } else {
    // Development: resources are relative to project root
    return path.join(
      __dirname,
      '..',
      '..',
      'resources',
      'ffmpeg',
      'bin',
      process.platform === 'win32' ? 'ffmpeg.exe' : 'ffmpeg'
    );
  }
}
```

## Command Structure

### Basic Encoding Command

```bash
ffmpeg -i <input> [options] -f flv <output>
```

### Full Command Template

```bash
ffmpeg \
  -i rtmp://localhost:1935/live/stream \    # Input
  -c:v libx264 \                            # Video codec
  -s 1920x1080 \                            # Resolution
  -b:v 6000k \                              # Video bitrate
  -r 30 \                                   # Framerate
  -c:a aac \                                # Audio codec
  -b:a 128k \                               # Audio bitrate
  -f flv rtmp://a.rtmp.youtube.com/live2/key  # Output
```

### Multi-Output Command

For multiple stream targets in one group:

```bash
ffmpeg -i rtmp://localhost:1935/live \
  -c:v libx264 -s 1920x1080 -b:v 6000k -r 30 \
  -c:a aac -b:a 128k \
  -f flv rtmp://youtube.com/live/key1 \
  -f flv rtmp://twitch.tv/app/key2 \
  -f flv rtmp://facebook.com/rtmp/key3
```

## Supported Encoders

### Video Encoders

| Encoder | Hardware | Description |
|---------|----------|-------------|
| `libx264` | CPU | H.264 software encoding |
| `libx265` | CPU | H.265/HEVC software encoding |
| `h264_nvenc` | NVIDIA GPU | H.264 NVIDIA hardware encoding |
| `hevc_nvenc` | NVIDIA GPU | H.265 NVIDIA hardware encoding |
| `h264_qsv` | Intel GPU | H.264 Intel Quick Sync |
| `hevc_qsv` | Intel GPU | H.265 Intel Quick Sync |
| `libaom-av1` | CPU | AV1 encoding |
| `libsvtav1` | CPU | AV1 (faster) encoding |
| `libvpx` | CPU | VP8 encoding |
| `libvpx-vp9` | CPU | VP9 encoding |
| `h264_vulkan` | GPU | H.264 Vulkan encoding |

### Audio Encoders

| Encoder | Description |
|---------|-------------|
| `aac` | AAC audio (default) |
| `libopus` | Opus audio (high quality) |
| `libmp3lame` | MP3 audio |

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

## Encoder Detection

### Detection Flow

```typescript
async getAvailableEncoders(): Promise<{ video: string[], audio: string[] }> {
  // 1. Execute ffmpeg -encoders
  const output = await this.execFfmpeg(['-encoders']);

  // 2. Parse output
  const videoEncoders = this.parseVideoEncoders(output);
  const audioEncoders = this.parseAudioEncoders(output);

  // 3. Load whitelist
  const whitelist = this.loadWhitelist();

  // 4. Filter to whitelisted only
  return {
    video: videoEncoders.filter(e => whitelist.video.includes(e)),
    audio: audioEncoders.filter(e => whitelist.audio.includes(e))
  };
}
```

### FFmpeg -encoders Output

```
 V..... = Video
 A..... = Audio
 S..... = Subtitle
 .F.... = Frame-level multithreading
 ..S... = Slice-level multithreading
 ...X.. = Codec is experimental
 ....B. = Supports draw_horiz_band
 .....D = Supports direct rendering

 V..... libx264              libx264 H.264 / AVC / MPEG-4 AVC
 V..... libx265              libx265 H.265 / HEVC
 V..... h264_nvenc           NVIDIA NVENC H.264 encoder
 A..... aac                  AAC (Advanced Audio Coding)
```

### Parsing Logic

```typescript
private parseVideoEncoders(output: string): string[] {
  const lines = output.split('\n');
  const encoders: string[] = [];

  for (const line of lines) {
    // Match lines starting with 'V' (video encoder)
    const match = line.match(/^\s*V[.\w]+\s+(\w+)\s+/);
    if (match) {
      encoders.push(match[1]);
    }
  }

  return encoders;
}
```

## Process Management

### Process Map

```typescript
class FFmpegHandler {
  private runningProcesses: Map<string, ChildProcess> = new Map();

  // Key: OutputGroup ID
  // Value: spawned child process
}
```

### Starting a Process

```typescript
async start(group: OutputGroupDTO, incomingUrl: string): Promise<ProcessInfo> {
  // Check if already running
  if (this.runningProcesses.has(group.id)) {
    throw new Error(`Stream for group ${group.id} is already running`);
  }

  // Build command arguments
  const args = this.buildArgs(group, incomingUrl);

  // Spawn FFmpeg process
  const process = spawn(this.ffmpegPath, args, {
    stdio: ['ignore', 'pipe', 'pipe']
  });

  // Handle output
  process.stderr.on('data', (data) => {
    // FFmpeg outputs to stderr
    Logger.getInstance().ffmpeg(data.toString());
  });

  // Handle exit
  process.on('exit', (code, signal) => {
    this.runningProcesses.delete(group.id);
    Logger.getInstance().log(
      `FFmpeg process ${group.id} exited: code=${code}, signal=${signal}`
    );
  });

  // Handle errors
  process.on('error', (error) => {
    Logger.getInstance().error(`FFmpeg error: ${error.message}`);
    this.runningProcesses.delete(group.id);
  });

  // Store process
  this.runningProcesses.set(group.id, process);

  return {
    pid: process.pid,
    groupId: group.id,
    status: 'running'
  };
}
```

### Stopping Processes

```typescript
async stop(groupId: string): Promise<void> {
  const process = this.runningProcesses.get(groupId);

  if (!process) {
    throw new Error(`No running stream for group ${groupId}`);
  }

  // Send SIGTERM for graceful shutdown
  process.kill('SIGTERM');

  // Wait for process to exit
  await new Promise<void>((resolve) => {
    const timeout = setTimeout(() => {
      // Force kill if not terminated
      process.kill('SIGKILL');
      resolve();
    }, 5000);

    process.on('exit', () => {
      clearTimeout(timeout);
      resolve();
    });
  });

  this.runningProcesses.delete(groupId);
}

async stopAll(): Promise<void> {
  const promises = Array.from(this.runningProcesses.keys())
    .map(groupId => this.stop(groupId));

  await Promise.all(promises);
}
```

## Command Building

### Arguments Builder

```typescript
private buildArgs(group: OutputGroupDTO, incomingUrl: string): string[] {
  const args: string[] = [];

  // Input
  args.push('-i', incomingUrl);

  // Video encoding
  args.push('-c:v', group.videoEncoder);
  args.push('-s', group.resolution);
  args.push('-b:v', `${group.videoBitrate}k`);
  args.push('-r', String(group.fps));

  // Audio encoding
  args.push('-c:a', group.audioCodec);
  args.push('-b:a', `${group.audioBitrate}k`);

  // PTS generation (for timestamp issues)
  if (group.generatePts) {
    args.push('-fflags', '+genpts');
  }

  // Outputs
  for (const target of group.streamTargets) {
    args.push('-f', 'flv', target.normalizedPath);
  }

  return args;
}
```

### Common Options

| Option | Description | Example |
|--------|-------------|---------|
| `-c:v` | Video codec | `libx264` |
| `-c:a` | Audio codec | `aac` |
| `-s` | Resolution | `1920x1080` |
| `-b:v` | Video bitrate | `6000k` |
| `-b:a` | Audio bitrate | `128k` |
| `-r` | Frame rate | `30` |
| `-f` | Output format | `flv` |
| `-fflags` | Format flags | `+genpts` |

### Advanced Options

```typescript
// Hardware acceleration input
args.push('-hwaccel', 'cuda');

// Preset for x264
args.push('-preset', 'veryfast');

// Profile for compatibility
args.push('-profile:v', 'high');

// Keyframe interval
args.push('-g', '60');  // GOP size

// B-frames
args.push('-bf', '2');
```

## Error Handling

### Common FFmpeg Errors

| Error | Cause | Solution |
|-------|-------|----------|
| "Connection refused" | RTMP server not running | Start RTMP server |
| "Unknown encoder" | Encoder not available | Use different encoder |
| "Permission denied" | File access issue | Check permissions |
| "Invalid data" | Corrupt input stream | Restart input |

### Error Recovery

```typescript
process.on('exit', (code) => {
  if (code !== 0) {
    Logger.getInstance().error(`FFmpeg exited with error code ${code}`);

    // Notify frontend
    mainWindow.webContents.send('stream:error', {
      groupId: group.id,
      code,
      message: 'Stream encoding failed'
    });
  }

  this.runningProcesses.delete(group.id);
});
```

## Performance Considerations

### CPU vs GPU Encoding

| Aspect | CPU (libx264) | GPU (nvenc) |
|--------|---------------|-------------|
| Quality | Excellent | Good |
| Speed | Slower | Very Fast |
| CPU Usage | High | Low |
| GPU Usage | None | Moderate |
| Latency | Higher | Lower |

### Recommended Settings

**Streaming to YouTube/Twitch:**
- Codec: `libx264` or `h264_nvenc`
- Resolution: `1920x1080` or `1280x720`
- Video Bitrate: `4500-6000k`
- Audio Bitrate: `128-320k`
- FPS: `30` or `60`

**Multi-platform Streaming:**
- Use same encoding for all targets
- Consider total upload bandwidth
- Lower bitrate for more targets

## Logging

### FFmpeg Output Logging

```typescript
// FFmpeg outputs progress and errors to stderr
process.stderr.on('data', (data) => {
  const message = data.toString();

  // Parse frame progress
  const frameMatch = message.match(/frame=\s*(\d+)/);
  if (frameMatch) {
    const frame = parseInt(frameMatch[1]);
    // Can use for progress tracking
  }

  // Log to ffmpeg.log
  Logger.getInstance().ffmpeg(message);
});
```

### Sample FFmpeg Output

```
frame=  100 fps= 30 q=28.0 size=     256kB time=00:00:03.33 bitrate= 629.3kbits/s
frame=  200 fps= 30 q=28.0 size=     512kB time=00:00:06.67 bitrate= 628.9kbits/s
```
