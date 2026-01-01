# Tauri Migration Plan

## Overview

MagillaStream is undergoing a complete lift-and-shift from Electron to Tauri 2.x. This is a full rewrite with **NO backwards compatibility**.

## Migration Summary

| Aspect | Current (Electron) | Target (Tauri) |
|--------|-------------------|----------------|
| Desktop Framework | Electron 34.x | Tauri 2.x |
| Backend Runtime | Node.js | Rust |
| Frontend Framework | Vanilla JS | React 18+ |
| Styling | Plain CSS | Tailwind CSS v4 |
| Build Tool | electron-builder | Vite + Tauri |
| Bundle Size | ~150MB+ | ~5-10MB |
| Memory Usage | Higher | Lower |

## Phase 1: Project Setup

### Prerequisites

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install Tauri CLI
cargo install tauri-cli

# Install Node.js 18+
# Already installed
```

### Initialize Project

```bash
# Create new Tauri project with React template
npm create tauri-app@latest magillastream-v2 -- --template react-ts

# Or add Tauri to existing project
cd magillastream
npm install @tauri-apps/cli @tauri-apps/api
npm run tauri init
```

### Directory Structure

```
magillastream/
├── src-frontend/          # NEW: React frontend
├── src-tauri/             # NEW: Rust backend
├── src/                   # OLD: Will be removed
├── package.json           # Updated for Vite + React
├── vite.config.ts         # NEW
├── tailwind.config.js     # NEW
└── tsconfig.json          # Updated
```

## Phase 2: Frontend Migration

### Step 2.1: Setup React + Tailwind

```bash
# Install React
npm install react react-dom
npm install -D @types/react @types/react-dom

# Install Tailwind CSS v4
npm install -D tailwindcss @tailwindcss/vite

# Install Radix UI primitives
npm install @radix-ui/react-dialog @radix-ui/react-select @radix-ui/react-switch

# Install utilities
npm install zustand clsx tailwind-merge framer-motion
```

### Step 2.2: Create Design System Tokens

Create `src-frontend/styles/tokens.css` with all CSS custom properties from the design system document.

### Step 2.3: Build Component Library

Priority order:
1. Base components: Button, Card, Input, Select, Switch
2. Layout: Sidebar, Header, MainContent
3. Features: ProfileCard, OutputGroupCard, StreamTargetCard
4. Specialized: StreamStatus, ThemeToggle, Modal

### Step 2.4: Implement State Management

```typescript
// stores/profileStore.ts
import { create } from 'zustand';
import { invoke } from '@tauri-apps/api';

interface ProfileState {
  profiles: string[];
  current: Profile | null;
  loading: boolean;
  loadProfiles: () => Promise<void>;
  loadProfile: (name: string, password?: string) => Promise<void>;
  // ...
}

