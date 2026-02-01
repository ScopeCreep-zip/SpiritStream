# Streaming Documentation

[Documentation](../README.md) > Streaming

---

## Overview

This section documents SpiritStream's streaming capabilities, including FFmpeg integration, RTMP protocol fundamentals, multi-destination architecture, and encoding configuration.

## Documents

| Document | Description | Audience |
|----------|-------------|----------|
| [01. FFmpeg Integration](./01-ffmpeg-integration.md) | Process management, relay architecture | Advanced |
| [02. RTMP Fundamentals](./02-rtmp-fundamentals.md) | Protocol basics for streaming | Beginner+ |
| [03. Multi-Destination](./03-multi-destination.md) | Output groups and target management | Intermediate+ |
| [04. Encoding Reference](./04-encoding-reference.md) | Codecs, presets, hardware acceleration | All levels |
| [05. Platform Registry](./05-platform-registry.md) | 80+ platforms, adding new platforms | All levels |

## Streaming Flow

```
RTMP Input (OBS) → SpiritStream → Multiple RTMP Outputs
                        ↓
    ┌───────────────────┼───────────────────┐
    ↓                   ↓                   ↓
YouTube             Twitch              Kick
```

## Key Concepts

- **Passthrough Mode**: Forward stream without re-encoding (codec: `copy`)
- **Relay Process**: UDP multicast distribution to output groups
- **Output Group**: Encoding settings + stream targets bundle
- **Tee Muxer**: FFmpeg's multi-output mechanism

## Supported Platforms

SpiritStream supports **80+ streaming platforms** via a JSON-driven registry. Common platforms include:

| Platform | Default Server | Protocol |
|----------|---------------|----------|
| YouTube | rtmps://a.rtmps.youtube.com:443/live2 | RTMPS |
| Twitch | rtmp://live.twitch.tv/app | RTMP |
| Kick | rtmps://...global-contribute.live-video.net/app | RTMPS |
| Facebook | rtmps://rtmp-api.facebook.com:443/rtmp/ | RTMPS |
| Custom | User-defined | RTMP/RTMPS |

For the complete list and adding new platforms, see [Platform Registry](./05-platform-registry.md).

---

*Section: 04-streaming*

