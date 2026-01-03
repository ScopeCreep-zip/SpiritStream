# Streaming Architecture Overview

This document defines the core configuration concepts used by the system, their responsibilities, and how they interrelate at runtime.

The current architecture assumes **a single type of input**:
> An incoming RTMP stream received over the network on a defined address and port.

---

## Core Concepts

### Profile

A **Profile** is the top-level persisted configuration object.

It represents a complete, reusable streaming setup and is the unit that users load, save, duplicate, encrypt, and select at application startup.

A profile owns:
- One **RTMP Input definition**
- One or more **Output Groups**
- Optional UI or behavioral preferences

**Responsibilities**
- Persistence and encryption
- Configuration grouping
- Lifecycle ownership of subordinate objects

**Mental model**
> “Everything required to run a stream, saved as a named bundle.”

---

### RTMP Input (Network Ingest)

An **RTMP Input** defines how the application receives an incoming stream.

At present, **this is the only supported input type**.

It is defined purely in network terms:
- Bind address (interface)
- TCP port
- Application / path (optional, service-dependent)

**Example**
- `rtmp://0.0.0.0:1935/live`
- `rtmp://127.0.0.1:1936/ingest`

**Responsibilities**
- Declare where FFmpeg listens for incoming media
- Provide a stable ingest endpoint for encoders (OBS, etc.)

**Non-responsibilities**
- Encoding
- Authentication (future concern)
- Output routing

**Mental model**
> “Where the stream enters the system.”

---

### Output Group

An **Output Group** defines *how the input stream is encoded*.

Each output group produces **exactly one encoded stream**, which can be fanned out to multiple destinations.

An output group contains:
- Video encoding settings
- Audio encoding settings
- Container / mux settings
- One or more **Stream Targets**

**Responsibilities**
- FFmpeg encoding configuration
- Ensuring consistent output characteristics
- Preventing redundant re-encoding

**Mental model**
> “One encoding recipe.”

---

### Stream Target

A **Stream Target** defines *where an encoded stream is sent*.

It represents a single destination endpoint.

Examples:
- YouTube RTMP ingest
- Twitch RTMP ingest
- Custom RTMP server
- Local RTMP relay (future)

A stream target contains:
- Destination URL
- Stream key or token
- Service-specific flags (primary/backup, etc.)

**Responsibilities**
- Destination addressing
- Credential binding
- Per-destination metadata

**Non-responsibilities**
- Encoding
- Input handling

**Mental model**
> “One place to send the stream.”

---

## Composition and Relationships

The hierarchy is strict and intentional:

At runtime:
1. FFmpeg listens on the RTMP input address and port
2. Each output group encodes the incoming stream once or uses the incoming stream directly
3. Each stream target receives that encoded output

---

## Design Rationale

This separation ensures:

- No duplicated encoding work
- Clear ownership boundaries
- Deterministic FFmpeg command generation
- Future extensibility (recording, preview, relays)

Each concept has exactly one reason to change.

---

## One-Sentence Summary

> A **Profile** defines *what runs*, an **RTMP Input** defines *where media enters*, **Output Groups** define *how it is encoded*, and **Stream Targets** define *where it goes*.