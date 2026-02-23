# SpiritStream Unification Plan

## Overview

Unify fragmented capture pipelines, state management, and repeated inline patterns into modular, maintainable architecture. Informed by OBS Studio (dual refcount lifecycle), Cap (Rust trait abstraction), and Sunshine (buffer pool pattern).

---

## Phase 1: Rust — Shared Capture Infrastructure Module

**Goal**: Extract duplicated patterns from 4 capture services into `server/src/services/capture_core/`.

### 1A. Create `capture_core` module

**New file**: `server/src/services/capture_core/mod.rs`

```rust
pub mod session;       // Generic session management
pub mod lifecycle;     // OBS-style dual refcount transitions
pub mod timeout;       // Async spawn_blocking with timeout helper
pub mod frame;         // Unified CaptureFrame type
```

### 1B. Extract `CaptureSessionManager<T>` — `capture_core/session.rs`

Replaces the identical `Mutex<HashMap<String, Active*>>` + `AtomicBool` + broadcast pattern duplicated in all 4 services.

**What moves here** (currently duplicated ~4x):
- `server/src/services/screen_capture.rs` lines 150-161 (ActiveCapture struct)
- `server/src/services/camera_capture.rs` lines 59-72 (ActiveCapture struct)
- `server/src/services/audio_capture.rs` lines 65-84 (ActiveStream struct)
- `server/src/services/h264_capture.rs` lines 100-166 (H264CaptureSession struct)

**Also consolidates these methods** (currently copy-pasted ~4x):
- `stop_capture()` — screen:440, camera:386, audio:315, h264:524
- `stop_all()` — screen:453, camera:406, audio:331, h264:544
- `is_capturing()` — screen:463, camera:423, audio:344, h264:518
- `active_count()` — screen:469, camera:429, audio:353, h264:567

```rust
/// Generic capture session with stop flag and broadcast channel
pub struct CaptureSession<T: Send + 'static> {
    pub stop_flag: Arc<AtomicBool>,
    pub tx: broadcast::Sender<Arc<T>>,
    pub metadata: SessionMetadata,
    _handle: Option<JoinHandle<()>>,
}

pub struct SessionMetadata {
    pub source_id: String,
    pub source_type: &'static str,
    pub started_at: Instant,
}

/// Manages active capture sessions with O(1) lookup
pub struct CaptureSessionManager<T: Send + 'static> {
    sessions: Mutex<HashMap<String, CaptureSession<T>>>,
}

impl<T: Send + 'static> CaptureSessionManager<T> {
    pub fn new() -> Self { ... }
    pub fn insert(&self, id: String, session: CaptureSession<T>) { ... }
    pub fn stop(&self, id: &str) -> bool { ... }         // Sets flag, removes entry
    pub fn stop_all(&self) { ... }                        // Stops everything
    pub fn is_active(&self, id: &str) -> bool { ... }
    pub fn active_count(&self) -> usize { ... }
    pub fn subscribe(&self, id: &str) -> Option<broadcast::Receiver<Arc<T>>> { ... }
    pub fn active_ids(&self) -> Vec<String> { ... }
}
```

**Integration**: Each capture service replaces its internal HashMap/AtomicBool with `CaptureSessionManager<FrameType>`:
- `ScreenCaptureService` → `CaptureSessionManager<scap::Frame>`
- `CameraCaptureService` → `CaptureSessionManager<VideoFrame>`
- `AudioCaptureService` → `CaptureSessionManager<AudioBuffer>`
- `H264CaptureService` → `CaptureSessionManager<Bytes>`

### 1C. Extract `spawn_blocking_with_timeout()` — `capture_core/timeout.rs`

Replaces repeated pattern in `screen_capture.rs` lines 247-263, 287-303.

```rust
pub async fn spawn_blocking_with_timeout<F, T>(
    label: &str,
    timeout_secs: u64,
    f: F,
) -> Option<T>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{ ... }
```

**Call sites that change**:
- `screen_capture.rs:list_displays_async()` → uses helper
- `screen_capture.rs:list_windows_async()` → uses helper
- `h264_capture.rs` (any spawn_blocking with timeout)

### 1D. Unify frame types — `capture_core/frame.rs`

