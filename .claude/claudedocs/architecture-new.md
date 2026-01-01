# MagillaStream New Architecture

## Overview

MagillaStream is undergoing a complete architectural transformation from Electron to Tauri, with a modern React frontend and Rust backend. This document describes the target architecture.

## Technology Stack

```
┌─────────────────────────────────────────────────────────────────────┐
│                         Technology Stack                             │
├─────────────────────────────────────────────────────────────────────┤
│  Frontend                                                            │
│  ├── React 18+          - UI framework                              │
│  ├── TypeScript 5.x     - Type safety                               │
│  ├── Tailwind CSS v4    - Styling                                   │
│  ├── Vite               - Build tool                                │
│  ├── Zustand/Jotai      - State management                          │
│  ├── Radix UI           - Accessible primitives                     │
│  └── Framer Motion      - Animations                                │
├─────────────────────────────────────────────────────────────────────┤
│  Backend                                                             │
│  ├── Tauri 2.x          - Desktop framework                         │
│  ├── Rust               - Backend language                          │
│  ├── tokio              - Async runtime                             │
│  ├── serde              - Serialization                             │
│  ├── aes-gcm            - Encryption                                │
│  └── argon2             - Key derivation                            │
├─────────────────────────────────────────────────────────────────────┤
│  System Integration                                                  │
│  ├── FFmpeg             - Stream encoding                           │
│  ├── std::process       - Process management                        │
│  └── Platform APIs      - File system, notifications                │
└─────────────────────────────────────────────────────────────────────┘
```

