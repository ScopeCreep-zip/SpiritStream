# Backend Documentation (Rust)

[Documentation](../README.md) > Backend

---

## Overview

This section documents SpiritStream's Rust backend, including the service layer, domain models, Tauri commands, and encryption implementation.

## Documents

| Document | Description | Audience |
|----------|-------------|----------|
| [01. Rust Overview](./01-rust-overview.md) | Crate structure and dependencies | Intermediate+ |
| [02. Services Layer](./02-services-layer.md) | ProfileManager, FFmpegHandler, Encryption | Advanced |
| [03. Models Reference](./03-models-reference.md) | Profile, OutputGroup, StreamTarget | All levels |
| [04. Tauri Commands](./04-tauri-commands.md) | Complete command API reference | All levels |
| [05. Encryption Implementation](./05-encryption-implementation.md) | AES-256-GCM + Argon2id details | Expert |
| [06. Platform Registry](./06-platform-registry.md) | PlatformRegistry service, JSON schema | Intermediate+ |

## Key Services

| Service | Purpose | Key Files |
|---------|---------|-----------|
| ProfileManager | Profile CRUD with encryption | `services/profile_manager.rs` |
| FFmpegHandler | Stream process management | `services/ffmpeg_handler.rs` |
| Encryption | AES-256-GCM operations | `services/encryption.rs` |
| SettingsManager | App settings persistence | `services/settings_manager.rs` |
| ThemeManager | Theme discovery and tokens | `services/theme_manager.rs` |
| PlatformRegistry | 80+ platform configurations | `services/platform_registry.rs` |

## Source Structure

```
src-tauri/src/
├── main.rs                 # Entry point
├── lib.rs                  # Tauri app initialization
├── commands/               # Tauri command handlers
│   ├── profile.rs          # Profile commands
│   ├── stream.rs           # Stream commands
│   ├── settings.rs         # Settings commands
│   └── ...
├── services/               # Business logic
│   ├── profile_manager.rs
│   ├── ffmpeg_handler.rs
│   └── ...
└── models/                 # Data structures
    ├── profile.rs
    ├── output_group.rs
    └── ...
```

---

*Section: 02-backend*