```rust
/// Common frame metadata shared across all capture types
pub struct FrameInfo {
    pub width: u32,
    pub height: u32,
    pub timestamp: Instant,
    pub format: PixelFormat,
}

pub enum PixelFormat {
    Bgra,
    Nv12,
    Rgb24,
}

/// Wraps platform-specific frames with common accessors
pub enum CaptureFrame {
    Screen(scap::Frame),
    Camera(VideoFrame),
    Encoded(Bytes),  // H.264 MPEG-TS
}

impl CaptureFrame {
    pub fn info(&self) -> FrameInfo { ... }
    pub fn as_bgra(&self) -> Option<&[u8]> { ... }
}
```

Replaces scattered `get_frame_dimensions()` / `extract_frame_data()` in `frame_processing.rs`.

---

## Phase 2: Rust — Source Lifecycle Wiring

**Goal**: Wire the existing `SourceLifecycleService` as the single entry point for all capture start/stop, implementing OBS's dual refcount model.

### 2A. Extend `SourceLifecycleService` — `server/src/services/source_lifecycle.rs`

Currently has `add_ref()` / `remove_ref()` / `set_visibility()` but is **not called by any route or service**. Wire it as the orchestrator.

**New methods**:
```rust
impl SourceLifecycleService {
    /// Called when a source needs to start capturing (enters any visible context)
    /// Delegates to the appropriate capture service based on source type
    pub async fn activate_source(
        &self,
        source: &Source,
        context: CaptureContext,  // Preview, Program, Multiview
    ) -> Result<(), String> {
        let transition = self.add_ref(source.id(), context.is_visible());
        match transition {
            SourceTransition::Shown => self.start_capture(source).await?,
            SourceTransition::Activated => { /* already capturing, just bump refcount */ }
            _ => {}
        }
        Ok(())
    }

    /// Called when a source leaves a context
    pub async fn deactivate_source(
        &self,
        source_id: &str,
        context: CaptureContext,
    ) -> Result<(), String> {
        let transition = self.remove_ref(source_id, context.is_visible());
        match transition {
            SourceTransition::Hidden => self.stop_capture(source_id).await?,
            SourceTransition::Deactivated => { /* still visible elsewhere */ }
            _ => {}
        }
        Ok(())
    }
}
```

**Holds references to all capture services** (injected at construction):
```rust
pub struct SourceLifecycleService {
    screen_capture: Arc<ScreenCaptureService>,
    camera_capture: Arc<CameraCaptureService>,
    audio_capture: Arc<AudioCaptureService>,
    h264_capture: Arc<H264CaptureService>,
    // ... existing refcount state
}
```

### 2B. Update route handlers — `server/src/routes/capture.rs`

**Before** (direct service calls):
```rust
// POST /api/capture/screen/start
state.screen_capture.start_display_capture(source_id, display_id, ...)
```

**After** (lifecycle-mediated):
```rust
// POST /api/capture/screen/start
state.source_lifecycle.activate_source(&source, CaptureContext::Preview)
```

**Routes that change**:
- `POST /api/capture/camera/start` → `source_lifecycle.activate_source()`
- `POST /api/capture/camera/stop` → `source_lifecycle.deactivate_source()`
- `POST /api/capture/screen/start` → `source_lifecycle.activate_source()`
- `POST /api/capture/screen/stop` → `source_lifecycle.deactivate_source()`
- `POST /api/capture/audio/start` → `source_lifecycle.activate_source()`
- `POST /api/capture/audio/stop` → `source_lifecycle.deactivate_source()`

### 2C. Update WebRTC route — `server/src/routes/webrtc.rs`

**Before** (lines 39-257, type-specific branching):
```rust
match &source {
    Source::ScreenCapture(sc) => {
        h264_capture.start_capture_http(...)
        go2rtc_manager.register_source(...)
    }
    Source::Camera(cam) => {
        go2rtc_manager.register_source(ffmpeg_device_url)
    }
}
```

**After**:
```rust
// Lifecycle handles the capture start
source_lifecycle.activate_source(&source, CaptureContext::Program).await?;
// Registration logic can stay in webrtc route (it's WebRTC-specific)
go2rtc_manager.register_source(source.id(), get_stream_url(&source)).await?;
```

### 2D. Update `main.rs` AppState construction

