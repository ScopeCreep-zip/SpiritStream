# Immutable Default Passthrough Group Implementation

**Date**: 2026-01-04
**Status**: Completed

## Summary

Implemented an **immutable default passthrough output group** that is automatically included with every profile. This group cannot be edited or deleted, and users must create additional output groups if they want custom encoding settings.

## Motivation

The original implementation allowed users to edit the default output group's encoding settings, which was confusing since the default should just be a simple RTMP relay. By making the default group immutable:

- **Clearer architecture**: Default group is always passthrough (copy mode)
- **Simpler UX**: Users understand that the default is just an RTMP relay
- **Forces intentionality**: Users must explicitly create a new group to customize encoding
- **Prevents misconfiguration**: Can't accidentally change the default to re-encode

## Architecture

### Concept Hierarchy

```
Profile
└── Output Groups
    ├── Default Group (immutable, always present, always "copy" mode)
    │   └── Stream Targets (can be added/removed)
    └── Custom Groups (editable, can be added/removed)
        └── Stream Targets (can be added/removed)
```

### Key Design Decisions

1. **Default group has fixed ID**: `"default"` - makes it easy to identify
2. **Default group always uses copy mode**: `video.codec = "copy"`, `audio.codec = "copy"`
3. **Cannot edit encoding settings**: Modal refuses to open for default group
4. **Cannot delete**: UI hides delete button, store prevents deletion
5. **Can add/remove targets**: Users can still configure where the stream goes
6. **Always created with profile**: New profiles automatically include the default group

## Changes Made

### 1. Backend (Rust)

**File**: `src-tauri/src/models/output_group.rs`

```rust
pub struct OutputGroup {
    pub id: String,
    pub name: String,
    pub is_default: bool,  // NEW: marks the immutable default group
    pub video: VideoSettings,
    pub audio: AudioSettings,
    pub container: ContainerSettings,
    pub stream_targets: Vec<StreamTarget>,
}

impl OutputGroup {
    /// Create new custom output group
    pub fn new() -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name: "New Output Group".to_string(),
            is_default: false,  // Custom groups are not default
            // ...
        }
    }

    /// Create the default passthrough group
    pub fn new_default() -> Self {
        Self {
            id: "default".to_string(),  // Fixed ID
            name: "Passthrough (Default)".to_string(),
            is_default: true,  // Marked as default
            video: VideoSettings::default(),  // codec: "copy"
            audio: AudioSettings::default(),  // codec: "copy"
            // ...
        }
    }
}
```

### 2. Frontend (TypeScript)

**File**: `src-frontend/types/profile.ts`

```typescript
export interface OutputGroup {
  id: string;
  name: string;
  isDefault?: boolean;  // NEW: marks immutable group
  video: VideoSettings;
  audio: AudioSettings;
  container: ContainerSettings;
  streamTargets: StreamTarget[];
}

// Factory for default passthrough group
export const createPassthroughOutputGroup = (): OutputGroup => ({
  id: 'default',
  name: 'Passthrough (Default)',
  isDefault: true,
  video: { codec: 'copy', width: 0, height: 0, fps: 0, bitrate: '0k' },
  audio: { codec: 'copy', bitrate: '0k', channels: 0, sampleRate: 0 },
  container: { format: 'flv' },
  streamTargets: [],
});

// Always include default group when creating profiles
export const createDefaultProfile = (name: string = 'New Profile'): Profile => ({
  id: crypto.randomUUID(),
  name,
  encrypted: false,
  input: createDefaultRtmpInput(),
  outputGroups: [createPassthroughOutputGroup()],  // ALWAYS included
});
```

**File**: `src-frontend/components/modals/OutputGroupModal.tsx`

```typescript
export function OutputGroupModal({ open, onClose, mode, group }: OutputGroupModalProps) {
  // Detect attempt to edit default group
  const isDefaultGroup = mode === 'edit' && group?.isDefault === true;

  // Refuse to open modal for default group
  if (isDefaultGroup && open) {
    setTimeout(() => onClose(), 0);
    return null;
  }

  // ... rest of modal
}
```

**File**: `src-frontend/components/encoder/EncoderCard.tsx`

```typescript
export function EncoderCard({ group, ... }: EncoderCardProps) {
  const isPassthrough = group.video.codec === 'copy' && group.audio.codec === 'copy';
  const isDefaultGroup = group.isDefault === true;

  // Show "Source" instead of actual values for passthrough
  const resolution = isPassthrough ? 'Source' : `${group.video.width}×${group.video.height}`;
  const bitrate = isPassthrough ? 'Source' : group.video.bitrate;
  const fps = isPassthrough ? 'Source' : `${group.video.fps} fps`;

  return (
    <Card>
      {/* ... */}
      <h3>
        {group.name}
        {isDefaultGroup && (
          <span className="text-xs text-tertiary"> (Read-only)</span>
        )}
      </h3>

      {/* Hide edit/duplicate/delete buttons for default group */}
      {!isDefaultGroup && (
        <>
          <Button onClick={onEdit}>Edit</Button>
          <Button onClick={onDuplicate}>Duplicate</Button>
          <Button onClick={onRemove}>Delete</Button>
        </>
      )}

      {isDefaultGroup && (
        <span className="text-xs italic">
          Default RTMP relay - cannot be edited or deleted
        </span>
      )}
    </Card>
  );
}
```

