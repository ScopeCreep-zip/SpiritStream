# SpiritStream Development Roadmap

**Last Updated**: 2026-01-05
**Current Version**: 0.1.0 (Pre-release)

---

## Overview

SpiritStream is positioned to become a comprehensive streaming solution, starting as an RTMP relay and evolving into a full OBS replacement. This roadmap outlines the development path from the current state through major feature releases.

---

## Current Architecture (v0.1.0)

### Technology Stack
- **Desktop Framework**: Tauri 2.x
- **Backend**: Rust (FFmpeg handler, profile management, encryption)
- **Frontend**: React 18+ with TypeScript
- **Styling**: Tailwind CSS v4
- **State Management**: Zustand
- **i18n**: 5 languages (en, de, es, fr, ja)

### Core Features (Implemented ✅)
1. **RTMP Relay Server**
   - UDP multicast relay for shared ingest
   - Multiple output groups with independent FFmpeg processes
   - Passthrough mode (copy) as default

2. **Profile Management**
   - AES-256-GCM encryption with Argon2id key derivation
   - Password-protected profiles
   - Profile import/export

3. **Output Groups**
   - Unlimited passthrough groups (copy mode)
   - Unlimited re-encode groups (hardware/software encoders)
   - Independent start/stop per group
   - Immutable default passthrough group

4. **Hardware Encoder Support**
   - NVENC (NVIDIA)
   - QuickSync (Intel)
   - AMF (AMD)
   - VideoToolbox (Apple Silicon)
   - Auto-detection and validation

5. **Stream Targets**
   - YouTube, Twitch, Kick, Facebook, Custom RTMP
   - Encrypted stream key storage
   - Per-target enable/disable (requires group restart)
   - Environment variable support (${ENV_VAR})

6. **Statistics & Monitoring**
   - Real-time FFmpeg stats (bitrate, fps, dropped frames)
   - Per-group stats aggregation
   - Dashboard totals (total bitrate, uptime, active streams)
   - 1-second emission rate limiting

7. **FFmpeg Management**
   - Auto-download with version checking
   - Custom path configuration
   - Bundled FFmpeg support

---

## Recent Fixes (2026-01-05)

### Critical Fixes ✅
1. **Relay Race Condition**: Added atomic reference counting to prevent premature relay shutdown when multiple groups finish simultaneously
2. **Poisoned Mutex Handling**: Graceful recovery from poisoned mutexes throughout FFmpegHandler
3. **Case-Insensitive Codec Detection**: Passthrough mode now handles "Copy", "COPY", etc.

---

## Version 1.0 - Production Release

**Target**: Q1 2026
**Status**: Release Candidate (cleanup-release-cand branch)

### Goals
- ✅ Stable RTMP relay architecture
- ✅ Comprehensive hardware encoder support
- ✅ Full documentation and user guides
- 🔲 End-to-end testing across platforms
- 🔲 Performance benchmarks
- 🔲 User onboarding flow

### Remaining Tasks
1. **Testing**
   - Platform-specific testing (Windows, macOS, Linux)
   - Load testing (10+ simultaneous targets)
   - Hardware encoder validation on different GPUs
   - Edge case handling (connection loss, encoder crashes)

2. **Documentation**
   - User guide with screenshots
   - Troubleshooting guide
   - Video tutorials
   - API documentation

3. **UI/UX Polish**
   - Onboarding wizard for first-time users
   - Tooltips and help system
   - Accessibility improvements (keyboard navigation, screen readers)
   - Error message clarity

4. **Performance Optimization**
   - Memory leak detection and fixing
   - CPU usage profiling
   - Network buffer tuning

---

## Version 1.1 - Enhanced Target Control

**Target**: Q2 2026
**Focus**: Per-target statistics and improved target management

### Features

#### 1. Per-Target Statistics (Optional Enhancement)
**Status**: Research Phase
**Complexity**: High

**Problem**: FFmpeg's tee muxer outputs aggregated stats for all targets in a group, not per-target stats.

**Approaches**:

| Approach | Pros | Cons | Feasibility |
|----------|------|------|-------------|
| **Separate Processes Per Target** | True isolation, full stats | 3x-10x more processes | Medium |
| **Network-Level Monitoring** | Non-invasive | Requires platform APIs, indirect | High |
| **FFmpeg `-vstats` Filter** | Built-in FFmpeg support | Adds encoding overhead | Medium |
| **Parse Detailed FFmpeg Logs** | No arch changes | Unreliable, format changes | Low |