## Layer Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                        PRESENTATION LAYER                            │
│                                                                      │
│   ┌──────────────────────────────────────────────────────────────┐  │
│   │                    React Components                           │  │
│   │  ┌─────────────┐ ┌─────────────┐ ┌─────────────┐             │  │
│   │  │   Pages     │ │  Features   │ │     UI      │             │  │
│   │  │ - Dashboard │ │ - Profiles  │ │ - Button    │             │  │
│   │  │ - Settings  │ │ - Streams   │ │ - Card      │             │  │
│   │  │ - Profile   │ │ - Encoders  │ │ - Input     │             │  │
│   │  └─────────────┘ └─────────────┘ └─────────────┘             │  │
│   └──────────────────────────────────────────────────────────────┘  │
│                              ▼                                       │
│   ┌──────────────────────────────────────────────────────────────┐  │
│   │                    State Management                           │  │
│   │  ┌─────────────┐ ┌─────────────┐ ┌─────────────┐             │  │
│   │  │profileStore │ │ streamStore │ │ themeStore  │             │  │
│   │  └─────────────┘ └─────────────┘ └─────────────┘             │  │
│   └──────────────────────────────────────────────────────────────┘  │
│                              ▼                                       │
│   ┌──────────────────────────────────────────────────────────────┐  │
│   │                    Tauri API Bridge                           │  │
│   │              import { invoke } from '@tauri-apps/api'         │  │
│   └──────────────────────────────────────────────────────────────┘  │
├─────────────────────────────────────────────────────────────────────┤
│                         IPC BOUNDARY                                 │
│                    (Serialized JSON Messages)                        │
├─────────────────────────────────────────────────────────────────────┤
│                        APPLICATION LAYER                             │
│                                                                      │
│   ┌──────────────────────────────────────────────────────────────┐  │
│   │                    Tauri Commands                             │  │
│   │  ┌─────────────┐ ┌─────────────┐ ┌─────────────┐             │  │
│   │  │  profile::  │ │  stream::   │ │  system::   │             │  │
│   │  │ - load      │ │ - start     │ │ - encoders  │             │  │
│   │  │ - save      │ │ - stop      │ │ - paths     │             │  │
│   │  │ - delete    │ │ - status    │ │ - prefs     │             │  │
│   │  └─────────────┘ └─────────────┘ └─────────────┘             │  │
│   └──────────────────────────────────────────────────────────────┘  │
│                              ▼                                       │
│   ┌──────────────────────────────────────────────────────────────┐  │
│   │                    Service Layer                              │  │
│   │  ┌─────────────┐ ┌─────────────┐ ┌─────────────┐             │  │
│   │  │ProfileMgr   │ │FFmpegHandler│ │ Encryption  │             │  │
│   │  └─────────────┘ └─────────────┘ └─────────────┘             │  │
│   └──────────────────────────────────────────────────────────────┘  │
│                              ▼                                       │
│   ┌──────────────────────────────────────────────────────────────┐  │
│   │                    Domain Models                              │  │
│   │  ┌─────────────┐ ┌─────────────┐ ┌─────────────┐             │  │
│   │  │   Profile   │ │ OutputGroup │ │StreamTarget │             │  │
│   │  └─────────────┘ └─────────────┘ └─────────────┘             │  │
│   └──────────────────────────────────────────────────────────────┘  │
├─────────────────────────────────────────────────────────────────────┤
│                       INFRASTRUCTURE LAYER                           │
│   ┌─────────────────┐ ┌─────────────────┐ ┌─────────────────┐       │
│   │   File System   │ │  Child Process  │ │    Encryption   │       │
│   │ - profiles/     │ │  - FFmpeg       │ │  - AES-256-GCM  │       │
│   │ - logs/         │ │  - Process Map  │ │  - Argon2id     │       │
│   └─────────────────┘ └─────────────────┘ └─────────────────┘       │
└─────────────────────────────────────────────────────────────────────┘
```

## Directory Structure

```
magillastream/
├── src-frontend/                    # React application
│   ├── components/
│   │   ├── ui/                     # Base components
│   │   │   ├── Button.tsx
│   │   │   ├── Card.tsx
│   │   │   ├── Input.tsx
│   │   │   ├── Select.tsx
│   │   │   ├── Switch.tsx
│   │   │   ├── Modal.tsx
│   │   │   └── index.ts
│   │   ├── layout/                 # Layout components
│   │   │   ├── Sidebar.tsx
│   │   │   ├── Header.tsx
│   │   │   └── MainContent.tsx
│   │   ├── profile/                # Profile features
│   │   │   ├── ProfileList.tsx
│   │   │   ├── ProfileCard.tsx
│   │   │   └── ProfileForm.tsx
│   │   ├── stream/                 # Stream features
│   │   │   ├── StreamControls.tsx
│   │   │   ├── OutputGroupCard.tsx
│   │   │   ├── StreamTargetCard.tsx
│   │   │   └── StreamStatus.tsx
│   │   └── settings/               # Settings features
│   │       ├── EncoderSettings.tsx
│   │       └── ThemeToggle.tsx
│   ├── hooks/                      # Custom hooks
│   │   ├── useProfile.ts
│   │   ├── useStream.ts
│   │   ├── useTheme.ts
│   │   └── useTauri.ts
│   ├── stores/                     # Zustand stores
│   │   ├── profileStore.ts
│   │   ├── streamStore.ts
│   │   └── themeStore.ts
│   ├── lib/                        # Utilities
│   │   ├── tauri.ts               # Tauri invoke wrappers
│   │   ├── utils.ts               # Helper functions
│   │   └── cn.ts                  # Class name utility
│   ├── types/                      # TypeScript types
│   │   ├── profile.ts
│   │   ├── stream.ts
│   │   └── api.ts
│   ├── styles/
│   │   ├── globals.css            # Global styles
│   │   └── tokens.css             # Design tokens
│   ├── App.tsx                     # Root component
│   ├── main.tsx                    # Entry point
│   └── vite-env.d.ts
│
├── src-tauri/                       # Rust backend
│   ├── src/
│   │   ├── main.rs                 # Tauri entry
│   │   ├── lib.rs                  # Library root
│   │   ├── commands/               # Tauri commands
│   │   │   ├── mod.rs
│   │   │   ├── profile.rs
│   │   │   ├── stream.rs
│   │   │   └── system.rs
│   │   ├── services/               # Business logic
│   │   │   ├── mod.rs
│   │   │   ├── profile_manager.rs
│   │   │   ├── ffmpeg_handler.rs
│   │   │   ├── encryption.rs
│   │   │   └── logger.rs
│   │   ├── models/                 # Data structures
│   │   │   ├── mod.rs
│   │   │   ├── profile.rs
│   │   │   ├── output_group.rs
│   │   │   └── stream_target.rs
│   │   └── utils/
│   │       ├── mod.rs
│   │       └── paths.rs
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   └── capabilities/               # Tauri permissions
│       └── default.json
│
├── .claude/                         # Claude Code config
├── public/                          # Static assets
├── index.html                       # HTML entry
├── package.json
├── tsconfig.json
├── tailwind.config.js
├── vite.config.ts
└── README.md
```

## Data Flow

### Profile Load Flow

```
┌──────────────┐     ┌──────────────┐     ┌──────────────┐     ┌──────────────┐
│   UI Click   │────▶│ profileStore │────▶│ invoke()     │────▶│ Tauri IPC    │
│   "Load"     │     │ setLoading() │     │ load_profile │     │              │
└──────────────┘     └──────────────┘     └──────────────┘     └──────────────┘
                                                                       │
                                                                       ▼
