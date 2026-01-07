# Adding New Streaming Platforms

## Overview

SpiritStream uses a modular platform registry system that makes it easy to add new streaming services. This guide shows you how to add support for a new platform.

## Step 1: Add Platform Enum

Edit `src-tauri/src/models/stream_target.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum Platform {
    Youtube,
    Twitch,
    Kick,
    Facebook,
    NewPlatform,  // Add your new platform here
    #[default]
    Custom,
}

impl std::fmt::Display for Platform {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Platform::Youtube => write!(f, "youtube"),
            Platform::Twitch => write!(f, "twitch"),
            Platform::Kick => write!(f, "kick"),
            Platform::Facebook => write!(f, "facebook"),
            Platform::NewPlatform => write!(f, "newplatform"),  // Add display name
            Platform::Custom => write!(f, "custom"),
        }
    }
}
```

## Step 2: Configure Platform Registry

Edit `src-tauri/src/services/platform_registry.rs` in the `PlatformRegistry::new()` method:

```rust
// New Platform
configs.insert(Platform::NewPlatform, PlatformConfig {
    name: "New Platform",                          // Display name
    default_server: "rtmp://stream.example.com/live",  // Default RTMP URL
    default_app_path: Some("live"),                // App path (e.g., "app", "live", "rtmp")
    stream_key_position: 2,                        // Position of stream key in URL
});
```

### Understanding stream_key_position

This tells the system which segment of the URL path contains the stream key:

- `stream_key_position: 1` → `rtmp://host.com/STREAM_KEY`
- `stream_key_position: 2` → `rtmp://host.com/app/STREAM_KEY` (most common)
- `stream_key_position: 3` → `rtmp://host.com/app/channel/STREAM_KEY`

**Examples:**
- YouTube: `rtmp://a.rtmp.youtube.com/live2/KEY` → position 2
- Twitch: `rtmp://ingest.twitch.tv/app/KEY` → position 2
- Facebook: `rtmps://live-api-s.facebook.com:443/rtmp/KEY` → position 2

### Understanding default_app_path

This is used for URL normalization. If a user provides a URL without the app path, it will be automatically added:

```rust
default_app_path: Some("app")
```

**Example:**
- User input: `rtmps://stream.example.com`
- Normalized: `rtmps://stream.example.com/app`

If your platform doesn't need normalization (URL is always complete), use `None`:

```rust
default_app_path: None
```

## Step 3: Add Frontend Configuration

Edit `src-frontend/types/profile.ts`:

```typescript
export type Platform = 'youtube' | 'twitch' | 'kick' | 'facebook' | 'newplatform' | 'custom';

export const PLATFORM_CONFIGS: Record<Platform, PlatformInfo> = {
  // ... existing platforms ...
  newplatform: {
    name: 'New Platform',
    abbreviation: 'NP',
    color: '#FF6600',           // Brand color
    textColor: '#FFFFFF',        // Text color (for readability on brand color)
    defaultServer: 'rtmp://stream.example.com/live',
  },
  // ... custom ...
};
```

### Color Guidelines

- **color**: The platform's brand color (used for icons and badges)
- **textColor**: Must have WCAG AA contrast ratio with `color` (usually white or black)

**Contrast Check:**
- Light backgrounds need dark text
- Dark backgrounds need light text
- Use online tools to verify WCAG AA compliance (4.5:1 ratio minimum)

## Step 4: Add Tests (Optional but Recommended)

Edit `src-tauri/src/services/platform_registry.rs` in the `#[cfg(test)]` section:

```rust
#[test]
fn test_newplatform_redaction() {
    let registry = PlatformRegistry::new();
    let url = "rtmp://stream.example.com/live/my_secret_key";
    let redacted = registry.redact_url(&Platform::NewPlatform, url);
    assert_eq!(redacted, "rtmp://stream.example.com/live/***");
}

#[test]
fn test_newplatform_url_normalization() {
    let registry = PlatformRegistry::new();
    let url = "rtmp://stream.example.com";
    let normalized = registry.normalize_url(&Platform::NewPlatform, url);
    assert_eq!(normalized, "rtmp://stream.example.com/live");
}
```

## Example: Adding Rumble

Let's add support for Rumble as a complete example.

### 1. Enum (stream_target.rs)

```rust
pub enum Platform {
    Youtube,
    Twitch,
    Kick,
    Facebook,
    Rumble,  // New
    #[default]
    Custom,
}

impl std::fmt::Display for Platform {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            // ... existing ...
            Platform::Rumble => write!(f, "rumble"),
            Platform::Custom => write!(f, "custom"),
        }
    }
}
```

### 2. Registry (platform_registry.rs)

```rust
// Rumble
configs.insert(Platform::Rumble, PlatformConfig {
    name: "Rumble",
    default_server: "rtmp://rmbl.to/app",
    default_app_path: Some("app"),
    stream_key_position: 2,  // /app/KEY
});
```

### 3. Frontend (profile.ts)

```typescript
export type Platform = 'youtube' | 'twitch' | 'kick' | 'facebook' | 'rumble' | 'custom';

rumble: {
  name: 'Rumble',
  abbreviation: 'R',
  color: '#85C742',        // Rumble green
  textColor: '#000000',    // Black text (good contrast on green)
  defaultServer: 'rtmp://rmbl.to/app',
},
```

### 4. Tests (platform_registry.rs)

```rust
#[test]
fn test_rumble_redaction() {
    let registry = PlatformRegistry::new();
    let url = "rtmp://rmbl.to/app/stream_key_xyz";
    let redacted = registry.redact_url(&Platform::Rumble, url);
    assert_eq!(redacted, "rtmp://rmbl.to/app/***");
}
```

## Testing Your New Platform

1. **Build the app:**
   ```bash
   cargo check --manifest-path src-tauri/Cargo.toml
   ```

2. **Run tests:**
   ```bash
   cargo test --manifest-path src-tauri/Cargo.toml platform_registry
   ```

3. **Test in the UI:**
   - Create a new stream target
   - Select your new platform from the dropdown
   - Verify the default URL is populated
   - Add a test stream key
   - Check logs to confirm stream key is masked (shows `***`)

## Common URL Patterns

| Platform | URL Pattern | stream_key_position | default_app_path |
|----------|-------------|---------------------|------------------|
| YouTube | `rtmp://a.rtmp.youtube.com/live2/KEY` | 2 | `Some("live2")` |
| Twitch | `rtmp://ingest.twitch.tv/app/KEY` | 2 | `Some("app")` |
| Facebook | `rtmps://live-api-s.facebook.com:443/rtmp/KEY` | 2 | `Some("rtmp")` |
| Kick | `rtmps://fa723fc1b171.global-contribute.live-video.net/app/KEY` | 2 | `Some("app")` |

## Security Notes

- **Stream keys are always redacted in logs** regardless of platform
- The redaction happens automatically using the `stream_key_position` setting
- Keys are also encrypted at rest using AES-256-GCM
- Never log stream keys in plaintext anywhere in the codebase

## Need Help?

- Check existing platform configurations in `platform_registry.rs`
- Look at the frontend configs in `profile.ts`
- Run tests to verify your configuration works
- Check FFmpeg logs to confirm URL normalization and key masking

---

**Last Updated**: 2026-01-06
