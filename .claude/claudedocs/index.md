# MagillaStream Documentation Index

Welcome to the MagillaStream documentation. This index provides quick access to all available documentation.

## Quick Start

- [CLAUDE.md](../../CLAUDE.md) - Main project context and overview

## Architecture & Design

| Document | Description |
|----------|-------------|
| [architecture.md](./architecture.md) | System architecture, layers, and data flow |
| [electron-ipc.md](./electron-ipc.md) | Electron IPC communication patterns |
| [models.md](./models.md) | Domain models and DTOs |
| [services.md](./services.md) | Service layer documentation |

## Implementation

| Document | Description |
|----------|-------------|
| [frontend.md](./frontend.md) | Frontend architecture and UI components |
| [ffmpeg-integration.md](./ffmpeg-integration.md) | FFmpeg process management |
| [security.md](./security.md) | Security features and best practices |
| [build-system.md](./build-system.md) | Build pipeline and scripts |

## Development

| Document | Description |
|----------|-------------|
| [development-workflow.md](./development-workflow.md) | Development setup and workflow |

## Custom Commands

Available slash commands for Claude Code:

| Command | Description |
|---------|-------------|
| `/build` | Run full build process |
| `/dev` | Start development environment |
| `/check-types` | Run TypeScript type checking |
| `/add-ipc-handler` | Add new IPC handler with full integration |
| `/add-model` | Create new domain model with DTO |
| `/review-security` | Review code for security issues |
| `/troubleshoot` | Diagnose common issues |
| `/analyze` | Deep analysis of file or component |

## Rules

Coding standards and patterns in `.claude/rules/`:

- **coding-standards.md** - TypeScript and general coding conventions
- **electron-patterns.md** - Electron-specific patterns and security
- **git-workflow.md** - Git branch and commit conventions

## Key Files Reference

### Main Process
- `src/electron/main.ts` - Application entry point
- `src/electron/ipcHandlers.ts` - IPC handler registration
- `src/electron/preload.ts` - Context bridge

### Models
- `src/models/Profile.ts` - User streaming profile
- `src/models/OutputGroup.ts` - Encoding configuration
- `src/models/StreamTarget.ts` - RTMP destination
- `src/models/Theme.ts` - UI theming

### Services
- `src/utils/profileManager.ts` - Profile persistence
- `src/utils/ffmpegHandler.ts` - FFmpeg process management
- `src/utils/encryption.ts` - AES-256-GCM encryption
- `src/utils/logger.ts` - Logging service
- `src/utils/encoderDetection.ts` - FFmpeg encoder discovery

### Frontend
- `src/frontend/index/index.html` - Main UI structure
- `src/frontend/index/index.js` - UI logic
- `src/frontend/index/index.css` - Styling

### Configuration
- `config/encoders.conf` - FFmpeg encoder whitelist
- `tsconfig.json` - TypeScript configuration
- `package.json` - NPM configuration

## Architecture Overview

```
┌────────────────────────────────────────────────┐
│                  Frontend                       │
│              (Vanilla JS + CSS)                 │
├────────────────────────────────────────────────┤
│                Preload Bridge                   │
│            (Context Isolation)                  │
├────────────────────────────────────────────────┤
│                IPC Handlers                     │
├────────────────────────────────────────────────┤
│                  Services                       │
│  ProfileManager │ FFmpegHandler │ Logger        │
├────────────────────────────────────────────────┤
│               Domain Models                     │
│  Profile │ OutputGroup │ StreamTarget           │
└────────────────────────────────────────────────┘
```

## Common Tasks

### Add a new feature
1. Read relevant architecture docs
2. Use `/add-model` if new data structure needed
3. Use `/add-ipc-handler` for new backend functionality
4. Update frontend to use new features
5. Run `/build` to verify

### Debug an issue
1. Use `/troubleshoot` with issue type
2. Check logs in `{userData}/logs/`
3. Use `/analyze` on relevant files

### Security review
1. Run `/review-security`
2. Address any findings
3. Update security.md if patterns change