┌──────────────┐     ┌──────────────┐     ┌──────────────┐     ┌──────────────┐
│   UI Update  │◀────│ profileStore │◀────│ Tauri IPC    │◀────│ Rust Command │
│   Render     │     │ setProfile() │     │ Response     │     │ load_profile │
└──────────────┘     └──────────────┘     └──────────────┘     └──────────────┘
                                                                       │
                                                                       ▼
                                                               ┌──────────────┐
                                                               │ProfileManager│
                                                               │ - read file  │
                                                               │ - decrypt    │
                                                               │ - parse JSON │
                                                               └──────────────┘
```

### Stream Start Flow

```
Frontend                    Tauri                      Rust Services
    │                          │                            │
    │ invoke('start_stream')   │                            │
    ├─────────────────────────▶│                            │
    │                          │ stream::start()            │
    │                          ├───────────────────────────▶│
    │                          │                            │
    │                          │     FFmpegHandler          │
    │                          │     ├─ build_command()     │
    │                          │     ├─ spawn_process()     │
    │                          │     └─ store_handle()      │
    │                          │                            │
    │                          │◀───────────────────────────┤
    │                          │   ProcessInfo              │
    │◀─────────────────────────┤                            │
    │   { pid, status }        │                            │
    │                          │                            │
    │                          │   emit('stream_status')    │
    │◀═════════════════════════╪════════════════════════════│
    │   Status updates         │   (ongoing events)         │
```

## State Management

### Zustand Store Structure

```typescript
// profileStore.ts
interface ProfileState {
  // State
  profiles: string[];
  currentProfile: Profile | null;
  isLoading: boolean;
  error: string | null;

  // Actions
  loadProfiles: () => Promise<void>;
  loadProfile: (name: string, password?: string) => Promise<void>;
  saveProfile: (password?: string) => Promise<void>;
  deleteProfile: (name: string) => Promise<void>;
  updateProfile: (updates: Partial<Profile>) => void;
}

// streamStore.ts
interface StreamState {
  // State
  streams: Map<string, StreamInfo>;
  isStreaming: boolean;

  // Actions
  startStream: (groupId: string) => Promise<void>;
  stopStream: (groupId: string) => Promise<void>;
  stopAllStreams: () => Promise<void>;
}

// themeStore.ts
interface ThemeState {
  // State
  theme: 'light' | 'dark' | 'system';

  // Actions
  setTheme: (theme: 'light' | 'dark' | 'system') => void;
  toggleTheme: () => void;
}
```

## Rust Backend Structure

### Command Pattern

```rust
// commands/profile.rs
use crate::services::ProfileManager;
use crate::models::Profile;

