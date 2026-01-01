# Tauri Migration Plan

## Overview

The project is on the `tauri-shift` branch, indicating a planned migration from Electron to Tauri. This document outlines the migration strategy.

## Why Tauri?

| Aspect | Electron | Tauri |
|--------|----------|-------|
| Bundle Size | ~150MB+ | ~5-10MB |
| Memory Usage | Higher (Chromium) | Lower (System WebView) |
| Startup Time | Slower | Faster |
| Language | JavaScript/TypeScript | Rust (backend) |
| Security | Good with proper config | Stronger by default |

## Migration Scope

### What Changes

1. **Backend Layer**
   - Node.js → Rust
   - Express/Electron main → Tauri commands
   - Child process (FFmpeg) → Rust subprocess

2. **IPC Layer**
   - Electron IPC → Tauri invoke/emit
   - Preload script → No longer needed
   - contextBridge → Direct invoke

3. **Build System**
   - electron-builder → Tauri bundler
   - Package.json scripts → Cargo + npm

### What Stays the Same

1. **Frontend**
   - HTML/CSS/JS structure
   - UI components and logic
   - Styling

2. **Business Logic (Conceptually)**
   - Profile management
   - Stream encoding
   - Encryption (different implementation)

3. **Models (Conceptually)**
   - Profile, OutputGroup, StreamTarget
   - Same data structures

## Migration Steps

### Phase 1: Setup
- [ ] Install Tauri CLI
- [ ] Initialize Tauri project
- [ ] Configure tauri.conf.json
- [ ] Set up Rust workspace

### Phase 2: Backend Port
- [ ] Port ProfileManager to Rust
- [ ] Port FFmpegHandler to Rust
- [ ] Port Encryption to Rust
- [ ] Port Logger to Rust

### Phase 3: IPC Migration
- [ ] Define Tauri commands
- [ ] Update frontend to use `@tauri-apps/api`
- [ ] Remove Electron-specific code
- [ ] Test all IPC operations

### Phase 4: Frontend Updates
- [ ] Update API calls to Tauri invoke
- [ ] Remove preload dependencies
- [ ] Test UI functionality
- [ ] Update error handling

### Phase 5: Build & Package
- [ ] Configure Tauri bundler
- [ ] Test on all platforms
- [ ] Update CI/CD pipelines
- [ ] Create release builds

## Code Mapping

### Electron to Tauri Command Mapping

| Electron IPC | Tauri Command |
|--------------|---------------|
| `profile:load` | `load_profile` |
| `profile:save` | `save_profile` |
| `profile:delete` | `delete_profile` |
| `ffmpeg:start` | `start_stream` |
| `ffmpeg:stop` | `stop_stream` |

### Frontend API Changes

**Before (Electron):**
```javascript
const profile = await window.electronAPI.profileManager.load(name);
```

**After (Tauri):**
```javascript
import { invoke } from '@tauri-apps/api';
const profile = await invoke('load_profile', { name });
```

## Rust Structure

```
src-tauri/
├── Cargo.toml
├── src/
│   ├── main.rs           # Entry point
│   ├── commands/         # Tauri commands
│   │   ├── mod.rs
│   │   ├── profile.rs
│   │   ├── ffmpeg.rs
│   │   └── logger.rs
│   ├── services/         # Business logic
│   │   ├── mod.rs
│   │   ├── profile_manager.rs
│   │   ├── ffmpeg_handler.rs
│   │   └── encryption.rs
│   └── models/           # Data structures
│       ├── mod.rs
│       ├── profile.rs
│       ├── output_group.rs
│       └── stream_target.rs
└── tauri.conf.json
```

## Key Differences

### Process Management
```rust
// Tauri (Rust)
use std::process::Command;

let child = Command::new("ffmpeg")
    .args(&["-i", &input_url, "-c:v", "libx264", ...])
    .spawn()?;
```

### Encryption
```rust
// Tauri (Rust) - using ring or aes-gcm crate
use aes_gcm::{Aes256Gcm, Key, Nonce};
use aes_gcm::aead::Aead;

let key = derive_key(password, &salt);
let cipher = Aes256Gcm::new(&key);
let ciphertext = cipher.encrypt(&nonce, plaintext)?;
```

### File Operations
```rust
// Tauri (Rust)
use std::fs;
use tauri::api::path::app_data_dir;

let data_dir = app_data_dir(&config)?;
let profiles_dir = data_dir.join("profiles");
fs::create_dir_all(&profiles_dir)?;
```

## Testing Strategy

1. **Unit Tests** (Rust)
   - Test each service independently
   - Mock file system operations
   - Test encryption/decryption

2. **Integration Tests**
   - Test Tauri commands
   - Test IPC communication
   - Test FFmpeg spawning

3. **E2E Tests**
   - Test full workflows
   - Test UI interactions
   - Test cross-platform behavior

## Rollback Plan

If migration fails:
1. Keep Electron code on `main` branch
2. Merge only after full testing
3. Maintain both until stable
4. Tag releases appropriately

## Resources

- [Tauri Documentation](https://tauri.app/v1/guides/)
- [Tauri Commands](https://tauri.app/v1/guides/features/command)
- [Rust Book](https://doc.rust-lang.org/book/)
- [Tauri + TypeScript](https://tauri.app/v1/guides/getting-started/prerequisites)