**Recommended**: Network-level monitoring for v1.1, consider separate processes for v2.0

**Implementation Plan**:
1. Add network monitoring service (Rust)
2. Track RTMP ACK packets per connection
3. Infer per-target bitrate from network stats
4. Update StreamStore to support per-target stats
5. Add per-target stats display in UI

**Timeline**: 4-6 weeks

---

#### 2. Dynamic Target Toggling (Major Enhancement)
**Status**: Design Phase
**Complexity**: High

**Current Behavior**: Toggling a target ON/OFF restarts the entire output group (~1-3 second interruption for all targets).

**Problem**: Restarting Group A (YouTube + Twitch) to toggle YouTube affects Twitch streaming.

**Solution Options**:

##### Option A: Dynamic Tee Muxer (Recommended)
Use named pipes (FIFOs) for each target, allowing FFmpeg to write to pipes independently.

**Architecture**:
```
FFmpeg (Group A)
  ├─> Named Pipe 1 (YouTube) ──> Writer Process ──> RTMP YouTube
  ├─> Named Pipe 2 (Twitch)  ──> Writer Process ──> RTMP Twitch
  └─> Named Pipe 3 (Kick)    ──> Writer Process ──> RTMP Kick
```

**Pros**:
- No FFmpeg restarts needed
- True per-target control
- Lower overhead than separate processes

**Cons**:
- Platform-specific (Windows named pipes differ from Unix FIFOs)
- More complex implementation
- Need to manage writer processes

**Implementation Complexity**: High (8-10 weeks)

##### Option B: Separate Processes Per Target
Each target gets its own FFmpeg process reading from UDP relay.

**Pros**:
- Simplest architecture
- Full isolation
- Independent stats

**Cons**:
- 3x-10x more processes
- Higher memory overhead (~100MB per process)
- More complex process management

**Implementation Complexity**: Medium (4-6 weeks)

##### Option C: Keep Current + Optimize (Fallback)
- Minimize group sizes (default: 1 target per group)
- Document restart behavior
- UI warning before toggle
- Fast restart optimization (<500ms)

**Implementation Complexity**: Low (1-2 weeks)

**Recommendation**: Start with Option C for v1.1, research Option A for v1.2

---

#### 3. Target Health Monitoring
**Status**: Design Phase
**Complexity**: Medium

**Features**:
- Detect connection failures per target
- Retry logic with exponential backoff
- Health status indicators (healthy, degraded, failed)
- Automatic failover to backup targets

**Implementation Plan**:
1. Monitor FFmpeg error output for per-target failures
2. Parse RTMP connection events
3. Add retry state machine
4. UI indicators for target health

**Timeline**: 3-4 weeks

---

## Version 1.2 - OBS Replacement Foundations

**Target**: Q3 2026
**Focus**: Capturing local video/audio sources (first step toward OBS replacement)

### Features

#### 1. Desktop Capture Sources
**Status**: Design Phase
**Complexity**: High

**Goal**: Capture desktop, windows, and application sources for streaming.

**Platform Support**:

| Platform | Capture API | FFmpeg Input |
|----------|-------------|--------------|
| Windows | Desktop Duplication API | `-f gdigrab` |
| macOS | AVFoundation | `-f avfoundation` |
| Linux | X11 / Wayland | `-f x11grab` (X11), `-f kmsgrab` (Wayland) |

**Features**:
- Full desktop capture
- Window-specific capture
- Application capture
- Region/crop selection
- Cursor overlay toggle

**Architecture Changes**:
- New `CaptureSource` model
- Capture source manager (Rust)
- Source selector UI
- Preview window

**FFmpeg Command Example**:
```bash
# Windows - Capture desktop
ffmpeg -f gdigrab -i desktop -c:v h264_nvenc -preset fast -b:v 6000k ...

# macOS - Capture desktop
ffmpeg -f avfoundation -i "0" -c:v h264_videotoolbox -b:v 6000k ...

# Linux (X11) - Capture desktop
ffmpeg -f x11grab -i :0.0 -c:v libx264 -preset fast -b:v 6000k ...
```

**Implementation Plan**:
1. Add capture source types to models
2. Implement platform-specific capture managers
3. Add source preview window
4. Update FFmpeg handler to accept capture sources
5. UI for source selection and configuration