export const useProfileStore = create<ProfileState>((set) => ({
  profiles: [],
  current: null,
  loading: false,
  loadProfiles: async () => {
    set({ loading: true });
    const profiles = await invoke<string[]>('get_all_profiles');
    set({ profiles, loading: false });
  },
  // ...
}));
```

## Phase 3: Backend Migration

### Step 3.1: Define Models (Rust)

```rust
// src-tauri/src/models/profile.rs
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub incoming_url: String,
    pub output_groups: Vec<OutputGroup>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputGroup {
    pub id: String,
    pub video_encoder: String,
    pub resolution: String,
    pub video_bitrate: u32,
    pub fps: u32,
    pub audio_codec: String,
    pub audio_bitrate: u32,
    pub generate_pts: bool,
    pub stream_targets: Vec<StreamTarget>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamTarget {
    pub id: String,
    pub url: String,
    pub stream_key: String,
    pub port: u16,
}
```

### Step 3.2: Implement Services

```rust
// src-tauri/src/services/profile_manager.rs
use std::path::PathBuf;
use tokio::fs;
use crate::models::Profile;

pub struct ProfileManager {
    profiles_dir: PathBuf,
}

impl ProfileManager {
    pub fn new(app_data_dir: PathBuf) -> Self {
        let profiles_dir = app_data_dir.join("profiles");
        std::fs::create_dir_all(&profiles_dir).ok();
        Self { profiles_dir }
    }

    pub async fn get_all_names(&self) -> Result<Vec<String>, String> {
        let mut names = Vec::new();
        let mut entries = fs::read_dir(&self.profiles_dir)
            .await
            .map_err(|e| e.to_string())?;

        while let Some(entry) = entries.next_entry().await.map_err(|e| e.to_string())? {
            if let Some(name) = entry.path().file_stem() {
                names.push(name.to_string_lossy().to_string());
            }
        }
        Ok(names)
    }

    pub async fn load(&self, name: &str, password: Option<&str>) -> Result<Profile, String> {
        let path = self.profiles_dir.join(format!("{}.json", name));
        let content = fs::read_to_string(&path)
            .await
            .map_err(|e| e.to_string())?;

        // Decrypt if needed
        let json = if let Some(pwd) = password {
            self.decrypt(&content, pwd)?
        } else {
            content
        };

        serde_json::from_str(&json).map_err(|e| e.to_string())
    }
}
```

### Step 3.3: Implement FFmpeg Handler

```rust
// src-tauri/src/services/ffmpeg_handler.rs
use std::collections::HashMap;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use crate::models::OutputGroup;

pub struct FFmpegHandler {
    ffmpeg_path: String,
    processes: Mutex<HashMap<String, Child>>,
}

impl FFmpegHandler {
    pub fn new() -> Self {
        Self {
            ffmpeg_path: Self::find_ffmpeg(),
            processes: Mutex::new(HashMap::new()),
        }
    }

    pub fn start(&self, group: &OutputGroup, incoming_url: &str) -> Result<u32, String> {
        let args = self.build_args(group, incoming_url);

        let child = Command::new(&self.ffmpeg_path)
            .args(&args)
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| e.to_string())?;

        let pid = child.id();
        self.processes.lock().unwrap().insert(group.id.clone(), child);

        Ok(pid)
    }

    pub fn stop(&self, group_id: &str) -> Result<(), String> {
        if let Some(mut child) = self.processes.lock().unwrap().remove(group_id) {
            child.kill().map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    fn build_args(&self, group: &OutputGroup, incoming_url: &str) -> Vec<String> {
        let mut args = vec![
            "-i".to_string(), incoming_url.to_string(),
            "-c:v".to_string(), group.video_encoder.clone(),
            "-s".to_string(), group.resolution.clone(),
            "-b:v".to_string(), format!("{}k", group.video_bitrate),
            "-r".to_string(), group.fps.to_string(),
            "-c:a".to_string(), group.audio_codec.clone(),
            "-b:a".to_string(), format!("{}k", group.audio_bitrate),
        ];

        if group.generate_pts {
            args.extend(["-fflags".to_string(), "+genpts".to_string()]);
        }

        for target in &group.stream_targets {
            args.extend([
                "-f".to_string(), "flv".to_string(),
                format!("{}/{}", target.url, target.stream_key),
            ]);
        }

        args
    }
}
```

### Step 3.4: Register Commands

```rust
// src-tauri/src/main.rs
mod commands;
mod models;
mod services;

use tauri::Manager;
use services::{ProfileManager, FFmpegHandler};

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let app_data_dir = app.path().app_data_dir().unwrap();

            app.manage(ProfileManager::new(app_data_dir.clone()));
            app.manage(FFmpegHandler::new());

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::profile::get_all_profiles,
            commands::profile::load_profile,
            commands::profile::save_profile,
            commands::profile::delete_profile,
            commands::stream::start_stream,
            commands::stream::stop_stream,
            commands::stream::stop_all_streams,
            commands::system::get_encoders,
        ])
        .run(tauri::generate_context!())
        .expect("error running tauri application");
}
```

## Phase 4: Integration

### Step 4.1: Wire Frontend to Backend

```typescript
// src-frontend/lib/tauri.ts
import { invoke } from '@tauri-apps/api';
import type { Profile, OutputGroup, Encoders } from '@/types';

export const api = {
  profile: {
    getAll: () => invoke<string[]>('get_all_profiles'),
    load: (name: string, password?: string) =>
      invoke<Profile>('load_profile', { name, password }),
    save: (profile: Profile, password?: string) =>
      invoke<void>('save_profile', { profile, password }),
    delete: (name: string) => invoke<void>('delete_profile', { name }),
  },
  stream: {
    start: (group: OutputGroup, incomingUrl: string) =>
      invoke<number>('start_stream', { group, incomingUrl }),
    stop: (groupId: string) => invoke<void>('stop_stream', { groupId }),
    stopAll: () => invoke<void>('stop_all_streams'),
  },
  system: {
    getEncoders: () => invoke<Encoders>('get_encoders'),
  },
};
```

### Step 4.2: Update Stores

```typescript
// stores/profileStore.ts
import { api } from '@/lib/tauri';

