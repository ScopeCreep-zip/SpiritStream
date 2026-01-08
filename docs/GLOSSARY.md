# Technical Glossary

[Documentation](./README.md) > Glossary

---

Definitions for technical terms, acronyms, and domain-specific vocabulary used throughout the SpiritStream documentation. Terms are organized alphabetically with cross-references to relevant sections.

---

## A

### AAC (Advanced Audio Coding)
A lossy audio compression format standardized as part of MPEG-4. SpiritStream uses AAC as the default audio codec for streaming due to its widespread platform support and efficient compression. See [Encoding Reference](./04-streaming/04-encoding-reference.md).

### AES-256-GCM (Advanced Encryption Standard with Galois/Counter Mode)
A symmetric encryption algorithm using 256-bit keys with authenticated encryption. SpiritStream uses AES-256-GCM to encrypt profile data and stream keys at rest. The GCM mode provides both confidentiality and integrity verification. See [Encryption Implementation](./02-backend/05-encryption-implementation.md).

### API (Application Programming Interface)
A set of protocols and tools that allow software components to communicate. In SpiritStream, the API refers to the Tauri commands exposed from the Rust backend to the React frontend. See [Commands API](./05-api-reference/01-commands-api.md).

### Argon2id
A memory-hard key derivation function (KDF) resistant to GPU-based attacks. SpiritStream uses Argon2id to derive encryption keys from user passwords. It combines Argon2i (data-independent) and Argon2d (data-dependent) modes for optimal security. See [Encryption Implementation](./02-backend/05-encryption-implementation.md).

### Async/Await
A programming pattern for handling asynchronous operations. In Rust, SpiritStream uses `async`/`await` with the Tokio runtime for non-blocking I/O operations. In TypeScript, Promise-based async operations communicate with the Tauri backend.

---

## B

### Bitrate
The amount of data processed per unit of time, typically measured in kilobits per second (kbps) or megabits per second (Mbps). Video bitrate significantly affects stream quality and bandwidth requirements. See [Encoding Reference](./04-streaming/04-encoding-reference.md).

### Bundle
The packaged application distributed to end users. Tauri bundles include platform-specific installers (.msi for Windows, .dmg for macOS, .AppImage/.deb for Linux). See [Building](./07-deployment/01-building.md).

---

## C

### C4 Model
A hierarchical approach to software architecture documentation using four levels: Context, Container, Component, and Code. SpiritStream documentation uses C4-style diagrams in the architecture section. See [System Overview](./01-architecture/01-system-overview.md).

### Capability
In Tauri, a permission declaration that grants the frontend access to specific backend functionality. Capabilities are defined in JSON files and enforce the principle of least privilege. See [Security Architecture](./01-architecture/04-security-architecture.md).

### CBR (Constant Bitrate)
An encoding mode where the output bitrate remains fixed throughout the stream. CBR is recommended for live streaming as it provides predictable bandwidth usage. See [Encoding Reference](./04-streaming/04-encoding-reference.md).

### Codec
Software that encodes or decodes digital data streams. Video codecs (H.264, HEVC) compress video frames; audio codecs (AAC, MP3) compress audio. SpiritStream supports both software and hardware codecs. See [Encoding Reference](./04-streaming/04-encoding-reference.md).

### Container (Tauri)
The native window wrapper that hosts the web-based frontend. Unlike Electron, Tauri containers use the operating system's native webview rather than bundling Chromium.

### CSP (Content Security Policy)
A security standard that helps prevent cross-site scripting (XSS) and other code injection attacks. SpiritStream's Tauri configuration includes strict CSP headers. See [Security Architecture](./01-architecture/04-security-architecture.md).

---

## D

### DTO (Data Transfer Object)
An object used to transfer data between processes or layers. In SpiritStream, DTOs define the shape of data passed between the Rust backend and TypeScript frontend via Tauri IPC.

---

## E

### Encoder
Software or hardware that converts raw video/audio into a compressed format. Hardware encoders (NVENC, QuickSync, AMF) offload work to the GPU, reducing CPU usage. See [FFmpeg Integration](./04-streaming/01-ffmpeg-integration.md).

### Event (Tauri)
A message emitted from the Rust backend and received by the frontend. SpiritStream uses events for real-time updates like `stream_stats`, `stream_ended`, and `themes_updated`. See [Events API](./05-api-reference/02-events-api.md).

---

## F

### FFmpeg
A comprehensive multimedia framework for encoding, decoding, transcoding, and streaming audio and video. SpiritStream uses FFmpeg as its streaming engine. See [FFmpeg Integration](./04-streaming/01-ffmpeg-integration.md).

### FPS (Frames Per Second)
The number of individual video frames displayed per second. Common streaming frame rates are 30 and 60 FPS. Higher FPS provides smoother motion but requires more bandwidth.

### Frontend
The user interface layer of an application. In SpiritStream, the frontend is built with React 19, TypeScript, and Tailwind CSS. See [React Architecture](./03-frontend/01-react-architecture.md).

---

## G