**Timeline**: 8-12 weeks

---

#### 2. Audio Device Management
**Status**: Design Phase
**Complexity**: Medium

**Goal**: Capture and mix audio from multiple sources.

**Platform Support**:

| Platform | Audio API | FFmpeg Input |
|----------|-----------|--------------|
| Windows | DirectShow | `-f dshow` |
| macOS | AVFoundation | `-f avfoundation` |
| Linux | PulseAudio / ALSA | `-f pulse` / `-f alsa` |

**Features**:
- Microphone input
- Desktop audio capture
- Multiple audio source mixing
- Per-source volume control
- Audio monitoring

**FFmpeg Filter Complex Example**:
```bash
# Mix microphone + desktop audio
ffmpeg \
  -f dshow -i audio="Microphone" \
  -f dshow -i audio="Desktop Audio" \
  -filter_complex "[0:a][1:a]amix=inputs=2:duration=longest[aout]" \
  -map "[aout]" \
  -c:a aac -b:a 160k ...
```

**Implementation Plan**:
1. Audio device enumeration (Rust + platform APIs)
2. Audio mixer using FFmpeg `-filter_complex`
3. Volume control UI
4. Audio monitoring (local playback)

**Timeline**: 6-8 weeks

---

#### 3. Camera Input
**Status**: Design Phase
**Complexity**: Low

**Goal**: Add webcam/camera inputs.

**Platform Support**: FFmpeg's device input filters (same as audio)

**Features**:
- Camera device selection
- Resolution and FPS configuration
- Position and scale control (overlay)

**FFmpeg Example**:
```bash
# Windows - Add webcam overlay
ffmpeg \
  -f gdigrab -i desktop \
  -f dshow -i video="Webcam" \
  -filter_complex "[0:v][1:v]overlay=W-w-10:H-h-10[out]" \
  -map "[out]" \
  -c:v h264_nvenc ...
```

**Timeline**: 3-4 weeks

---

## Version 2.0 - Full OBS Replacement

**Target**: Q4 2026
**Focus**: Scene management, transitions, effects

### Features

#### 1. Scene Management System
**Status**: Planning
**Complexity**: Very High

**Goal**: Multi-source compositing with scene switching.

**Architecture**:
```
Profile
  ├── Scene 1 (Gaming)
  │     ├── Source: Desktop Capture
  │     ├── Source: Webcam (overlay, bottom-right)
  │     ├── Source: Browser (chat, right panel)
  │     └── Source: Image (logo, top-left)
  ├── Scene 2 (BRB Screen)
  │     ├── Source: Image (BRB background)
  │     └── Source: Text (scrolling text)
  └── Scene 3 (Talking Head)
        └── Source: Webcam (fullscreen)
```

**Features**:
- Unlimited scenes per profile
- Multiple sources per scene
- Source layering (z-index)
- Source transformations (position, scale, rotation, crop)
- Scene preview
- Hotkey switching

**Implementation**:
- New `Scene` and `Source` models
- Scene manager (Rust)
- FFmpeg filter complex generation
- Scene switching without interruption
- Scene preview renderer

**Timeline**: 16-20 weeks

---

#### 2. Transitions
**Status**: Planning
**Complexity**: High

**Goal**: Smooth transitions between scenes.

**Transition Types**:
- Cut (instant)
- Fade
- Swipe
- Dissolve
- Custom (FFmpeg xfade filter)

**FFmpeg Transition Example**:
```bash
# Fade transition between two scenes
ffmpeg \
  -i scene1.mp4 \
  -i scene2.mp4 \
  -filter_complex "[0:v][1:v]xfade=transition=fade:duration=1:offset=5[out]" \
  -map "[out]" ...
```

**Implementation**:
- Transition engine
- Transition preview
- Hotkey configuration

**Timeline**: 6-8 weeks

---

#### 3. Effects & Filters
**Status**: Planning
**Complexity**: Medium

**Goal**: Real-time video effects.

**Effect Categories**:
- **Color Correction**: Brightness, contrast, saturation, hue
- **Filters**: Blur, sharpen, noise reduction
- **Chroma Key**: Green screen removal
- **Overlays**: Images, text, shapes
- **Audio Filters**: Noise suppression, compression, EQ

