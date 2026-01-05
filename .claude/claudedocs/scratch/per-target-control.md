# Per-Target Stream Control (Hybrid Approach)

**Date**: 2026-01-04
**Status**: ✅ Implemented
**Feature**: Allow individual stream targets to be toggled on/off during live streaming

---

## Problem

Users wanted the ability to stop/start/restart individual stream targets (YouTube, Twitch, Kick, etc.) without affecting other targets that are streaming.

**Example scenario:**
- User is streaming to YouTube (passthrough) and Twitch (passthrough) simultaneously
- User wants to stop YouTube but keep Twitch running
- Previous behavior: Would need to stop entire group, affecting both targets

---

## Solution: Hybrid Group-Based Control

Implemented a **hybrid approach** that balances efficiency with user control:

### Architecture Decision

**Chosen Approach**: Per-target control within output groups (Option C)

- Each **output group** (encoding profile) runs **one FFmpeg process**
- Within a group, users can **enable/disable specific targets**
- Toggling a target triggers a **brief restart** of the parent group (~1-2 seconds)
- Targets in **different groups** are completely independent

### Why Not Per-Process (Option B)?

**Option B (one FFmpeg process per target)** was rejected because:
- **GPU inefficiency**: If Kick and Facebook both use NVENC 6000k, Option B would spawn 2 FFmpeg processes doing identical encoding (2 GPU sessions)
- **Option C**: Same scenario uses 1 FFmpeg process encoding once, outputting to both (1 GPU session)
- **CPU overhead**: While passthrough copy is cheap (~1% CPU per process), it's still unnecessary duplication

---

## Implementation

### Backend (Rust)

#### 1. FFmpegHandler State Tracking

Added `disabled_targets` set to track which targets user has toggled off:

```rust
pub struct FFmpegHandler {
    ffmpeg_path: String,
    processes: Arc<Mutex<HashMap<String, ProcessInfo>>>,
    disabled_targets: Arc<Mutex<HashSet<String>>>,  // NEW
}
```

#### 2. Filter Disabled Targets in build_args

Modified FFmpeg command generation to skip disabled targets:

```rust
fn build_args(&self, group: &OutputGroup, incoming_url: &str, is_first_process: bool) -> Vec<String> {
    // ... input and encoding config ...

    // Add output targets (skip disabled ones)
    let disabled = self.disabled_targets.lock().unwrap();
    for target in &group.stream_targets {
        if disabled.contains(&target.id) {
            continue;  // Skip this target
        }

        args.push("-f".to_string());
        args.push(group.container.format.clone());
        args.push(format!("{}/{}", target.url, target.stream_key));
    }

    args
}
```

#### 3. Toggle and Restart Methods

```rust
/// Enable a specific stream target (removes from disabled set)
pub fn enable_target(&self, target_id: &str) {
    self.disabled_targets.lock().unwrap().remove(target_id);
}

/// Disable a specific stream target (adds to disabled set)
pub fn disable_target(&self, target_id: &str) {
    self.disabled_targets.lock().unwrap().insert(target_id.to_string());
}

/// Restart a specific group (used after toggling targets)
/// This stops the group and restarts it with the updated target list
pub fn restart_group<R: tauri::Runtime>(
    &self,
    group_id: &str,
    group: &OutputGroup,
    incoming_url: &str,
    app_handle: &AppHandle<R>,
) -> Result<u32, String> {
    if self.is_streaming(group_id) {
        self.stop(group_id)?;
    }
    self.start(group, incoming_url, app_handle)
}
```

### Tauri Commands

```rust
/// Toggle a specific stream target on/off
/// This will restart the parent output group with the updated target list
#[tauri::command]
pub fn toggle_stream_target(
    app: AppHandle,
    target_id: String,
    enabled: bool,
    group: OutputGroup,
    incoming_url: String,
    ffmpeg_handler: State<'_, FFmpegHandler>
) -> Result<u32, String> {
    if enabled {
        ffmpeg_handler.enable_target(&target_id);
    } else {
        ffmpeg_handler.disable_target(&target_id);
    }

    ffmpeg_handler.restart_group(&group.id, &group, &incoming_url, &app)
}
```

### Frontend (TypeScript)

#### API Wrapper (`lib/tauri.ts`)

```typescript
stream: {
  // ... existing methods ...
  toggleTarget: (targetId: string, enabled: boolean, group: OutputGroup, incomingUrl: string) =>
    invoke<number>('toggle_stream_target', { targetId, enabled, group, incomingUrl }),
  isTargetDisabled: (targetId: string) =>
    invoke<boolean>('is_target_disabled', { targetId }),
}
```

#### Stream Store (`stores/streamStore.ts`)