Add capture service references to `SourceLifecycleService::new()`:
```rust
let source_lifecycle = Arc::new(SourceLifecycleService::new(
    screen_capture.clone(),
    camera_capture.clone(),
    audio_capture.clone(),
    h264_capture.clone(),
));
```

---

## Phase 3: Rust — Source Model Cleanup

**Goal**: Eliminate 14-arm match statements via macro + metadata.

### 3A. Create `source_base` macro — `server/src/models/source_macros.rs`

```rust
/// Generates common accessor methods for Source enum variants
macro_rules! source_accessor {
    ($method:ident, $field:ident, $ret:ty) => {
        pub fn $method(&self) -> $ret {
            match self {
                $(Source::$variant(s) => &s.$field,)*
            }
        }
    };
}
```

Or alternatively, a `SourceBase` trait that each variant's inner struct implements:

```rust
pub trait SourceBase {
    fn id(&self) -> &str;
    fn name(&self) -> &str;
}
```

Then a blanket impl on Source:
```rust
impl Source {
    fn base(&self) -> &dyn SourceBase { ... } // single match
    pub fn id(&self) -> &str { self.base().id() }
    pub fn name(&self) -> &str { self.base().name() }
}
```

### 3B. Add `SourceCapabilities` metadata — `server/src/models/source.rs`

Replace `has_video()` / `has_audio()` match arms with declarative metadata:

```rust
pub struct SourceCapabilities {
    pub has_video: bool,
    pub has_audio: bool,
    pub needs_device: bool,
    pub capture_service: CaptureBackend,
}

pub enum CaptureBackend {
    Screen,
    Camera,
    Audio,
    FFmpeg,       // media files, RTMP
    ClientSide,   // color, text, browser — rendered in frontend
    None,         // nested scene
}

impl Source {
    pub fn capabilities(&self) -> SourceCapabilities { ... }
}
```

This replaces the per-method match arms and makes it trivial to add new source types.

---

## Phase 4: TypeScript — Source Metadata Registry

**Goal**: Replace 14 switch/case blocks and 14 factory functions with a single metadata-driven registry.

### 4A. Create `sourceRegistry.ts` — `apps/web/src/lib/sourceRegistry.ts`

**What moves here** (currently inline in `types/source.ts`):
- `createDefaultRtmpSource()` and 13 siblings (lines 402-617)
- `getSourceTypeLabel()` switch (lines 622-653)
- `getSourceTypeIcon()` switch (lines 658-689)
- `sourceHasVideo()` switch (lines 340-361)
- `sourceHasAudio()` switch (lines 367-397)

**New structure**:
```typescript
// Single source of truth for all source type metadata
interface SourceTypeEntry {
  type: SourceType;
  label: string;
  icon: string;                          // Lucide icon name
  category: 'capture' | 'media' | 'overlay' | 'composition';
  hasVideo: boolean | ((s: Source) => boolean);
  hasAudio: boolean | ((s: Source) => boolean);
  needsDevice: boolean;                  // Requires device discovery
  defaults: () => Partial<Source>;       // Factory defaults
  formComponent: () => Promise<ComponentType>;  // Lazy-loaded form
}

export const SOURCE_REGISTRY: Record<SourceType, SourceTypeEntry> = {
  rtmp: {
    type: 'rtmp',
    label: 'RTMP Input',
    icon: 'Radio',
    category: 'capture',
    hasVideo: true,
    hasAudio: (s) => (s as RtmpSource).captureAudio,
    needsDevice: false,
    defaults: () => ({ bindAddress: '0.0.0.0', port: 1935, application: 'live', captureAudio: true }),
    formComponent: () => import('../components/modals/source-forms/RtmpSourceForm'),
  },
  camera: { ... },
  screenCapture: { ... },
  // ... all 14 types
};

// Derived helpers (replace switch statements)
export function getSourceTypeLabel(type: SourceType): string {
  return SOURCE_REGISTRY[type].label;
}

export function getSourceTypeIcon(type: SourceType): string {
  return SOURCE_REGISTRY[type].icon;
}

export function sourceHasVideo(source: Source): boolean {
  const entry = SOURCE_REGISTRY[source.type];
  return typeof entry.hasVideo === 'function' ? entry.hasVideo(source) : entry.hasVideo;
}

export function sourceHasAudio(source: Source): boolean {
  const entry = SOURCE_REGISTRY[source.type];
  return typeof entry.hasAudio === 'function' ? entry.hasAudio(source) : entry.hasAudio;
}

export function createDefaultSource(type: SourceType, overrides?: Partial<Source>): Source {
  const entry = SOURCE_REGISTRY[type];
  return {
    type,
    id: crypto.randomUUID(),
    name: entry.label,
    ...entry.defaults(),
    ...overrides,
  } as Source;
}
```

