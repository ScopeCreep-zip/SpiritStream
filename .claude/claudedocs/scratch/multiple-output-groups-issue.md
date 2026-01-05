# Multiple Output Groups FFmpeg Issue

**Date**: 2026-01-04
**Issue**: Second output group doesn't receive stream when both use same incoming URL
**Status**: 🔍 **Investigating**

## Problem Report

User tried to stream:
- **YouTube** → Passthrough group (copy mode)
- **Twitch** → Re-encoding group (transcode to 6000k)

YouTube received the stream fine, but **Twitch did not receive anything**.

## Root Cause Analysis

### Current Architecture

Each output group creates a **separate FFmpeg process**:

**Passthrough Group (Default)**:
```bash
ffmpeg -listen 1 -i rtmp://0.0.0.0:1935/live \
  -c:v copy -c:a copy \
  -map 0:v -map 0:a \
  -f flv rtmp://a.rtmp.youtube.com/live2/{key}
```

**Re-encode Group (Twitch)**:
```bash
ffmpeg -listen 1 -i rtmp://0.0.0.0:1935/live \
  -c:v libx264 -s 1920x1080 -b:v 6000k -r 30 \
  -c:a aac -b:a 160k -ac 2 -ar 48000 \
  -map 0:v -map 0:a \
  -f flv rtmp://live.twitch.tv/app/{key}
```

### The Problem

**Both processes use `-listen 1` flag with the same incoming URL**.

The `-listen 1` flag tells FFmpeg to **listen as an RTMP server** on the input URL.

❌ **Only ONE process can bind to a socket** (network limitation)
- First FFmpeg process (passthrough) successfully binds to `:1935/live`
- Second FFmpeg process (re-encode) **fails to bind** because port is already in use
- Second process may:
  - Fail silently
  - Wait indefinitely
  - Exit with error (but error not visible to user)

## Solutions

### Option 1: Single FFmpeg with Multiple Outputs ⚠️ Won't Work

Use one FFmpeg process with multiple outputs (tee muxer or multiple `-map`):

```bash
ffmpeg -listen 1 -i rtmp://0.0.0.0:1935/live \
  -map 0:v -map 0:a -c:v copy -c:a copy -f flv rtmp://youtube/{key} \
  -map 0:v -map 0:a -c:v libx264 -b:v 6000k -c:a aac -f flv rtmp://twitch/{key}
```

**Problem**: Can't have different encoding settings per output in a simple way.
**Problem**: Breaks our architecture where each output group is independent.
**Problem**: Complex FFmpeg command generation.

### Option 2: First Process Receives, Others Read ✅ RECOMMENDED

**Architecture Change**:
1. **First output group** (e.g., default passthrough): Uses `-listen 1` to receive from OBS
2. **Additional output groups**: Read from the first FFmpeg's output (not from OBS)

**Problem**: Requires the first process to **also output to a local RTMP server** that others can read from.
**Complexity**: Needs intermediate RTMP relay.

### Option 3: Input Application Name Per Group ❌ Breaks UX

Each output group uses a different application name:
- Passthrough: `rtmp://0.0.0.0:1935/live`
- Re-encode: `rtmp://0.0.0.0:1935/twitch`

User would need to configure OBS to stream to **both** URLs simultaneously.

**Problem**: User can't stream to multiple destinations from a single OBS stream.
**Problem**: Defeats the purpose of SpiritStream as a relay.

### Option 4: Chain FFmpeg Processes ✅ SIMPLE & WORKS

**Approach**:
1. **First FFmpeg** (default group): Listens for OBS stream, outputs to YouTube + local file/pipe
2. **Second FFmpeg** (custom group): Reads from local file/pipe, re-encodes, outputs to Twitch

**Using Unix pipes/named pipes**:
```bash
# Process 1: Passthrough to YouTube + output to pipe
ffmpeg -listen 1 -i rtmp://0.0.0.0:1935/live \
  -c:v copy -c:a copy \
  -f flv rtmp://youtube/{key} \
  -c:v copy -c:a copy \
  -f flv pipe:1

# Process 2: Read from pipe, re-encode to Twitch
ffmpeg -i pipe:0 \
  -c:v libx264 -b:v 6000k -c:a aac \
  -f flv rtmp://twitch/{key}
```

**Problem**: Inter-process communication complexity.
**Problem**: Buffering issues.
**Problem**: If first process dies, all others die.

### Option 5: Dedicated RTMP Relay (nginx-rtmp or mediamtx) ✅ ROBUST

**Architecture**:
1. **RTMP relay** (nginx-rtmp or mediamtx): Receives stream from OBS
2. **FFmpeg processes**: Each reads from the relay

```
OBS → RTMP Relay (localhost:1936) → FFmpeg 1 (passthrough to YouTube)
                                   → FFmpeg 2 (re-encode to Twitch)
                                   → FFmpeg N (...)
```

**Pros**:
- Scalable (N output groups)
- Reliable (relay is separate from processing)
- Each FFmpeg is independent

