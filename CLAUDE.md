# MagillaStream - Claude Code Context

> This file provides persistent context for Claude Code sessions. It is automatically loaded at session start.

## Project Overview

**MagillaStream** is a desktop streaming application built with Electron and TypeScript. It manages RTMP stream configurations, handles FFmpeg-based stream processing, and provides a graphical interface for multi-output streaming with profile management.

**Repository**: https://github.com/billboyles/magillastream
**Current Branch**: tauri-shift (migration in progress)

## Technology Stack

| Category | Technology |
|----------|------------|
| Framework | Electron ^34.3.0 |
| Language | TypeScript ^5.8.2 |
| Runtime | Node.js |
| Module System | CommonJS with ESNext target |
| Build | electron-builder (24.6.0) |
| Packaging | NSIS (Windows) |

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│                     Frontend (Vanilla JS)                    │
│                    src/frontend/index/                       │
├─────────────────────────────────────────────────────────────┤
│                 Preload (Context Bridge)                     │
│                  src/electron/preload.ts                     │
├─────────────────────────────────────────────────────────────┤
│                     IPC Handlers                             │
│                 src/electron/ipcHandlers.ts                  │
├─────────────────────────────────────────────────────────────┤
│                    Main Process                              │
│                  src/electron/main.ts                        │
├─────────────────────────────────────────────────────────────┤
│                   Services Layer                             │
│    ProfileManager │ FFmpegHandler │ Logger │ Encryption      │
│                     src/utils/                               │
├─────────────────────────────────────────────────────────────┤
│                    Domain Models                             │
│       Profile │ OutputGroup │ StreamTarget │ Theme           │
│                    src/models/                               │
└─────────────────────────────────────────────────────────────┘
```

## Directory Structure

```
magillastream/
├── src/
│   ├── electron/          # Electron main process
│   │   ├── main.ts        # Entry point, window creation
│   │   ├── ipcHandlers.ts # IPC event handlers
│   │   └── preload.ts     # Context bridge for secure IPC
│   ├── models/            # Core domain models
│   │   ├── Profile.ts     # User streaming profile
│   │   ├── OutputGroup.ts # Video/audio encoding group
│   │   ├── StreamTarget.ts# RTMP destination target
│   │   └── Theme.ts       # UI theming
│   ├── utils/             # Utility services
│   │   ├── profileManager.ts    # Profile persistence
│   │   ├── ffmpegHandler.ts     # FFmpeg process management
│   │   ├── encoderDetection.ts  # FFmpeg encoder discovery
│   │   ├── encryption.ts        # AES-256-GCM encryption
│   │   ├── logger.ts            # File-based logging
│   │   ├── rendererLogger.ts    # Frontend logging bridge
│   │   └── dtoUtils.ts          # DTO conversion utilities
│   ├── frontend/          # UI layer (vanilla JS)
│   │   └── index/
│   │       ├── index.html
│   │       ├── index.js
│   │       └── index.css
│   ├── shared/            # Shared interfaces
│   │   └── interfaces.ts  # TypeScript DTOs for IPC
│   └── types/             # Type definitions
│       └── preload.d.ts
├── config/
│   └── encoders.conf      # FFmpeg encoder whitelist
├── scripts/               # Build scripts
├── docs/                  # Documentation
├── .claude/               # Claude Code configuration
│   └── claudedocs/        # Extended documentation
└── package.json
```

## Core Domain Models

### Profile
Top-level configuration entity containing:
- ID, name, incoming RTMP URL
- Array of OutputGroups
- Optional Theme
- Methods: `toDTO()`, `export()`, getters/setters

### OutputGroup
Encoding profile for one stream target:
- Video encoder (H.264/H.265/NVENC/etc)
- Resolution, bitrate, FPS
- Audio codec, audio bitrate
- PTS (Presentation Time Stamp) generation flag
- Array of StreamTargets

### StreamTarget
RTMP destination:
- URL, stream key, RTMP port (default 1935)
- Computed `normalizedPath` property for complete RTMP URL
- Methods: `toDTO()`, `export()`

## IPC Commands

### Profile Management
- `profile:getAllProfileNames` - List profiles
- `profile:load` - Load profile (with optional password)
- `profile:save` - Save profile (with optional encryption)
- `profile:delete` - Remove profile
- `profile:getLastUsed` - Retrieve last used profile
- `profile:saveLastUsed` - Store last used profile name

### FFmpeg Operations
- `ffmpeg:test` - Verify FFmpeg installation
- `ffmpeg:start` - Start encoding with output groups
- `ffmpeg:stop` - Stop encoding for a group
- `ffmpeg:stopAll` - Stop all encoding
- `ffmpeg:getAudioEncoders` - List available audio codecs
- `ffmpeg:getVideoEncoders` - List available video codecs

## Build Commands

```bash
npm run clean      # Delete dist/ and release/
npm run compile    # TypeScript compilation
npm run pack       # electron-builder packaging
npm run build      # Full build: clean → compile → pack → copy resources
npm run dev        # Development: compile → copy → electron
npm run start      # Direct Electron launch
```

## Key Design Patterns

1. **Singleton Pattern**: ProfileManager, FFmpegHandler, Logger, Encryption
2. **DTO Pattern**: Data transfer objects for IPC serialization
3. **Service Layer**: Clear separation between models and services
4. **Factory Pattern**: dtoUtils provides reconstruction methods
5. **Process Management**: Maps for tracking running FFmpeg instances

## Security Features

- Context isolation enabled in Electron
- Sandbox mode enabled
- Node integration disabled
- AES-256-GCM for profile encryption
- PBKDF2 key derivation with salt (100,000 iterations)
- User data isolated in AppData/MagillaStream

## Coding Standards

### TypeScript
- Strict mode enabled
- Use explicit types for function parameters and returns
- Prefer interfaces over type aliases for object shapes
- Use `readonly` for immutable properties

### Naming Conventions
- PascalCase for classes, interfaces, types
- camelCase for variables, functions, methods
- UPPER_SNAKE_CASE for constants
- Prefix private class members with underscore

### File Organization
- One class per file (models)
- Related utilities grouped in single files
- Index files for barrel exports where appropriate

### Error Handling
- Always use try/catch in async functions
- Log errors with appropriate context
- Return meaningful error messages to frontend

## Extended Documentation

For detailed documentation, see `.claude/claudedocs/`:

@.claude/claudedocs/architecture.md
@.claude/claudedocs/electron-ipc.md
@.claude/claudedocs/models.md
@.claude/claudedocs/services.md
@.claude/claudedocs/frontend.md
@.claude/claudedocs/security.md
@.claude/claudedocs/build-system.md
@.claude/claudedocs/ffmpeg-integration.md
@.claude/claudedocs/development-workflow.md
