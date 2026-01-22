# SpiritStream Documentation

[Back to Project](../README.md)

---

SpiritStream is a multi-destination streaming application that lets you stream to YouTube, Twitch, Kick, Facebook, and custom RTMP servers simultaneously. Built with Tauri 2.x, Rust, and React.

---

## Quick Navigation

| I Need To... | Start Here |
|--------------|------------|
| Install and run SpiritStream | [Getting Started](./06-tutorials/01-getting-started.md) |
| Understand the architecture | [System Overview](./01-architecture/01-system-overview.md) |
| Learn about backend services | [Services Layer](./02-backend/02-services-layer.md) |
| Understand frontend state | [State Management](./03-frontend/02-state-management.md) |
| Configure streaming | [FFmpeg Integration](./04-streaming/01-ffmpeg-integration.md) |
| Reference the API | [Commands API](./05-api-reference/01-commands-api.md) |
| Deploy with Docker | [Building](./07-deployment/01-building.md#docker-build) |
| Look up a term | [Glossary](./GLOSSARY.md) |

---

## Project Statistics

| Metric | Value |
|--------|-------|
| **Framework** | Tauri 2.x + Axum |
| **Backend** | Rust (10,000+ lines) |
| **Frontend** | React 19 + TypeScript (8,700+ lines) |
| **Tauri Commands** | 30+ |
| **UI Components** | 40+ |
| **Supported Platforms** | Windows, macOS, Linux |
| **Deployment Modes** | Desktop, Docker, Cloud (future) |
| **Supported Languages** | 5 (en, es, fr, de, ja) |

---

## Reading Paths

### Beginner (2-4 hours)
New to desktop apps or streaming? Start here:

1. [Glossary](./GLOSSARY.md) — Learn key terminology
2. [Getting Started](./06-tutorials/01-getting-started.md) — Install and first run
3. [First Stream](./06-tutorials/02-first-stream.md) — Set up your first stream
4. [System Overview](./01-architecture/01-system-overview.md) — High-level concepts

### Intermediate (4-8 hours)
Comfortable with React and TypeScript? Go deeper:

5. [React Architecture](./03-frontend/01-react-architecture.md)
6. [State Management](./03-frontend/02-state-management.md)
7. [FFmpeg Integration](./04-streaming/01-ffmpeg-integration.md)
8. [Multi-Platform Tutorial](./06-tutorials/03-multi-platform.md)

### Advanced (8-16 hours)
Ready for implementation details and security?

9. [Security Architecture](./01-architecture/04-security-architecture.md)
10. [Services Layer](./02-backend/02-services-layer.md)
11. [Encryption Implementation](./02-backend/05-encryption-implementation.md)
12. [Commands API](./05-api-reference/01-commands-api.md)

---

## Table of Contents

### Glossary
- [Technical Glossary](./GLOSSARY.md) — 50+ terms and definitions

### Architecture
- [Section Overview](./01-architecture/README.md)
- [System Overview](./01-architecture/01-system-overview.md) — High-level architecture with diagrams
- [Component Architecture](./01-architecture/02-component-architecture.md) — Detailed component breakdown
- [Data Flow](./01-architecture/03-data-flow.md) — Data flow and sequence diagrams
- [Security Architecture](./01-architecture/04-security-architecture.md) — Security model, encryption, Tauri permissions

### Backend (Rust)
- [Section Overview](./02-backend/README.md)
- [Rust Overview](./02-backend/01-rust-overview.md) — Crate structure, dependencies
- [Services Layer](./02-backend/02-services-layer.md) — ProfileManager, FFmpegHandler, Encryption
- [Models Reference](./02-backend/03-models-reference.md) — Profile, OutputGroup, StreamTarget
- [Tauri Commands](./02-backend/04-tauri-commands.md) — All 30+ command signatures
- [Encryption Implementation](./02-backend/05-encryption-implementation.md) — AES-256-GCM + Argon2id

### Frontend (React)
- [Section Overview](./03-frontend/README.md)
- [React Architecture](./03-frontend/01-react-architecture.md) — Component hierarchy and patterns
- [State Management](./03-frontend/02-state-management.md) — Zustand stores (profile, stream, theme)
- [Component Library](./03-frontend/03-component-library.md) — UI components with props and usage
- [Tauri Integration](./03-frontend/04-tauri-integration.md) — IPC patterns and api wrapper
- [Theming and i18n](./03-frontend/05-theming-i18n.md) — Theme system and internationalization

### Streaming
- [Section Overview](./04-streaming/README.md)
- [FFmpeg Integration](./04-streaming/01-ffmpeg-integration.md) — Process management, relay architecture
- [RTMP Fundamentals](./04-streaming/02-rtmp-fundamentals.md) — Protocol basics for streaming
- [Multi-Destination](./04-streaming/03-multi-destination.md) — Output groups and target management
- [Encoding Reference](./04-streaming/04-encoding-reference.md) — Codecs, presets, hardware acceleration

### API Reference
- [Section Overview](./05-api-reference/README.md)
- [Commands API](./05-api-reference/01-commands-api.md) — Complete Tauri command reference
- [Events API](./05-api-reference/02-events-api.md) — Event system documentation
- [Types Reference](./05-api-reference/03-types-reference.md) — TypeScript and Rust type definitions
- [Error Handling](./05-api-reference/04-error-handling.md) — Error codes and recovery patterns

### Tutorials
- [Section Overview](./06-tutorials/README.md)
- [Getting Started](./06-tutorials/01-getting-started.md) — Installation on all platforms
- [First Stream](./06-tutorials/02-first-stream.md) — Basic streaming setup
- [Multi-Platform](./06-tutorials/03-multi-platform.md) — Streaming to multiple services
- [Custom Encoding](./06-tutorials/04-custom-encoding.md) — Advanced encoding configuration
- [Contributing](./06-tutorials/05-contributing.md) — Development setup and code style

### Deployment
- [Section Overview](./07-deployment/README.md)
- [Building](./07-deployment/01-building.md) — Build process documentation
- [Platform Guides](./07-deployment/02-platform-guides.md) — Windows, macOS, Linux specifics
- [Distribution Strategy](./07-deployment/03-distribution-strategy.md) — Desktop, Docker, Cloud
- [Release Process](./07-deployment/04-release-process.md) — Versioning and distribution

---

## Technology Stack

```
┌─────────────────────────────────────────────────────────────────────┐
│                     CLIENT LAYER                                     │
│  ┌──────────────────────────┐  ┌──────────────────────────┐        │
│  │     Tauri Desktop        │  │      Web Browser         │        │
│  │    (Embedded Webview)    │  │    (Remote Access)       │        │
│  └────────────┬─────────────┘  └────────────┬─────────────┘        │
│               │                             │                       │
│               └──────────────┬──────────────┘                       │
│                              │                                      │
├──────────────────────────────┼──────────────────────────────────────┤
│                     API LAYER│                                      │
│              HTTP/WebSocket API (Axum)                              │
│  ┌──────────────┐ ┌──────────────┐ ┌──────────────┐                │
│  │ POST /api/*  │ │   WS /ws     │ │  Static UI   │                │
│  └──────────────┘ └──────────────┘ └──────────────┘                │
├─────────────────────────────────────────────────────────────────────┤
│                     APPLICATION LAYER                                │
│                     Rust Services                                    │
│  ┌──────────────┐ ┌──────────────┐ ┌──────────────┐                 │
│  │ProfileManager│ │FFmpegHandler │ │  Encryption  │                 │
│  └──────────────┘ └──────────────┘ └──────────────┘                 │
├─────────────────────────────────────────────────────────────────────┤
│                     INFRASTRUCTURE                                   │
│  ┌──────────────┐ ┌──────────────┐ ┌──────────────┐                 │
│  │   FFmpeg     │ │  File System │ │   Crypto     │                 │
│  │  Processes   │ │   (Profiles) │ │ (AES-256)    │                 │
│  └──────────────┘ └──────────────┘ └──────────────┘                 │
└─────────────────────────────────────────────────────────────────────┘
```

---

## Deployment Modes

| Mode | Use Case | Setup |
|------|----------|-------|
| **Desktop** | Local streaming with GPU acceleration | Download installer |
| **Docker** | Self-hosted on your server | `docker pull` + compose |
| **Cloud** | Managed service (future) | Sign up |

See [Distribution Strategy](./07-deployment/03-distribution-strategy.md) for details.

---

## Diagrams

All Mermaid diagrams use a dark theme:

- **Background:** `#0F0A14` (deep purple-black)
- **Primary:** `#7C3AED` / `#A78BFA` (violet)
- **Text:** `#F4F2F7` (off-white)

---

## Code References

Source code links use the format: [`filename.rs:line`](../path/to/file.rs#L123)

---

## Contributing

See [Contributing Guide](./06-tutorials/05-contributing.md) for documentation style and review process.

