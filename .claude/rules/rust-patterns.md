# Rust Coding Patterns

These patterns apply to all Rust code in `server/` and `apps/desktop/src-tauri/`.

## Naming Conventions

- **Structs/Enums/Traits**: PascalCase (`FFmpegHandler`, `OutputGroup`)
- **Functions/Methods**: snake_case (`start_stream`, `get_encoders`)
- **Constants**: UPPER_SNAKE_CASE (`DEFAULT_PORT`, `RELAY_HOST`)
- **Modules**: snake_case (`ffmpeg_handler`, `profile_manager`)
- **Type parameters**: Single uppercase letter or PascalCase (`T`, `E`, `State`)

## Error Handling

### Use Result for fallible operations
```rust
pub fn load_profile(&self, name: &str) -> Result<Profile, String> {
    // Return descriptive error messages
    let content = fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read profile: {e}"))?;
    Ok(profile)
}
```

### Avoid unwrap() in production code
```rust
// Bad
let value = map.get("key").unwrap();

// Good
let value = map.get("key").ok_or("Key not found")?;
```

### Use anyhow for application code, thiserror for libraries
```rust
// Application code (main.rs, commands) - use anyhow
use anyhow::{Context, Result};

fn process() -> Result<()> {
    do_thing().context("Failed to do thing")?;
    Ok(())
}

// Library/service code - use thiserror for typed errors
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ProfileError {
    #[error("Profile not found: {0}")]
    NotFound(String),

    #[error("Failed to read profile")]
    ReadError(#[from] std::io::Error),

    #[error("Invalid profile format")]
    ParseError(#[source] serde_json::Error),
}

// Don't create too many variants - group related errors
// Always derive Debug on error types
// Always use #[source] or #[from] to preserve error chain
```

## Struct Patterns

### Builder pattern for complex configs
```rust
pub struct StreamConfig {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
}

impl Default for StreamConfig {
    fn default() -> Self {
        Self { width: 1920, height: 1080, fps: 30 }
    }
}
```

### Derive common traits
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutputGroup {
    pub id: String,
    pub name: String,
}
```

## Concurrency

### Use Arc<Mutex<T>> for shared mutable state
```rust
pub struct Handler {
    processes: Arc<Mutex<HashMap<String, ProcessInfo>>>,
}
```

### Async best practices (Tokio)
```rust
// Async I/O - futures are lazy, always .await them
tokio::spawn(async move {
    handle_connection(stream).await;
});

// Run multiple futures in parallel with join!
let (result1, result2) = tokio::join!(
    fetch_data(),
    process_stream()
);

// Or try_join! for Result-returning futures
let (a, b) = tokio::try_join!(async_op1(), async_op2())?;
```

### Never block the async runtime
```rust
// BAD - blocks the executor
std::thread::sleep(Duration::from_secs(1));

// GOOD - use spawn_blocking for blocking operations
tokio::task::spawn_blocking(move || {
    // CPU-bound or blocking I/O (FFmpeg stderr parsing)
    parse_ffmpeg_output(reader);
}).await?;

// GOOD - use async sleep
tokio::time::sleep(Duration::from_secs(1)).await;
```

### Lifetime considerations across .await
```rust
// BAD - reference may not live long enough
async fn process(data: &str) {
    some_async_op().await;  // data must be valid here
    println!("{}", data);
}

// GOOD - prefer owned data across await points
async fn process(data: String) {
    some_async_op().await;
    println!("{}", data);
}

// Or use Arc for shared ownership
let shared = Arc::new(data);
```

### Use atomic types for simple counters
```rust
relay_refcount: Arc<AtomicUsize>,

// Increment
self.relay_refcount.fetch_add(1, Ordering::SeqCst);
```

## Module Organization

```
server/src/
├── main.rs          # Entry point, router setup
├── lib.rs           # Re-exports for external use
├── commands/        # HTTP command handlers
│   └── mod.rs
├── models/          # Data structures, DTOs
│   ├── mod.rs
│   ├── profile.rs
│   └── settings.rs
└── services/        # Business logic
    ├── mod.rs
    ├── ffmpeg_handler.rs
    └── profile_manager.rs
```

## Serde Patterns

### Use camelCase for JSON interop with TypeScript
```rust
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamTarget {
    pub stream_key: String,  // Serializes as "streamKey"
}
```

### Default values for optional fields
```rust
#[derive(Deserialize)]
pub struct Settings {
    #[serde(default)]
    pub start_minimized: bool,

    #[serde(default = "default_port")]
    pub port: u16,
}

fn default_port() -> u16 { 8008 }
```

## Platform-Specific Code

### Use cfg attributes
```rust
#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

#[cfg(target_os = "macos")]
fn get_screen_capture_permission() -> bool {
    // macOS-specific implementation
}
```

## Documentation

### Document public APIs
```rust
/// Manages FFmpeg streaming processes.
///
/// Handles relay setup, process spawning, and real-time stats collection.
pub struct FFmpegHandler {
    // ...
}

/// Start streaming to the configured targets.
///
/// # Arguments
/// * `group` - Output group configuration
/// * `incoming_url` - Source RTMP URL
///
/// # Errors
/// Returns error if FFmpeg fails to start or relay setup fails.
pub fn start_stream(&self, group: OutputGroup, incoming_url: &str) -> Result<(), String>
```

## Logging

### Use the log crate with appropriate levels
```rust
use log::{debug, info, warn, error};

info!("Starting stream for group: {}", group.id);
debug!("FFmpeg args: {:?}", args);
warn!("Relay already running, reusing");
error!("Failed to spawn FFmpeg: {}", e);
```

### Redact sensitive data
```rust
// Never log stream keys directly
info!("Connecting to target: {}", redact_stream_key(&url));
```
