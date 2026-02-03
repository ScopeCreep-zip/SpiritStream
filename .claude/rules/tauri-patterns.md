# Tauri Development Patterns

These patterns apply to the Tauri desktop wrapper in `apps/desktop/src-tauri/`.

## Architecture

SpiritStream uses a **thin Tauri wrapper** that:
1. Spawns the backend server as a sidecar process
2. Displays the web UI in a webview
3. Handles platform-specific permissions

The actual business logic lives in `server/`, not in Tauri commands.

## Sidecar Pattern

### Spawning the server
```rust
let sidecar = app.shell().sidecar("spiritstream-server")?;
let (mut rx, child) = sidecar
    .args([
        "--host", &host,
        "--port", &port,
    ])
    .spawn()?;
```

### Health check before showing UI
```rust
async fn wait_for_server(url: &str, timeout: Duration) -> bool {
    let client = reqwest::Client::new();
    let start = Instant::now();

    while start.elapsed() < timeout {
        if client.get(format!("{url}/health")).send().await.is_ok() {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    false
}
```

### Clean shutdown
```rust
app.run(|app_handle, event| {
    if let RunEvent::ExitRequested { .. } = event {
        // Kill sidecar process
        if let Some(child) = state.child.lock().unwrap().take() {
            let _ = child.kill();
        }
    }
});
```

## Tauri Commands

### Keep commands minimal - delegate to HTTP API
```rust
// Good: Permission checks that need native APIs
#[tauri::command]
async fn check_permissions() -> HashMap<String, bool> {
    permissions::check_all()
}

// Bad: Business logic that should be in the server
#[tauri::command]
async fn start_stream(group: OutputGroup) -> Result<(), String> {
    // Don't do this - call HTTP API instead
}
```

### Use proper error types
```rust
#[tauri::command]
async fn request_permission(permission: String) -> Result<bool, String> {
    permissions::request(&permission)
        .map_err(|e| format!("Permission request failed: {e}"))
}
```

## State Management

### Use Tauri's managed state for sidecar process
```rust
struct ServerProcess {
    child: Mutex<Option<CommandChild>>,
    pid: Mutex<Option<u32>>,
}

// In setup
app.manage(ServerProcess {
    child: Mutex::new(None),
    pid: Mutex::new(None),
});
```

### Access state in commands
```rust
#[tauri::command]
fn get_server_pid(state: State<ServerProcess>) -> Option<u32> {
    *state.pid.lock().unwrap()
}
```

## Window Management

### Configure in tauri.conf.json
```json
{
  "app": {
    "windows": [{
      "title": "SpiritStream",
      "width": 1280,
      "height": 800,
      "visible": false,
      "decorations": true
    }]
  }
}
```

### Show window after server ready
```rust
if wait_for_server(&url, Duration::from_secs(30)).await {
    window.show()?;
    window.set_focus()?;
} else {
    // Show error dialog
}
```

## Plugins

### Use official Tauri plugins
```rust
tauri::Builder::default()
    .plugin(tauri_plugin_http::init())    // HTTP requests
    .plugin(tauri_plugin_shell::init())   // Sidecar spawning
    .plugin(tauri_plugin_fs::init())      // File system access
    .plugin(tauri_plugin_dialog::init())  // Native dialogs
    .plugin(tauri_plugin_log::init())     // Logging
```

### Configure capabilities in capabilities/*.json
```json
{
  "identifier": "default",
  "windows": ["main"],
  "permissions": [
    "shell:allow-spawn",
    "shell:allow-execute",
    "fs:default",
    "dialog:default"
  ]
}
```

## Security & Capabilities (Tauri 2.x)

Tauri 2.x uses a **default-deny** security model with capabilities, permissions, and scopes.

### Capabilities System
Capabilities define which permissions are granted to which windows/webviews.

```json
// src-tauri/capabilities/default.json
{
  "$schema": "../gen/schemas/desktop-schema.json",
  "identifier": "default",
  "description": "Default capabilities for main window",
  "windows": ["main"],
  "permissions": [
    "core:default",
    "shell:allow-spawn",
    "fs:read-files",
    "dialog:default",
    {
      "identifier": "fs:scope",
      "allow": [{ "path": "$APPDATA/**" }]
    }
  ]
}
```

### Permission Types
- **Commands**: Enable specific Tauri commands (`shell:allow-spawn`)
- **Scopes**: Fine-grained path/resource restrictions
- **Deny takes precedence**: If a path is denied, it's blocked even if allowed elsewhere

### Window-Level Security
```json
// Different capabilities for different windows
{
  "identifier": "settings-window",
  "windows": ["settings"],
  "permissions": [
    "core:default"
    // Fewer permissions for settings window
  ]
}
```

### Never expose full shell access
```json
// Bad - allows arbitrary command execution
"shell:allow-execute"

// Good - only allow specific sidecars
"shell:allow-spawn"
```

### Use allowlist for IPC
```rust
.invoke_handler(tauri::generate_handler![
    // Only expose necessary commands
    permissions::check_permissions,
    permissions::request_permission,
])
```

### CSP for webview
```json
{
  "app": {
    "security": {
      "csp": "default-src 'self'; connect-src 'self' http://127.0.0.1:8008 ws://127.0.0.1:8008"
    }
  }
}
```

### Trust Boundary Principles
- Rust core has full system access - validate all inputs from frontend
- WebView only accesses what's explicitly exposed via IPC
- Always sanitize data passed between boundaries
- Keep business logic in the backend server, not Tauri commands

## Platform-Specific Code

### Permission handling (macOS)
```rust
#[cfg(target_os = "macos")]
pub fn check_screen_capture() -> bool {
    use core_graphics::access::ScreenCaptureAccess;
    ScreenCaptureAccess::preflight()
}

#[cfg(not(target_os = "macos"))]
pub fn check_screen_capture() -> bool {
    true // Other platforms don't need explicit permission
}
```

### Window attributes
```rust
#[cfg(target_os = "macos")]
{
    use tauri::TitleBarStyle;
    window.set_title_bar_style(TitleBarStyle::Overlay)?;
}
```

## Logging

### Configure log targets
```rust
let mut targets = vec![
    Target::new(TargetKind::LogDir {
        file_name: Some("spiritstream".to_string()),
    }),
    Target::new(TargetKind::Webview),
];

if cfg!(debug_assertions) {
    targets.push(Target::new(TargetKind::Stdout));
}
```

## Frontend Communication

### Emit events to webview
```rust
app.emit("server-ready", json!({ "url": server_url }))?;
app.emit("server-error", json!({ "message": error }))?;
```

### Listen in frontend
```typescript
import { listen } from '@tauri-apps/api/event';

listen('server-ready', (event) => {
  console.log('Server URL:', event.payload.url);
});
```