#[tauri::command]
pub async fn load_profile(
    name: String,
    password: Option<String>,
    state: tauri::State<'_, ProfileManager>,
) -> Result<Profile, String> {
    state.load(&name, password.as_deref())
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn save_profile(
    profile: Profile,
    password: Option<String>,
    state: tauri::State<'_, ProfileManager>,
) -> Result<(), String> {
    state.save(&profile, password.as_deref())
        .await
        .map_err(|e| e.to_string())
}
```

### Service Pattern

```rust
// services/profile_manager.rs
use std::path::PathBuf;
use tokio::fs;
use crate::models::Profile;
use crate::services::Encryption;

pub struct ProfileManager {
    profiles_dir: PathBuf,
    encryption: Encryption,
}

impl ProfileManager {
    pub fn new(app_data_dir: PathBuf) -> Self {
        Self {
            profiles_dir: app_data_dir.join("profiles"),
            encryption: Encryption::new(),
        }
    }

    pub async fn load(
        &self,
        name: &str,
        password: Option<&str>,
    ) -> Result<Profile, ProfileError> {
        let path = self.profiles_dir.join(format!("{}.json", name));
        let content = fs::read_to_string(&path).await?;

        let json = match password {
            Some(pwd) => self.encryption.decrypt(&content, pwd)?,
            None => content,
        };

        let profile: Profile = serde_json::from_str(&json)?;
        Ok(profile)
    }

    pub async fn save(
        &self,
        profile: &Profile,
        password: Option<&str>,
    ) -> Result<(), ProfileError> {
        let json = serde_json::to_string_pretty(profile)?;

        let content = match password {
            Some(pwd) => self.encryption.encrypt(&json, pwd)?,
            None => json,
        };

        let path = self.profiles_dir.join(format!("{}.json", profile.name));
        fs::write(&path, content).await?;
        Ok(())
    }
}
```

## Security Model

### Tauri Capabilities

```json
// capabilities/default.json
{
  "identifier": "default",
  "description": "Default capability for MagillaStream",
  "windows": ["main"],
  "permissions": [
    "core:default",
    "fs:default",
    "shell:allow-spawn",
    "process:default",
    {
      "identifier": "fs:allow-read",
      "allow": [
        { "path": "$APPDATA/profiles/*" },
        { "path": "$APPDATA/logs/*" }
      ]
    },
    {
      "identifier": "fs:allow-write",
      "allow": [
        { "path": "$APPDATA/profiles/*" },
        { "path": "$APPDATA/logs/*" }
      ]
    }
  ]
}
```

### Encryption Flow

```rust
// AES-256-GCM with Argon2id key derivation
pub struct Encryption;

impl Encryption {
    pub fn encrypt(&self, plaintext: &str, password: &str) -> Result<String, EncryptionError> {
        // 1. Generate random salt (32 bytes)
        let salt = rand::random::<[u8; 32]>();

        // 2. Derive key using Argon2id
        let key = argon2::hash_encoded(
            password.as_bytes(),
            &salt,
            &argon2::Config::default(),
        )?;

        // 3. Generate random nonce (12 bytes)
        let nonce = rand::random::<[u8; 12]>();

        // 4. Encrypt with AES-256-GCM
        let cipher = Aes256Gcm::new_from_slice(&key)?;
        let ciphertext = cipher.encrypt(&nonce.into(), plaintext.as_bytes())?;

        // 5. Combine salt + nonce + ciphertext and encode
        let combined = [&salt[..], &nonce[..], &ciphertext].concat();
        Ok(base64::encode(combined))
    }
}
```

## Build Configuration

### Vite Config

```typescript
// vite.config.ts
import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import path from 'path';

export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: {
      '@': path.resolve(__dirname, './src-frontend'),
    },
  },
  build: {
    outDir: 'dist',
    emptyOutDir: true,
  },
  server: {
    port: 5173,
    strictPort: true,
  },
});
```

### Tauri Config

```json
// src-tauri/tauri.conf.json
{
  "productName": "MagillaStream",
  "version": "0.1.0",
  "identifier": "com.magillastream.app",
  "build": {
    "beforeDevCommand": "npm run dev",
    "beforeBuildCommand": "npm run build",
    "devUrl": "http://localhost:5173",
    "frontendDist": "../dist"
  },
  "app": {
    "windows": [
      {
        "title": "MagillaStream",
        "width": 1200,
        "height": 800,
        "minWidth": 800,
        "minHeight": 600
      }
    ],
    "security": {
      "csp": "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'"
    }
  },
  "bundle": {
    "active": true,
    "targets": "all",
    "icon": ["icons/icon.png"]
  }
}
```