### 4B. Simplify `AddSourceModal.tsx`

**Before** (lines 229-304, massive switch):
```typescript
switch (type) {
  case 'screenCapture': { ... 8 lines ... }
  case 'windowCapture': { ... 6 lines ... }
  case 'camera': { ... 4 lines ... }
  // ... 11 more cases
}
```

**After**:
```typescript
const entry = SOURCE_REGISTRY[type];
if (entry.needsDevice) await discoverDevices();
setFormData(createDefaultSource(type));
```

Form rendering replaces the switch with:
```typescript
const FormComponent = await SOURCE_REGISTRY[formData.type].formComponent();
return <FormComponent data={formData} onChange={setFormData} devices={devices} />;
```

### 4C. Extract filter factory registry — `apps/web/src/lib/filterRegistry.ts`

**What moves here** (currently inline in `types/source.ts`):
- 5 audio filter factories (lines 788-846)
- 9 video filter factories (lines 1015-1124)

```typescript
interface FilterEntry<T> {
  type: string;
  label: string;
  defaults: () => T;
}

export const AUDIO_FILTER_REGISTRY: Record<string, FilterEntry<AudioFilter>> = {
  compressor: { type: 'compressor', label: 'Compressor', defaults: () => ({ ... }) },
  noiseGate: { ... },
  // ...
};

export const VIDEO_FILTER_REGISTRY: Record<string, FilterEntry<VideoFilter>> = { ... };

export function createAudioFilter(type: string): AudioFilter {
  return { ...AUDIO_FILTER_REGISTRY[type].defaults(), id: crypto.randomUUID() };
}
```

---

## Phase 5: TypeScript — Store Consolidation

**Goal**: Fix cross-store sync bugs, make source CRUD atomic.

### 5A. Merge sourceStore CRUD into profileStore

**Problem**: `sourceStore.addSource()` calls backend but doesn't sync to profileStore. `removeSource()` does sync (via dynamic import line 196). Inconsistent.

**Solution**: Move `addSource`, `updateSource`, `removeSource`, `reorderSources` into `profileStore.ts` as actions that:
1. Call backend API
2. Update `current.sources` in one atomic `set()` call
3. Handle linked audio source cascade (camera → audio device)

**sourceStore keeps**: Device discovery + cache only (renamed to `deviceStore.ts`).

```typescript
// profileStore.ts — new source actions
addSource: async (profileName, source) => {
  const updatedSources = await api.source.add(profileName, source);
  set((state) => ({
    current: state.current ? { ...state.current, sources: updatedSources } : state.current,
  }));
  return updatedSources;
},

removeSource: async (profileName, sourceId, password?) => {
  const result = await api.source.remove(profileName, sourceId, password);
  set((state) => {
    if (!state.current) return state;
    const removedIds = new Set([sourceId, ...(result.linkedRemoved || [])]);
    return {
      current: {
        ...state.current,
        sources: state.current.sources.filter(s => !removedIds.has(s.id)),
      },
    };
  });
},
```

### 5B. Fix studioStore direct mutations

**Before** (`studioStore.ts` line 206):
```typescript
useProfileStore.getState().setCurrentActiveScene(previewSceneId);
```

**After**: Use callback pattern:
```typescript
// studioStore receives profileStore actions via parameter
executeTake: async () => {
  // ... transition logic ...
  const { setCurrentActiveScene } = useProfileStore.getState();
  setCurrentActiveScene(previewSceneId);  // Still calls profileStore, but via documented API
}
```

This is already the pattern used — the issue is that `setCurrentActiveScene` is called as a side effect. Document this as intentional cross-store orchestration in a comment.

### 5C. Rename sourceStore → deviceStore

After moving CRUD to profileStore:

**`apps/web/src/stores/deviceStore.ts`** (was sourceStore.ts):
```typescript
interface DeviceState {
  devices: {
    cameras: CameraDevice[];
    displays: DisplayInfo[];
    windows: WindowInfo[];
    audioDevices: AudioInputDevice[];
    captureCards: CaptureCardDevice[];
    isDiscovering: boolean;
  };
  discoverDevices: () => Promise<void>;
  listCameras: () => Promise<CameraDevice[]>;
  listDisplays: () => Promise<DisplayInfo[]>;
  // ... device-only methods
}
```

### 5D. Create `useSourceLookup` hook — `apps/web/src/hooks/useSourceLookup.ts`

Replaces repeated `useMemo(() => new Map(...))` pattern:

```typescript
export function useSourceLookup(): Map<string, Source> {
  const sources = useProfileStore((s) => s.current?.sources ?? []);
  return useMemo(() => new Map(sources.map(s => [s.id, s])), [sources]);
}
```

Used by: `SourcesPanel.tsx`, `AudioMixerPanel.tsx`, `SortableLayerItem.tsx`, any component needing source-by-id lookup.

---

## Phase 6: Audio Track Creation Consolidation

**Goal**: Single code path for audio track creation (currently in 3 places).

### 6A. Create `useSourceActions` hook — `apps/web/src/hooks/useSourceActions.ts`

Consolidates the scattered add-source-with-audio-track-and-layer logic from `AddSourceModal.tsx` (lines 361-396) and `SourcesPanel.tsx` (lines 354-389).

```typescript
export function useSourceActions() {
  const addSource = useProfileStore(s => s.addSource);
  const addLayer = useSceneStore(s => s.addLayer);
  // ...

  const addSourceToScene = useCallback(async (
    profileName: string,
    source: Source,
    sceneId: string,
  ) => {
    // 1. Add source to profile (atomic)
    await addSource(profileName, source);

    // 2. Handle linked audio (cameras)
    if (source.type === 'camera' && source.captureAudio && source.linkedAudioDeviceId) {
      const linkedAudio = createDefaultSource('audioDevice', {
        name: `${source.name} Audio`,
        deviceId: source.linkedAudioDeviceId,
        linkedToSourceId: source.id,
      });
      await addSource(profileName, linkedAudio);
    }

    // 3. Add layer to scene (backend returns authoritative audio tracks)
    const result = await addLayer(profileName, sceneId, source.id);

    // 4. Sync audio tracks from backend (single source of truth)
    if (result.audioTracks) {
      setCurrentAudioTracks(sceneId, result.audioTracks);
    }

    return result;
  }, [addSource, addLayer]);

  return { addSourceToScene };
}
```

**AddSourceModal** and **SourcesPanel** both call `addSourceToScene()` instead of duplicating the logic.

---

## Phase 7: Preview Service Consolidation (Future)

**Goal**: Merge 3 preview services into one with strategy pattern. This is the largest change and can follow after Phases 1-6 stabilize.

### 7A. Unified preview dispatcher

```rust
pub struct PreviewService {
    native: Arc<NativePreviewService>,    // JPEG from raw frames
    ffmpeg: Arc<PreviewHandler>,          // MJPEG from FFmpeg
    compositor: Arc<Compositor>,          // Scene composition
}

impl PreviewService {
    pub async fn start_preview(&self, source: &Source) -> Result<PreviewHandle> {
        match source.capabilities().capture_service {
            CaptureBackend::Screen | CaptureBackend::Camera => {
                // Use native JPEG (turbojpeg, fast)
                self.native.start_preview(source)
            }
            CaptureBackend::FFmpeg => {
                // Use FFmpeg MJPEG (media files, RTMP)
                self.ffmpeg.start_source_preview(source)
            }
            CaptureBackend::ClientSide => {
                // No backend preview needed (color, text, browser render in frontend)
                Ok(PreviewHandle::ClientSide)
            }
        }
    }
}
```

### 7B. Expose NativePreviewService via HTTP

Currently `NativePreviewService` is internal-only. Add route:
```rust
GET /api/preview/native/:source_id → native_preview.subscribe()
```

This gives the frontend access to the faster turbojpeg path for cameras/screens instead of routing through FFmpeg MJPEG.

---

## File Change Summary