**Cons**:
- Requires bundling/installing nginx-rtmp or mediamtx
- Extra complexity

### Option 6: First FFmpeg as Relay, Others as Clients ✅ SIMPLEST FIX

**Architecture Change**:
1. **First output group**: Uses `-listen 1` to receive from OBS, re-publishes to `rtmp://localhost:1936/stream`
2. **Subsequent groups**: Read from `rtmp://localhost:1936/stream` (no `-listen`)

**First process (Passthrough + Relay)**:
```bash
ffmpeg -listen 1 -i rtmp://0.0.0.0:1935/live \
  -c:v copy -c:a copy -f flv rtmp://youtube/{key} \
  -c:v copy -c:a copy -f flv rtmp://localhost:1936/stream
```

**Second process (Re-encode)**:
```bash
ffmpeg -re -i rtmp://localhost:1936/stream \
  -c:v libx264 -b:v 6000k -c:a aac \
  -f flv rtmp://twitch/{key}
```

**Problem**: Requires first FFmpeg to have a local RTMP server listening on `:1936`.
**Solution**: Use file or named pipe instead of RTMP.

### Option 7: File-Based Relay ✅ PRACTICAL

**Architecture**:
1. **First output group**: Outputs to BOTH YouTube AND a local RTMP URL
2. **Local RTMP server**: Built into first FFmpeg using `-f flv rtmp://localhost:PORT/stream`
3. **Problem**: Need another RTMP server.

**Better: HLS or File**:
```bash
# First process: Output to YouTube + HLS
ffmpeg -listen 1 -i rtmp://0.0.0.0:1935/live \
  -c:v copy -c:a copy -f flv rtmp://youtube/{key} \
  -c:v copy -c:a copy -f hls /tmp/stream.m3u8

# Second process: Read from HLS
ffmpeg -re -i /tmp/stream.m3u8 \
  -c:v libx264 -b:v 6000k -c:a aac \
  -f flv rtmp://twitch/{key}
```

**Problem**: Latency (HLS segments).
**Problem**: File I/O overhead.

## Recommended Solution: **Shared Input via TCP Socket** ✅

### Final Architecture

**Use FFmpeg TCP streaming for intermediate relay**:

1. **First FFmpeg** (receives from OBS):
   - Listens on `:1935/live` for OBS
   - Outputs to YouTube (passthrough)
   - **Also outputs to TCP server** on `localhost:5000`

2. **Subsequent FFmpeg processes**:
   - Read from `tcp://localhost:5000`
   - Apply their own encoding
   - Output to their targets

**Commands**:

**Process 1 (Passthrough to YouTube + TCP relay)**:
```bash
ffmpeg -listen 1 -i rtmp://0.0.0.0:1935/live \
  -c:v copy -c:a copy -f flv rtmp://youtube/{key} \
  -c:v copy -c:a copy -f mpegts tcp://127.0.0.1:5000?listen=1
```

**Process 2 (Re-encode to Twitch)**:
```bash
ffmpeg -i tcp://127.0.0.1:5000 \
  -c:v libx264 -s 1920x1080 -b:v 6000k -r 30 \
  -c:a aac -b:a 160k \
  -f flv rtmp://twitch/{key}
```

### Implementation

**Changes needed**:

1. **Detect if multiple output groups exist**
2. **First group**: Add TCP relay output (`-c:v copy -c:a copy -f mpegts tcp://127.0.0.1:5000?listen=1`)
3. **Other groups**: Change input from `-listen 1 -i rtmp://...` to `-i tcp://127.0.0.1:5000`

## Alternative: FFserver Replacement - **SRS (Simple RTMP Server)**

Embed a lightweight RTMP server (SRS) that:
- Receives stream from OBS
- Serves stream to multiple FFmpeg processes

**Too complex for this fix**.

## Immediate Fix: Use Different Input Methods

### Short-term Solution

**Don't use `-listen 1` for multiple processes**. Instead:

1. **First process** (default group): Uses `-listen 1` to receive from OBS
2. **Add output to first process**: `-c:v copy -c:a copy -f mpegts udp://127.0.0.1:5000`
3. **Subsequent processes**: Use `-i udp://127.0.0.1:5000` (no `-listen`)

**UDP multicast**:
```bash
# First process
ffmpeg -listen 1 -i rtmp://0.0.0.0:1935/live \
  -c:v copy -c:a copy -f flv rtmp://youtube/{key} \
  -c:v copy -c:a copy -f mpegts udp://239.0.0.1:5000

# Other processes
ffmpeg -i udp://239.0.0.1:5000 \
  -c:v libx264 -b:v 6000k -c:a aac \
  -f flv rtmp://twitch/{key}
```

**Problem with UDP**: Packet loss, unreliable on some systems.

---

## Decision

Use **TCP relay** approach (Option 7):
- First output group adds TCP output
- Subsequent groups read from TCP
- Clean, reliable, low latency
- No external dependencies

---

**Status**: Fix needed in `ffmpeg_handler.rs`
