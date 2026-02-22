# Architecture

## System Overview

```text
┌─────────────────────────────────────────────────────────────────────┐
│                         CLIENT LAYER                                │
│  ┌─────────────────────────┐    ┌─────────────────────────┐        │
│  │  Tauri Desktop          │    │  Web Browser            │        │
│  │  (Embedded Webview)     │    │  (Remote Access)        │        │
│  └───────────┬─────────────┘    └───────────┬─────────────┘        │
│              │         HTTP/WS API          │                      │
│              └──────────────┬───────────────┘                      │
├─────────────────────────────┼──────────────────────────────────────┤
│                             ▼                                      │
│                    HOST SERVER (Rust + Axum)                       │
│       POST /api/invoke/* │ REST /api/* │ WS /ws │ Static UI       │
├────────────────────────────────────────────────────────────────────┤
│                       ROUTE LAYER                                  │
│   invoke │ capture │ preview │ webrtc │ audio │ devices │ health  │
│   recording │ permissions │ files │ websocket                      │
├────────────────────────────────────────────────────────────────────┤
│                       SERVICE LAYER                                │
│   Compositor │ AudioLevels │ DeviceDiscovery │ ScreenCapture      │
│   CameraCapture │ H264Capture │ JpegPreview │ SourceLifecycle    │
│   Go2rtcManager │ FFmpegHandler │ ProfileManager │ Encryption    │
│   RecordingService │ ReplayBuffer │ SettingsManager │ Permissions │
├────────────────────────────────────────────────────────────────────┤
│                       MEDIA LAYER                                  │
│   FFmpeg (encoding) │ go2rtc (WebRTC/RTMP) │ scap (screen cap)   │
│   turbojpeg (JPEG encode) │ nokhwa (camera)                       │
├────────────────────────────────────────────────────────────────────┤
│                       STORAGE LAYER                                │
│              Profiles │ Settings │ Logs │ Themes                   │
└────────────────────────────────────────────────────────────────────┘
```

## Deployment Modes

- **Desktop**: Tauri launcher spawns host server, UI in embedded webview
- **Docker**: Host server in container, UI served or separate
- **Cloud**: Managed host servers with multi-tenant storage (future)

## Directory Structure

```text
spiritstream/
├── apps/
│   ├── web/                         # React frontend (standalone)
│   │   └── src/
│   │       ├── components/          # React components
│   │       │   ├── ui/              # Base UI (Button, Card, etc.)
│   │       │   ├── layout/          # Layout components
│   │       │   ├── stream/          # Streaming controls
│   │       │   ├── modals/          # Modal dialogs
│   │       │   ├── sources/         # Source-specific components
│   │       │   ├── dashboard/       # Dashboard views
│   │       │   ├── encoder/         # Encoder configuration
│   │       │   ├── settings/        # Settings panels
│   │       │   ├── navigation/      # Nav components
│   │       │   └── feedback/        # Feedback/toast UI
│   │       ├── hooks/               # Custom React hooks
│   │       ├── stores/              # Zustand state (17 stores)
│   │       ├── lib/
│   │       │   ├── backend/         # Backend abstraction (Tauri/HTTP)
│   │       │   └── audio/           # Audio meter workers
│   │       ├── types/               # TypeScript types
│   │       ├── utils/               # Utility functions
│   │       ├── styles/              # Global styles + Tailwind
│   │       ├── locales/             # i18n translations (5 langs)
│   │       └── views/               # Page views
│   │
│   └── desktop/                     # Tauri wrapper (minimal)
│       └── src-tauri/
│           ├── Cargo.toml           # Minimal deps (launcher only)
│           ├── tauri.conf.json      # Sidecar config + security
│           └── src/main.rs          # Spawns server sidecar
│
├── server/                          # Standalone Rust backend
│   └── src/
│       ├── main.rs                  # Axum HTTP server entry
│       ├── lib.rs                   # Re-exports
│       ├── routes/                  # HTTP route handlers (12 modules)
│       ├── commands/                # Business logic (12 modules)
│       │   └── capture/             # Capture subcommands
│       ├── models/                  # Domain models (10 modules)
│       └── services/                # Service layer (35+ modules)
│           ├── device_discovery/    # Device enumeration (module dir)
│           └── ffmpeg_handler/      # FFmpeg management (module dir)
│
├── config/                          # Configuration files
├── themes/                          # Theme definitions
├── scripts/                         # Build/setup scripts
├── docker/                          # Docker configuration
├── docs/                            # Project documentation
└── .claude/                         # Claude Code config
    ├── claudedocs/                  # Generated documentation
    ├── commands/                    # Custom slash commands
    └── rules/                       # Coding standards
```

## Design System

Purple & Pink theme with light/dark mode support (WCAG 2.2 AA compliant):
- **Primary**: Violet (#7C3AED / #A78BFA)
- **Secondary**: Fuchsia (#C026D3 / #E879F9)
- **Accent**: Pink (#DB2777 / #F472B6)
