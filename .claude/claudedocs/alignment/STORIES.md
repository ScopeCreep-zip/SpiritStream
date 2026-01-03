# User Stories for MagillaStream

The following user stories describe how MagillaStream supports real-world streaming workflows. These stories are written from the perspective of six streamers — **Yin, Yang, Yuma, Yuki, Yennifur, and Yulia** — who live together in a shared streamer house and use a single, shared streaming studio and computer.

Each streamer maintains multiple profiles to support different platforms, audiences, and stream formats. MagillaStream enables them to switch quickly, avoid reconfiguration errors, and stream to one or many destinations efficiently.

---

## 1. Profile-Based Streaming Workflows

### 1.1 Create a Streaming Profile

**As Yin**,
**I want to create multiple streaming profiles**,
**So that I can quickly switch between streaming only to Twitch, only to YouTube, or to both at once without reconfiguring everything.**

✅ Yin can create named profiles for each streaming scenario.
✅ Each profile contains its own input, output groups, and stream targets.
✅ Profiles can be duplicated and adjusted without affecting others.

---

### 1.2 Load a Profile on a Shared Streaming PC

**As Yang**,
**I want to load my own profile when I sit down at the shared streaming computer**,
**So that I don’t accidentally stream using someone else’s settings or destinations.**

✅ Profiles are clearly named and listed.
✅ Only the selected profile is active.
✅ Stream keys and destinations are isolated per profile.

---

### 1.3 Remember the Last Used Profile

**As Yuki**,
**I want MagillaStream to remember the last profile I used**,
**So that I can start streaming faster when I return later.**

✅ The last-used profile is remembered per session.
✅ Encrypted profiles still require a password before loading.
✅ Accidental profile switching is avoided.

---

## 2. RTMP Input (Shared Studio Ingest)

### 2.1 Define a Stable RTMP Ingest Endpoint

**As Yuma**,
**I want the studio streaming computer to always listen on the same RTMP address and port**,
**So that OBS and other encoders never need to be reconfigured.**

✅ The RTMP input is defined once per profile.
✅ The ingest address and port are clearly visible.
✅ External encoders can reliably push to MagillaStream.

---

### 2.2 Avoid Input Conflicts in a Shared Space

**As Yennifur**,
**I want MagillaStream to prevent conflicting input configurations**,
**So that two people don’t accidentally try to use different ingest ports on the same machine.**

✅ Each profile declares its RTMP input explicitly.
✅ Validation prevents invalid or conflicting input definitions.
✅ The studio setup remains predictable.

---

## 3. Output Groups (Encoding Reuse)

### 3.1 Encode Once, Stream Everywhere

**As Yulia**,
**I want to encode my stream once and send it to multiple platforms**,
**So that the shared computer is not overloaded and quality remains consistent.**

✅ Output groups encode the input stream a single time.
✅ Multiple stream targets reuse the same encoded output.
✅ CPU and GPU usage are minimized.

---

### 3.2 Maintain Consistent Quality Across Platforms

**As Yin**,
**I want all platforms to receive the same resolution, bitrate, and framerate**,
**So that my stream quality is predictable everywhere.**

✅ Encoding settings live in one place (the output group).
✅ Platform-specific quirks do not require re-encoding.
✅ Changes are made once and apply everywhere.

---

### 3.3 Support Multiple Output Formats

**As Yang**,
**I want separate output groups for different use cases**,
**So that I can have one high-quality stream for live platforms and another for recording later.**

✅ A profile may define multiple output groups.
✅ Each output group has its own encoding settings.
✅ Future targets (recording, relays) can be added without redesign.

---

## 4. Stream Targets (Destinations)

### 4.1 Stream to a Single Platform Easily

**As Yuki**,
**I want to stream to just one platform on some days**,
**So that I don’t have to manage unnecessary destinations.**

✅ Profiles can include a single stream target.
✅ No special configuration is required for “single-platform” streaming.
✅ The system behaves the same regardless of target count.

---

### 4.2 Stream to Many Platforms Simultaneously

**As Yuma**,
**I want to stream to Twitch, YouTube, and other platforms at the same time**,
**So that I can reach different audiences without extra effort.**

✅ Multiple stream targets can be attached to one output group.
✅ Each target has its own URL and stream key.
✅ All targets go live simultaneously.

---

### 4.3 Keep Credentials Secure on a Shared Machine

**As Yennifur**,
**I want my stream keys protected even though we share a computer**,
**So that no one accidentally exposes or uses my credentials.**

✅ Profiles may be encrypted with a password.
✅ Stream keys are never logged in plain text.
✅ Each streamer controls access to their own profiles.

---

## 5. Profile Management and Safety

### 5.1 Duplicate a Profile for Experimentation

**As Yulia**,
**I want to duplicate an existing profile**,
**So that I can experiment with changes without breaking a working setup.**

✅ Profiles can be duplicated with a new name and ID.
✅ Changes to the copy do not affect the original.
✅ Experimentation is low-risk.

---

### 5.2 Be Warned About Unsaved Changes

**As Yin**,
**I want to be warned if I try to close MagillaStream with unsaved changes**,
**So that I don’t lose configuration updates before going live.**

✅ Unsaved changes are tracked.
✅ The user can save, discard, or cancel on exit.
✅ Accidental loss of work is prevented.

---

### 5.3 Remove Old or Unused Profiles

**As Yang**,
**I want to delete profiles I no longer use**,
**So that the profile list stays clean and easy to navigate.**

✅ Profiles can be deleted with confirmation.
✅ Deleting a profile does not affect others.
✅ The shared system remains organized.

---

## 6. Overall Value

### 6.1 Reduce Setup Time in a Shared Environment

**As the streamer house**,
**We want to switch between streamers and platforms quickly**,
**So that studio time is spent streaming, not reconfiguring software.**

✅ Profiles encapsulate all required settings.
✅ Switching users is fast and safe.
✅ Errors caused by manual reconfiguration are eliminated.

---

### 6.2 Enable Growth Without Relearning the System

**As all streamers**,
**We want a system that scales from one platform to many**,
**So that our workflows don’t need to change as our audiences grow.**

✅ Single- and multi-platform streaming use the same concepts.
✅ Complexity grows linearly, not exponentially.
✅ MagillaStream remains understandable even as usage expands.