### GCM (Galois/Counter Mode)
An authenticated encryption mode that provides both confidentiality and data integrity verification. Used with AES in SpiritStream's encryption implementation.

---

## H

### H.264 (AVC)
A widely-supported video compression standard. Also known as Advanced Video Coding (AVC) or MPEG-4 Part 10. H.264 is the most compatible codec for RTMP streaming.

### Hardware Acceleration
Using dedicated hardware (GPU) for video encoding instead of the CPU. Examples include NVIDIA NVENC, Intel QuickSync, and AMD AMF. See [Encoding Reference](./04-streaming/04-encoding-reference.md).

### Hook (React)
A function that lets React components use state and lifecycle features. SpiritStream uses custom hooks like `useStreamStats`, `useToast`, and `useFFmpegDownload`. See [React Architecture](./03-frontend/01-react-architecture.md).

---

## I

### i18n (Internationalization)
The process of designing software to support multiple languages. SpiritStream uses i18next for internationalization, supporting English, Spanish, French, German, and Japanese. See [Theming and i18n](./03-frontend/05-theming-i18n.md).

### Ingest
The entry point where a streaming service receives video data. SpiritStream receives ingest via RTMP and distributes to multiple output destinations.

### IPC (Inter-Process Communication)
Communication between separate processes. In Tauri, IPC occurs between the Rust backend and the webview frontend via the `invoke()` function. See [Tauri Integration](./03-frontend/04-tauri-integration.md).

### Invoke
The Tauri function that calls a Rust command from the frontend. Syntax: `invoke<ReturnType>('command_name', { params })`. See [Commands API](./05-api-reference/01-commands-api.md).

---

## J

### JSON (JavaScript Object Notation)
A lightweight data interchange format. SpiritStream uses JSON for profile storage, settings, and IPC communication between frontend and backend.

### JSONC
JSON with Comments. SpiritStream's theme files use the `.jsonc` extension to allow inline documentation. Standard JSON parsers may require comment stripping.

---

## K

### KDF (Key Derivation Function)
A cryptographic function that derives encryption keys from passwords or other input. SpiritStream uses Argon2id as its KDF. See [Encryption Implementation](./02-backend/05-encryption-implementation.md).

### Keyframe
A complete video frame that doesn't depend on other frames for decoding. Also called I-frames. Keyframe interval affects seeking precision and error recovery. See [Encoding Reference](./04-streaming/04-encoding-reference.md).

---

## L

### Latency
The delay between video capture and viewer playback. Lower latency improves interactivity but may reduce quality or reliability. RTMP streaming typically has 3-10 seconds of latency.

### libx264
An open-source software H.264 encoder. SpiritStream uses libx264 as the fallback when hardware encoders are unavailable. See [FFmpeg Integration](./04-streaming/01-ffmpeg-integration.md).

---

## M

### Mermaid
A JavaScript-based diagramming tool that renders diagrams from text definitions. SpiritStream documentation uses Mermaid for architecture and flow diagrams with dark theming.

### Muxer
A component that combines multiple streams (video, audio) into a container format. FFmpeg uses the `tee` muxer to output to multiple RTMP destinations simultaneously.

---

## N

### Nonce
A number used once in cryptographic operations to ensure uniqueness. SpiritStream generates a 12-byte random nonce for each AES-GCM encryption operation.

### NVENC
NVIDIA's hardware video encoder available on GeForce and Quadro GPUs. NVENC significantly reduces CPU usage during streaming. Codec identifier: `h264_nvenc`. See [Encoding Reference](./04-streaming/04-encoding-reference.md).

---

## O

### Output Group
A configuration that bundles encoding settings with one or more stream targets. Each output group can have different resolution, bitrate, and codec settings. See [Multi-Destination](./04-streaming/03-multi-destination.md).

---

## P

### Passthrough
An encoding mode where the input stream is forwarded without re-encoding (codec set to `copy`). Passthrough minimizes CPU usage and preserves original quality. See [FFmpeg Integration](./04-streaming/01-ffmpeg-integration.md).

### Platform
A streaming service destination (YouTube, Twitch, Kick, Facebook, or Custom RTMP). Each platform has specific requirements for stream keys and server URLs.

### Preset
A predefined set of encoder parameters balancing quality and performance. Common presets: `ultrafast`, `veryfast`, `medium`, `slow`. Faster presets use less CPU but produce lower quality.

### Profile (Encoder)
A subset of the codec specification defining feature support. H.264 profiles include Baseline, Main, and High. Higher profiles support more features but require more processing.

### Profile (SpiritStream)
A saved configuration containing input settings, output groups, and stream targets. Profiles can be encrypted with a password. See [Models Reference](./02-backend/03-models-reference.md).

---

## Q

### QuickSync
Intel's hardware video encoder built into Intel CPUs with integrated graphics. Codec identifier: `h264_qsv`. See [Encoding Reference](./04-streaming/04-encoding-reference.md).

---

## R