export const useProfileStore = create<ProfileState>((set, get) => ({
  // ...
  loadProfile: async (name, password) => {
    set({ loading: true, error: null });
    try {
      const profile = await api.profile.load(name, password);
      set({ current: profile, loading: false });
    } catch (error) {
      set({ error: String(error), loading: false });
    }
  },
}));
```

## Phase 5: Build & Package

### Development

```bash
# Start development server
npm run tauri dev
```

### Production Build

```bash
# Build for current platform
npm run tauri build

# Build for specific platform
npm run tauri build -- --target x86_64-pc-windows-msvc
npm run tauri build -- --target x86_64-apple-darwin
npm run tauri build -- --target x86_64-unknown-linux-gnu
```

### Configuration

```json
// src-tauri/tauri.conf.json
{
  "productName": "MagillaStream",
  "version": "2.0.0",
  "identifier": "com.magillastream.app",
  "build": {
    "beforeDevCommand": "npm run dev",
    "beforeBuildCommand": "npm run build",
    "devUrl": "http://localhost:5173",
    "frontendDist": "../dist"
  },
  "bundle": {
    "active": true,
    "targets": ["msi", "dmg", "appimage"],
    "icon": ["icons/icon.png"],
    "resources": ["resources/*"]
  }
}
```

## Phase 6: Cleanup

### Remove Old Code

```bash
# After successful migration
rm -rf src/electron
rm -rf src/models
rm -rf src/utils
rm -rf src/frontend
rm -rf src/shared
rm -rf src/types
```

### Update .gitignore

```gitignore
# Tauri
src-tauri/target/
dist/

# Node
node_modules/

# Build
*.exe
*.dmg
*.AppImage
*.msi
```

## Migration Checklist

### Phase 1: Setup
- [ ] Install Rust toolchain
- [ ] Install Tauri CLI
- [ ] Initialize Tauri project
- [ ] Configure Vite

### Phase 2: Frontend
- [ ] Set up React + TypeScript
- [ ] Configure Tailwind CSS v4
- [ ] Create design tokens
- [ ] Build base components
- [ ] Build feature components
- [ ] Implement state management

### Phase 3: Backend
- [ ] Define Rust models
- [ ] Implement ProfileManager
- [ ] Implement FFmpegHandler
- [ ] Implement Encryption
- [ ] Register Tauri commands

### Phase 4: Integration
- [ ] Create API wrapper
- [ ] Connect stores to API
- [ ] Test all features
- [ ] Handle errors

### Phase 5: Build
- [ ] Test development build
- [ ] Create production build
- [ ] Test on all platforms
- [ ] Configure signing

### Phase 6: Cleanup
- [ ] Remove old Electron code
- [ ] Update documentation
- [ ] Update CI/CD
- [ ] Tag release

## Dependencies

### Frontend (package.json)

```json
{
  "dependencies": {
    "react": "^18.2.0",
    "react-dom": "^18.2.0",
    "@tauri-apps/api": "^2.0.0",
    "@radix-ui/react-dialog": "^1.0.5",
    "@radix-ui/react-select": "^2.0.0",
    "@radix-ui/react-switch": "^1.0.3",
    "@radix-ui/react-icons": "^1.3.0",
    "zustand": "^4.4.0",
    "clsx": "^2.0.0",
    "tailwind-merge": "^2.0.0",
    "framer-motion": "^10.16.0"
  },
  "devDependencies": {
    "@tauri-apps/cli": "^2.0.0",
    "@types/react": "^18.2.0",
    "@types/react-dom": "^18.2.0",
    "typescript": "^5.0.0",
    "tailwindcss": "^4.0.0",
    "@tailwindcss/vite": "^4.0.0",
    "vite": "^5.0.0",
    "@vitejs/plugin-react": "^4.0.0"
  }
}
```

### Backend (Cargo.toml)

```toml
[package]
name = "magillastream"
version = "2.0.0"
edition = "2021"

[dependencies]
tauri = { version = "2.0", features = ["shell-all"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
tokio = { version = "1.0", features = ["full"] }
aes-gcm = "0.10"
argon2 = "0.5"
rand = "0.8"
base64 = "0.21"
uuid = { version = "1.0", features = ["v4"] }

[build-dependencies]
tauri-build = "2.0"
```