**FFmpeg Filter Examples**:
```bash
# Chroma key (green screen)
ffmpeg -i input.mp4 -vf "chromakey=green:0.1:0.2" ...

# Color correction
ffmpeg -i input.mp4 -vf "eq=brightness=0.1:contrast=1.2:saturation=1.5" ...

# Audio noise suppression
ffmpeg -i input.mp4 -af "afftdn=nf=-20" ...
```

**Implementation**:
- Filter system (Rust)
- Real-time filter preview
- Per-source filter stack
- Filter preset library

**Timeline**: 10-12 weeks

---

#### 4. Virtual Camera Output
**Status**: Planning
**Complexity**: High

**Goal**: Output SpiritStream scenes as a virtual webcam for use in video calls.

**Platform Support**:

| Platform | Virtual Camera Solution |
|----------|------------------------|
| Windows | OBS Virtual Camera (DirectShow filter) |
| macOS | CoreMediaIO plugin |
| Linux | v4l2loopback kernel module |

**Implementation**:
- Platform-specific virtual camera drivers
- FFmpeg output to virtual device
- UI toggle for virtual camera

**Timeline**: 8-10 weeks

---

#### 5. Recording
**Status**: Planning
**Complexity**: Low

**Goal**: Local recording alongside streaming.

**Features**:
- Simultaneous stream + record
- Separate recording settings (higher quality)
- Multiple output formats (MP4, MKV, MOV)
- Recording pause/resume
- File management

**FFmpeg Example**:
```bash
# Stream to Twitch + record locally
ffmpeg -i input \
  -c:v h264_nvenc -b:v 6000k \
  -f tee "[f=flv]rtmp://twitch.tv/app/key|[f=mp4]recording.mp4"
```

**Implementation**:
- Add recording output type
- Recording controls UI
- File browser integration

**Timeline**: 3-4 weeks

---

## Version 3.0 - Advanced Features

**Target**: Q1 2027
**Focus**: Cloud integration, collaboration, analytics

### Features

#### 1. Cloud Profiles
- Profile sync across devices
- Cloud backup
- Team collaboration (shared profiles)

#### 2. Stream Analytics
- Viewer count history
- Bitrate graphs
- Frame drop analysis
- Platform-specific stats (YouTube, Twitch APIs)

#### 3. Stream Alerts & Overlays
- Donation alerts
- Follower/subscriber notifications
- Chat overlay
- WebSocket/WebRTC integration

#### 4. Stream Replay Buffer
- Local buffering for instant replay
- Clip export
- Highlight markers

#### 5. NDI Support
- NDI input sources
- NDI output for multi-PC streaming

---

## Performance Goals

### Current Performance (v0.1.0)

| Metric | Current | Target (v1.0) |
|--------|---------|---------------|
| Binary Size | ~10MB | <15MB |
| Memory (Idle) | ~40MB | <50MB |
| Memory (Streaming 3 targets, passthrough) | ~120MB | <150MB |
| Memory (Streaming 3 targets, 1080p NVENC) | ~200MB | <250MB |
| CPU (Relay Process) | 2-5% | <5% |
| CPU (Passthrough Group, 3 targets) | 15-20% | <20% |
| CPU (1080p30 libx264) | 80-120% | <100% |
| CPU (1080p30 NVENC) | 8-12% | <10% |
| Startup Time | <2s | <1s |

### OBS Replacement Performance (v2.0 Target)

| Metric | OBS (Baseline) | Target |
|--------|----------------|--------|
| Binary Size | 150MB | <30MB |
| Memory (Idle) | 200MB | <100MB |
| Memory (Streaming + Recording) | 400MB | <300MB |
| CPU (Desktop Capture + Encode) | 15-25% | <20% |

---

## Architecture Evolution

### Current Architecture (v0.1.0)
```
OBS (External Encoder)
  └─> RTMP ──> SpiritStream Relay ──> UDP Multicast
                                           ├─> Group A (Passthrough) ──> Twitch + YouTube
                                           ├─> Group B (Re-encode) ──> Kick
                                           └─> Group C (Re-encode) ──> Facebook
```

### Target Architecture (v2.0)
```
SpiritStream
  ├─> Capture Manager (Desktop, Camera, Audio)
  ├─> Scene Manager (Compositing, Transitions)
  ├─> Filter Engine (Effects, Color Correction)
  └─> Output Manager
        ├─> Encoder Groups (NVENC, QuickSync, libx264)
        ├─> Stream Targets (YouTube, Twitch, etc.)
        ├─> Recording Outputs (MP4, MKV)
        └─> Virtual Camera Output
```