### Relay
An FFmpeg process that receives the incoming stream and distributes it to multiple output groups via UDP multicast. The relay prevents the source from being consumed multiple times. See [FFmpeg Integration](./04-streaming/01-ffmpeg-integration.md).

### RTMP (Real-Time Messaging Protocol)
A TCP-based protocol designed for streaming audio, video, and data. RTMP is the standard protocol for ingesting streams to major platforms. Default port: 1935. See [RTMP Fundamentals](./04-streaming/02-rtmp-fundamentals.md).

### Rust
A systems programming language emphasizing safety, concurrency, and performance. SpiritStream's backend is written in Rust. See [Rust Overview](./02-backend/01-rust-overview.md).

---

## S

### Salt
Random data added to a password before hashing to prevent rainbow table attacks. SpiritStream generates a 32-byte random salt for each encryption operation.

### Serde
A Rust framework for serializing and deserializing data structures. SpiritStream uses Serde for JSON serialization in IPC and file storage.

### State (Zustand)
Centralized application data managed by Zustand stores. SpiritStream has stores for profiles, streams, themes, and language settings. See [State Management](./03-frontend/02-state-management.md).

### Stream Key
A secret token that authenticates a streamer to a platform. Stream keys should never be exposed in logs or transmitted insecurely. SpiritStream supports encrypted storage and environment variable interpolation (`${ENV_VAR}`).

### Stream Target
A destination endpoint for the video stream, consisting of a platform, URL, and stream key. See [Models Reference](./02-backend/03-models-reference.md).

---

## T

### Tailwind CSS
A utility-first CSS framework. SpiritStream uses Tailwind for styling with custom design tokens defined as CSS variables. See [Theming and i18n](./03-frontend/05-theming-i18n.md).

### Tauri
A framework for building desktop applications with web technologies (HTML, CSS, JavaScript) and a Rust backend. Tauri provides smaller bundle sizes and better security than Electron. See [System Overview](./01-architecture/01-system-overview.md).

### Tee Muxer
An FFmpeg muxer that duplicates output to multiple destinations. SpiritStream uses the tee muxer to send encoded video to multiple RTMP servers simultaneously.

### Theme
A collection of CSS custom properties (variables) that define the application's visual appearance. SpiritStream supports light, dark, and custom themes. See [Theming and i18n](./03-frontend/05-theming-i18n.md).

### Tokio
An asynchronous runtime for Rust. SpiritStream uses Tokio for non-blocking file I/O and process management.

### Transcoding
Converting media from one format to another, involving decoding and re-encoding. Transcoding allows changing resolution, bitrate, or codec but is CPU-intensive.

### TypeScript
A typed superset of JavaScript. SpiritStream's frontend is written in TypeScript for improved type safety and developer experience.

---

## U

### UDP (User Datagram Protocol)
A connectionless network protocol. SpiritStream's relay uses UDP multicast (239.255.0.1:5000) to distribute the incoming stream to multiple output group processes.

### UUID (Universally Unique Identifier)
A 128-bit identifier used to uniquely identify objects. SpiritStream uses UUIDv4 for profile, output group, and stream target identifiers.

---

## V

### VBR (Variable Bitrate)
An encoding mode where bitrate varies based on content complexity. VBR can achieve better quality than CBR at the same average bitrate but may cause buffering on bandwidth-constrained connections.

### VideoToolbox
Apple's hardware video encoding framework available on macOS and iOS. Codec identifier: `h264_videotoolbox`. See [Encoding Reference](./04-streaming/04-encoding-reference.md).

### Vite
A modern frontend build tool that provides fast development server and optimized production builds. SpiritStream uses Vite for the React frontend. See [Building](./07-deployment/01-building.md).

---

## W

### WebView
A native component that renders web content. Tauri uses the operating system's native webview (WebView2 on Windows, WebKit on macOS/Linux) rather than bundling a browser engine.

---

## Z

### Zustand
A lightweight state management library for React. SpiritStream uses Zustand for global state including profiles, stream status, themes, and language settings. See [State Management](./03-frontend/02-state-management.md).

---

## Acronym Reference

| Acronym | Full Form |
|---------|-----------|
| AAC | Advanced Audio Coding |
| AES | Advanced Encryption Standard |
| AMF | Advanced Media Framework (AMD) |
| API | Application Programming Interface |
| AVC | Advanced Video Coding |
| CBR | Constant Bitrate |
| CSP | Content Security Policy |
| DTO | Data Transfer Object |
| FPS | Frames Per Second |
| GCM | Galois/Counter Mode |
| GPU | Graphics Processing Unit |
| HEVC | High Efficiency Video Coding |
| i18n | Internationalization |
| IPC | Inter-Process Communication |
| JSON | JavaScript Object Notation |
| KDF | Key Derivation Function |
| NVENC | NVIDIA Encoder |
| RTMP | Real-Time Messaging Protocol |
| UDP | User Datagram Protocol |
| UUID | Universally Unique Identifier |
| VBR | Variable Bitrate |

---

**Related:** [System Overview](./01-architecture/01-system-overview.md) | [Types Reference](./05-api-reference/03-types-reference.md)