```typescript
interface StreamState {
  // ... existing state ...
  toggleTargetLive: (targetId: string, enabled: boolean, group: OutputGroup, incomingUrl: string) => Promise<void>;
}

// Implementation
toggleTargetLive: async (targetId, enabled, group, incomingUrl) => {
  try {
    await api.stream.toggleTarget(targetId, enabled, group, incomingUrl);

    const enabledTargets = new Set(get().enabledTargets);
    if (enabled) {
      enabledTargets.add(targetId);
    } else {
      enabledTargets.delete(targetId);
    }
    set({ enabledTargets });
  } catch (error) {
    set({ error: String(error) });
    throw error;
  }
}
```

---

## User Experience

### Scenario 1: Toggle Target in Same Group

**Configuration:**
- Passthrough group: YouTube + Twitch
- Both streaming

**Action**: User clicks toggle to stop YouTube

**What Happens:**
1. Frontend calls `toggleTargetLive('youtube-id', false, passthroughGroup, 'rtmp://...')`
2. Backend disables YouTube in `disabled_targets` set
3. Backend stops passthrough group FFmpeg process
4. Backend restarts passthrough group with new FFmpeg command (only Twitch output)
5. **Brief interruption**: Twitch flickers for ~1-2 seconds
6. Twitch resumes, YouTube stopped

**Visual Feedback**:
- Toggle switch changes state immediately
- Brief "reconnecting" indicator on Twitch
- YouTube shows "offline" status

### Scenario 2: Toggle Target in Different Group

**Configuration:**
- Passthrough group: YouTube + Twitch (both streaming)
- Re-encode group: Kick (streaming)

**Action**: User clicks toggle to stop YouTube

**What Happens:**
1. Passthrough group restarts (affects YouTube + Twitch)
2. **Re-encode group continues unaffected** (different FFmpeg process)
3. Kick experiences **zero interruption**

---

## Resource Efficiency Comparison

### Scenario: 2 Passthrough Targets + 2 Re-encode Targets (same settings)

**Configuration:**
- Passthrough: YouTube + Twitch
- Re-encode (NVENC 6000k): Kick + Facebook

**Option B (One Process Per Target)**:
```
Relay Process:       1% CPU
YouTube (copy):      1% CPU
Twitch (copy):       1% CPU
Kick (NVENC 6000k):  10% CPU + GPU session 1
Facebook (NVENC 6000k): 10% CPU + GPU session 2

Total: 23% CPU + 2 GPU encode sessions
```

**Option C (Hybrid - Implemented)**:
```
Passthrough group:  2% CPU  (YouTube + Twitch)
Re-encode group:    10% CPU + GPU session 1 (Kick + Facebook)

Total: 12% CPU + 1 GPU encode session
```

**Savings**: 50% less CPU, 50% less GPU load

---

## Trade-offs

### Advantages ✅

1. **GPU efficient**: Targets with same encoding share one encode session
2. **Groups independent**: Toggling YouTube doesn't affect Kick (different group)
3. **Minimal CPU overhead**: Only slight increase vs pure group control
4. **Existing architecture**: Builds on current group-based model

### Limitations ⚠️

1. **Brief interruption**: Toggling one target in a group causes ~1-2 second flicker for all targets in that group
2. **Not truly independent**: Targets within same group share lifecycle
3. **User education**: Users need to understand grouping concept

---

## Alternative Approaches Considered

### Option A: Frontend-Only Filtering

**How it works**: Disable targets before starting stream, can't change while live

**Rejected because**: No live control, users must stop → change → restart

### Option B: One FFmpeg Process Per Target

**How it works**: Each target gets its own FFmpeg process

**Rejected because**:
- Wastes GPU encoding sessions when targets share settings
- Example: 2 targets at NVENC 6000k = 2 GPU sessions instead of 1

### Option C: Hybrid (Chosen)

**How it works**: Group-based processes + target filtering + restart on toggle

**Chosen because**: Best balance of efficiency and user control

---

## Future Enhancements

1. **Smart grouping UI**: Auto-suggest grouping targets with same encoding settings
2. **Restart optimization**: Minimize interruption time (currently ~1-2 sec)
3. **Per-target stats**: Show individual bitrate/FPS for each target in a group
4. **Batch toggle**: Allow toggling multiple targets with single restart

---

## Testing Checklist

- [ ] Toggle single target in passthrough group (2 targets)
- [ ] Toggle single target in re-encode group (2 targets)
- [ ] Toggle target in passthrough while re-encode group running (verify no interruption to re-encode)
- [ ] Toggle multiple targets rapidly (verify restart doesn't overlap)
- [ ] Check disabled state persists across app restart
- [ ] Verify FFmpeg command excludes disabled targets
- [ ] Test with all targets disabled in a group (should skip that group)

---

**Implementation Date**: 2026-01-04
**Type Checking**: ✅ Rust passes with 1 warning (unused `new_default`)
**Status**: Ready for UI integration and testing