**File**: `src-frontend/stores/profileStore.ts`

```typescript
removeOutputGroup: async (groupId) => {
  const current = get().current;
  if (current) {
    // Prevent deletion of default group
    const groupToDelete = current.outputGroups.find((g) => g.id === groupId);
    if (groupToDelete?.isDefault) {
      console.warn('Cannot delete the default passthrough output group');
      return;  // Silently refuse
    }

    // Remove non-default groups
    set({
      current: {
        ...current,
        outputGroups: current.outputGroups.filter((g) => g.id !== groupId),
      },
    });
    await get().saveProfile();
  }
},
```

## User-Facing Changes

### Before

- Profiles could have zero output groups
- Default output group showed encoding settings (1080p60, 6000k, etc.)
- Users could edit the default group
- Confusing whether to edit default or create new group

### After

- Every profile has at least one output group (the default passthrough)
- Default group shows "Source" for all encoding parameters
- Default group clearly marked as "Read-only"
- Edit/Duplicate/Delete buttons hidden for default group
- If users want custom encoding, they must click "Add Encoder"

### UI Indicators

**EncoderCard for Default Group:**
```
┌─────────────────────────────────────────────────┐
│ 🖥️ Passthrough (Default) (Read-only)      [●] │
│    Passthrough                                   │
│                                                  │
│ ┌────────┬────────┬────────┬────────┐           │
│ │ Source │ Source │ Source │   —    │           │
│ │  Res   │ Bitrat │  FPS   │ Preset │           │
│ └────────┴────────┴────────┴────────┘           │
│                                                  │
│ 🔊 Audio: Source                                 │
│ Default RTMP relay - cannot be edited or deleted│
└─────────────────────────────────────────────────┘
```

**EncoderCard for Custom Group:**
```
┌─────────────────────────────────────────────────┐
│ 🎬 High Quality Stream                    [●]   │
│    NVENC (Hardware)                              │
│                                                  │
│ ┌─────────┬────────┬────────┬─────────┐         │
│ │1920×1080│ 6000k  │ 60 fps │ Balanced│         │
│ │   Res   │ Bitrat │  FPS   │  Preset │         │
│ └─────────┴────────┴────────┴─────────┘         │
│                                                  │
│ 🔊 Audio: AAC @ 160k • Profile: HIGH             │
│ [✏️ Edit] [📋 Copy] [🗑️ Delete]                  │
└─────────────────────────────────────────────────┘
```

## Benefits

1. **Clearer Mental Model**: "Default = passthrough, custom = encoding"
2. **Prevents Accidents**: Can't accidentally break the default relay
3. **Simpler Onboarding**: New users get working passthrough immediately
4. **Forces Intent**: Want encoding? Explicitly create a group for it
5. **Consistent**: Every profile works the same way

## Workflow

### Basic Workflow (Passthrough Only)

1. Create profile → Gets default passthrough group automatically
2. Add stream targets to default group
3. Start streaming → FFmpeg relays to all targets without re-encoding

### Advanced Workflow (Custom Encoding)

1. Create profile → Gets default passthrough group
2. Click "Add Encoder" → Create custom group with encoding settings
3. Add stream targets to custom group(s)
4. Default group can remain for simple relay targets
5. Custom groups used for platforms needing specific encoding

## Future Considerations

1. **Migration**: Existing profiles without `isDefault` flag will need migration
2. **UI Clarity**: Could add tooltip explaining passthrough mode
3. **Profile Templates**: Could offer "Passthrough Only" vs "Encoding" templates
4. **Target Assignment**: Could add UI to easily move targets between groups

## Testing

- ✅ Rust compilation successful
- ✅ TypeScript type checking successful
- ✅ Default group created with new profiles
- ✅ Modal refuses to open for default group
- ✅ Edit/Delete buttons hidden for default group
- ✅ Store prevents deletion of default group
- ✅ EncoderCard shows "Source" for passthrough parameters

## Related Files

- Backend: `src-tauri/src/models/output_group.rs`
- Frontend types: `src-frontend/types/profile.ts`
- Modal: `src-frontend/components/modals/OutputGroupModal.tsx`
- Card: `src-frontend/components/encoder/EncoderCard.tsx`
- Store: `src-frontend/stores/profileStore.ts`
- Previous work: `.claude/claudedocs/scratch/passthrough-mode-changes.md`
