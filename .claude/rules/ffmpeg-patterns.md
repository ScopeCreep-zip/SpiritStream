# FFmpeg Integration Patterns

These patterns apply to FFmpeg process management in `server/src/services/ffmpeg_handler.rs`.

## Integration Approach

SpiritStream uses **process spawning** (Command) rather than FFI bindings for FFmpeg integration.
This approach offers:
- Simpler error handling (no unsafe code)
- Process isolation (crashes don't affect the main app)
- Easier debugging (standard FFmpeg CLI)
- No FFmpeg version compatibility concerns

**Alternative approaches** (not currently used):
- `ez-ffmpeg` - Safe Rust interface with async support
- `rsmpeg` - Thin FFI wrapper (supports FFmpeg 6.x, 7.x)
- `rust-ffmpeg` - Maintenance-mode FFI bindings

## Process Spawning

### Use Command with proper configuration
```rust
let mut cmd = Command::new(&self.ffmpeg_path);
cmd.args(&args)
    .stdin(Stdio::null())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped());

// Windows: Hide console window
#[cfg(windows)]
cmd.creation_flags(CREATE_NO_WINDOW);

let child = cmd.spawn()
    .map_err(|e| format!("Failed to spawn FFmpeg: {e}"))?;
```

### Track processes for cleanup
```rust
struct ProcessInfo {
    child: Child,
    start_time: Instant,
    group_id: String,
}

// Store in HashMap for management
self.processes.lock().unwrap().insert(group_id.clone(), ProcessInfo {
    child,
    start_time: Instant::now(),
    group_id,
});
```

## Relay Architecture

### Single relay for multiple outputs
```
RTMP Input → FFmpeg Relay → TCP localhost:20001 → Output Group 1
                          → TCP localhost:20002 → Output Group 2
                          → TCP localhost:20003 → Output Group 3
```

### Port allocation
```rust
const RELAY_HOST: &str = "localhost";
const RELAY_PORT_BASE: u16 = 20000;
const RELAY_PORT_RANGE: u16 = 20000;

fn get_relay_port(group_id: &str) -> u16 {
    let hash = calculate_hash(group_id);
    RELAY_PORT_BASE + (hash % RELAY_PORT_RANGE) as u16
}
```

### Relay with tee muxer for fan-out
```rust
// Build tee output for multiple destinations
let tee_outputs: Vec<String> = ports.iter()
    .map(|port| format!(
        "[f=mpegts]{options}tcp://{host}:{port}?{query}",
        options = RELAY_TEE_FIFO_OPTIONS,
        host = RELAY_HOST,
        port = port,
        query = RELAY_TCP_OUT_QUERY
    ))
    .collect();

args.extend(["-f", "tee", &tee_outputs.join("|")]);
```

## Command Building

### Input configuration
```rust
// RTMP input with timeout
args.extend([
    "-rtmp_live", "live",
    "-timeout", &format!("{}", RELAY_RTMP_TIMEOUT_SECS),
    "-tcp_nodelay", RELAY_RTMP_TCP_NODELAY,
    "-i", incoming_url,
]);
```

### Video encoding
```rust
match encoder.as_str() {
    "libx264" => args.extend([
        "-c:v", "libx264",
        "-preset", "veryfast",
        "-tune", "zerolatency",
        "-b:v", &format!("{}k", bitrate),
    ]),
    "h264_videotoolbox" => args.extend([
        "-c:v", "h264_videotoolbox",
        "-b:v", &format!("{}k", bitrate),
        "-realtime", "1",
    ]),
    // ... other encoders
}
```

### Audio encoding
```rust
args.extend([
    "-c:a", "aac",
    "-b:a", &format!("{}k", audio_bitrate),
    "-ar", "48000",
    "-ac", "2",
]);
```

### Output to RTMP
```rust
args.extend([
    "-f", "flv",
    &format!("{}/{}", target.url, target.stream_key),
]);
```

## Stats Collection

### Parse stderr for progress
```rust
fn parse_ffmpeg_output(reader: BufReader<ChildStderr>, stats_tx: Sender<StreamStats>) {
    for line in reader.lines().filter_map(|l| l.ok()) {
        if let Some(stats) = parse_progress_line(&line) {
            let _ = stats_tx.send(stats);
        }
    }
}

fn parse_progress_line(line: &str) -> Option<StreamStats> {
    // frame=  123 fps= 30 q=28.0 size=    1234kB time=00:00:04.10 bitrate=2468.5kbits/s
    let frame = extract_value(line, "frame=")?;
    let fps = extract_value(line, "fps=")?;
    let bitrate = extract_value(line, "bitrate=")?;
    // ...
}
```

### Emit stats events
```rust
emit_event(&event_sink, "stream_stats", json!({
    "groupId": group_id,
    "fps": stats.fps,
    "bitrate": stats.bitrate,
    "droppedFrames": stats.dropped_frames,
    "uptime": stats.uptime_seconds,
}));
```

## Native Capture (stdin pipe)

### Video frame input
```rust
pub struct NativeVideoConfig {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub pixel_format: String, // "rgb24", "bgra", "nv12"
}

// FFmpeg args for stdin input
args.extend([
    "-f", "rawvideo",
    "-pix_fmt", &config.pixel_format,
    "-s", &format!("{}x{}", config.width, config.height),
    "-r", &config.fps.to_string(),
    "-i", "pipe:0",
]);
```

### Write frames to stdin
```rust
pub struct NativeCaptureHandle {
    pub stdin: ChildStdin,
}

impl NativeCaptureHandle {
    pub fn write_frame(&mut self, data: &[u8]) -> std::io::Result<()> {
        self.stdin.write_all(data)?;
        self.stdin.flush()
    }
}
```

## Process Lifecycle

### Graceful shutdown
```rust
pub fn stop_stream(&self, group_id: &str) -> Result<(), String> {
    // Mark as stopping to prevent race conditions
    self.stopping_groups.lock().unwrap().insert(group_id.to_string());

    // Get process handle
    let mut processes = self.processes.lock().unwrap();
    if let Some(mut info) = processes.remove(group_id) {
        // Try graceful shutdown first
        let _ = info.child.kill();
        let _ = info.child.wait();
    }

    self.stopping_groups.lock().unwrap().remove(group_id);
    Ok(())
}
```

### Stop all on exit
```rust
pub fn stop_all(&self) {
    let mut processes = self.processes.lock().unwrap();
    for (_, mut info) in processes.drain() {
        let _ = info.child.kill();
    }

    // Also stop relay
    if let Some(mut relay) = self.relay.lock().unwrap().take() {
        let _ = relay.child.kill();
    }
}
```

## Error Handling

### Check FFmpeg availability
```rust
pub fn test_ffmpeg(&self) -> Result<String, String> {
    let output = Command::new(&self.ffmpeg_path)
        .args(["-version"])
        .output()
        .map_err(|e| format!("Failed to run FFmpeg: {e}"))?;

    if !output.status.success() {
        return Err("FFmpeg returned non-zero exit code".to_string());
    }

    String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()
        .map(|s| s.to_string())
        .ok_or("No version output".to_string())
}
```

### Handle process crashes
```rust
// Monitor in background thread
thread::spawn(move || {
    let status = child.wait();
    if !is_intentional_stop {
        emit_event(&event_sink, "stream_error", json!({
            "groupId": group_id,
            "error": format!("FFmpeg exited: {:?}", status),
        }));
    }
});
```

## Security

### Redact stream keys in logs
```rust
fn redact_stream_key(url: &str) -> String {
    // rtmp://server/app/STREAM_KEY -> rtmp://server/app/****
    if let Some(idx) = url.rfind('/') {
        format!("{}/*****", &url[..idx])
    } else {
        url.to_string()
    }
}

info!("Streaming to: {}", redact_stream_key(&target_url));
```

### Validate paths
```rust
pub fn validate_ffmpeg_path(path: &str) -> Result<(), String> {
    let path = Path::new(path);

    if !path.exists() {
        return Err("Path does not exist".to_string());
    }

    if !path.is_file() {
        return Err("Path is not a file".to_string());
    }

    // Check it's actually FFmpeg
    let output = Command::new(path).args(["-version"]).output()
        .map_err(|_| "Cannot execute file")?;

    if !String::from_utf8_lossy(&output.stdout).contains("ffmpeg") {
        return Err("Not a valid FFmpeg binary".to_string());
    }

    Ok(())
}
```