### New files
| File | Purpose |
|------|---------|
| `server/src/services/capture_core/mod.rs` | Module root |
| `server/src/services/capture_core/session.rs` | `CaptureSessionManager<T>` |
| `server/src/services/capture_core/timeout.rs` | `spawn_blocking_with_timeout()` |
| `server/src/services/capture_core/frame.rs` | `CaptureFrame` + `FrameInfo` |
| `server/src/models/source_macros.rs` | Source accessor macro |
| `apps/web/src/lib/sourceRegistry.ts` | Source type metadata registry |
| `apps/web/src/lib/filterRegistry.ts` | Audio/video filter factories |
| `apps/web/src/hooks/useSourceLookup.ts` | Shared source map hook |
| `apps/web/src/hooks/useSourceActions.ts` | Consolidated add-source-with-audio logic |

### Modified files
| File | Change |
|------|--------|
| `server/src/services/screen_capture.rs` | Use `CaptureSessionManager<Frame>`, remove duplicated methods |
| `server/src/services/camera_capture.rs` | Use `CaptureSessionManager<VideoFrame>`, remove duplicated methods |
| `server/src/services/audio_capture.rs` | Use `CaptureSessionManager<AudioBuffer>`, remove duplicated methods |
| `server/src/services/h264_capture.rs` | Use `CaptureSessionManager<Bytes>`, remove duplicated methods |
| `server/src/services/source_lifecycle.rs` | Add capture service refs, `activate_source()`/`deactivate_source()` |
| `server/src/services/mod.rs` | Add `pub mod capture_core;` |
| `server/src/models/source.rs` | Use macro/trait for accessors, add `capabilities()` |
| `server/src/models/mod.rs` | Add `pub mod source_macros;` |
| `server/src/routes/capture.rs` | Route through `source_lifecycle` |
| `server/src/routes/webrtc.rs` | Route through `source_lifecycle` |
| `server/src/main.rs` | Updated `SourceLifecycleService::new()` with capture service refs |
| `apps/web/src/types/source.ts` | Remove factory fns, switch blocks → re-export from registry |
| `apps/web/src/stores/profileStore.ts` | Add source CRUD actions |
| `apps/web/src/stores/sourceStore.ts` | Rename to `deviceStore.ts`, remove CRUD, keep device discovery |
| `apps/web/src/components/modals/AddSourceModal.tsx` | Use `SOURCE_REGISTRY`, remove switch blocks |
| `apps/web/src/components/stream/SourcesPanel.tsx` | Use `useSourceActions`, `useSourceLookup` |

### Deleted code (moved to modules)
| Location | What | Moved To |
|----------|------|----------|
| screen_capture.rs:440-472 | stop/stopAll/isCapturing/activeCount | `capture_core/session.rs` |
| camera_capture.rs:386-432 | stop/stopAll/isCapturing/activeCount | `capture_core/session.rs` |
| audio_capture.rs:315-358 | stop/stopAll/isCapturing/activeCount | `capture_core/session.rs` |
| h264_capture.rs:518-573 | stop/stopAll/isCapturing/activeCount | `capture_core/session.rs` |
| source.ts:340-689 | 6 switch statements (14 arms each) | `sourceRegistry.ts` |
| source.ts:402-617 | 14 factory functions | `sourceRegistry.ts` |
| source.ts:788-846 | 5 audio filter factories | `filterRegistry.ts` |
| source.ts:1015-1124 | 9 video filter factories | `filterRegistry.ts` |

---

## Implementation Order

```
Phase 1 (capture_core module)     ← No breaking changes, additive only
  ↓
Phase 3 (source model macros)     ← Internal refactor, no API change
  ↓
Phase 4 (source registry TS)      ← Internal refactor, no API change
  ↓
Phase 5 (store consolidation)     ← Fixes sync bugs, renames sourceStore
  ↓
Phase 6 (audio track consolidation) ← Fixes duplication bugs
  ↓
Phase 2 (lifecycle wiring)        ← Routes change, needs integration testing
  ↓
Phase 7 (preview consolidation)   ← Largest change, deferred until stable
```

Phases 1, 3, 4 can run in parallel (Rust vs TypeScript, no overlap).
Phase 5 depends on Phase 4 (registry must exist before store uses it).
Phase 2 depends on Phase 1 (lifecycle uses capture_core).
Phase 7 deferred — builds on all prior phases.