---

## Technology Debt & Refactoring

### Identified Technical Debt
1. ~~**Relay Race Condition**~~ ✅ Fixed (2026-01-05)
2. ~~**Poisoned Mutex Handling**~~ ✅ Fixed (2026-01-05)
3. **Incoming URL Change**: Changing incoming URL requires stopping all streams (design limitation)
4. **Relay Stderr Thread**: No JoinHandle stored for cleanup tracking
5. **Lock Ordering**: Inconsistent lock acquisition order (low priority, no circular deps)

### Planned Refactoring (v1.1+)
1. **Modular FFmpeg Builder**: Extract FFmpeg argument building into separate module
2. **State Machine for Stream Lifecycle**: Formal state machine (offline → connecting → live → stopping → offline)
3. **Pluggable Encoder System**: Abstract encoder interface for easier encoder additions
4. **Event Bus**: Replace direct Tauri emits with event bus pattern
5. **Async Rust**: Migrate blocking operations to Tokio async runtime

---

## Dependencies & External Tools

### Current Dependencies
- **FFmpeg**: Core encoding/streaming engine
- **Tauri 2.x**: Desktop framework
- **Rust Ecosystem**: tokio, serde, aes-gcm, argon2, reqwest
- **React Ecosystem**: React, Zustand, Tailwind, i18next

### Future Dependencies (v2.0+)
- **Platform Capture APIs**: Desktop Duplication (Windows), AVFoundation (macOS), X11/Wayland (Linux)
- **Audio APIs**: DirectShow (Windows), CoreAudio (macOS), PulseAudio (Linux)
- **Virtual Camera**: v4l2loopback (Linux), CoreMediaIO (macOS), DirectShow filter (Windows)
- **NDI SDK**: NewTek NDI (optional, v3.0)

---

## Testing Strategy

### Current Testing (v0.1.0)
- Manual testing
- Platform-specific validation
- No automated tests

### Planned Testing (v1.0+)

#### Unit Tests
- Rust backend logic (profile manager, encryption, FFmpeg args builder)
- Frontend stores and utilities
- Target coverage: 70%

#### Integration Tests
- FFmpeg process lifecycle
- Profile save/load with encryption
- Stream start/stop/restart flows
- Target coverage: 50%

#### End-to-End Tests
- Full streaming workflows
- UI automation (Tauri test harness)
- Cross-platform validation
- Target coverage: 30%

#### Performance Tests
- Memory leak detection (valgrind, instruments)
- CPU profiling (perf, Instruments)
- Load testing (10+ concurrent streams)
- Benchmarking regression detection

---

## Community & Contributions

### v1.0 Release Goals
- Public GitHub repository
- Contribution guidelines
- Issue templates
- Code of conduct
- Discord community server

### v2.0 Goals
- Plugin system for custom sources/filters
- Community preset library
- Theme customization
- Translation contributions (expand from 5 to 10+ languages)

---

## License & Legal

**Current License**: GPL-3.0
**Rationale**: Ensures derivative works remain open source

**Future Considerations**:
- Dual licensing (GPL + commercial) for enterprise features (v3.0+)
- FFmpeg compliance (GPL/LGPL depending on build configuration)
- Platform SDK licensing (NDI, virtual camera drivers)

---

## Milestones Summary

| Version | Target Date | Key Features | Status |
|---------|-------------|--------------|--------|
| **v0.1.0** | Q1 2026 | RTMP Relay, Profiles, Hardware Encoders | 🟢 RC |
| **v1.0** | Q1 2026 | Stable Release, Documentation, Testing | 🟡 In Progress |
| **v1.1** | Q2 2026 | Per-Target Stats, Target Health Monitoring | 🔵 Planned |
| **v1.2** | Q3 2026 | Desktop Capture, Audio Mixing, Camera Input | 🔵 Planned |
| **v2.0** | Q4 2026 | Scenes, Transitions, Filters, OBS Replacement | 🔵 Planned |
| **v3.0** | Q1 2027 | Cloud Sync, Analytics, Collaboration | 🔵 Planned |

---

**Document Version**: 1.0
**Last Reviewed**: 2026-01-05
**Next Review**: 2026-02-01
