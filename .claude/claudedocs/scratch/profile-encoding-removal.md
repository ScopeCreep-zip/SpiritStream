# Profile Encoding Settings Removal

**Date**: 2026-01-04
**Status**: Completed

## Summary

Removed all encoding-related settings from the Profile configuration UI, clarifying that:
- **Incoming stream encoding is configured in OBS** (or other streaming software)
- **Profile only defines where the RTMP server listens** (IP, port, application)
- **Default output group = passthrough** (relay stream as-is without re-encoding)
- **Custom output groups = re-encoding** (transcode incoming stream to different settings)

## Motivation

User feedback clarified the architecture:
> "kali streams to youtube and the youtube backup ingest, as well as twitch. in obs, kali sets her stream to use a 1080p30 H.264 at 240000K bitrate. This is sent to SpiritStream. Kali uses the default output group for Youtube, so the stream is forwarded as-is directly to Youtube. Kali creates a new output group that uses an encoding of 1080p30 H.264 6000K. Kali sends this re-encoding to twitch and the youtube backup URL."

The key insight: **SpiritStream receives an already-encoded stream from OBS**. It doesn't need to know the incoming encoding settings - those are configured in the external encoder (OBS).

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                     OBS (External Encoder)                      │
│                                                                 │
│  User configures: 1080p30 H.264 @ 240000K                      │
│                          ↓                                      │
│              RTMP stream to SpiritStream                        │
└─────────────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────────────┐
│                      SpiritStream                               │
│                                                                 │
│  Profile: Defines RTMP server listen settings                  │
│  ├─ Bind Address: 0.0.0.0                                       │
│  ├─ Port: 1935                                                  │
│  └─ Application: live                                           │
│                                                                 │
│  Default Output Group (Passthrough):                            │
│  ├─ Codec: copy (no re-encoding)                                │
│  ├─ Forwards stream as-is                                       │
│  └─ Stream Targets: [YouTube]                                   │
│                                                                 │
│  Custom Output Group (Re-encode):                               │
│  ├─ Codec: libx264                                              │
│  ├─ Settings: 1080p30 H.264 @ 6000K                            │
│  ├─ Re-encodes incoming stream                                  │
│  └─ Stream Targets: [Twitch, YouTube Backup]                    │
└─────────────────────────────────────────────────────────────────┘
```

## Changes Made

### 1. ProfileModal - Removed Encoding Settings

**File**: `src-frontend/components/modals/ProfileModal.tsx`

**Removed**:
- Resolution select dropdown
- Frame rate select dropdown
- Video bitrate input
- All related constants (RESOLUTION_OPTIONS, FPS_VALUES)
- All related form fields and validation

**Kept**:
- Profile name
- RTMP input settings (bind address, port, application)
- Password protection (for encryption)

**Added clarification**:
```tsx
<div style={{...}}>
  {tDynamic('modals.profileExplanation', {
    defaultValue: 'Configure your streaming software (OBS, etc.) to send to this RTMP URL.
    Encoding settings are configured in your streaming software, not in the profile.
    Use output groups to re-encode to different settings for different platforms.'
  })}
</div>
```

**Before**:
```typescript
interface FormData {
  name: string;
  bindAddress: string;
  port: string;
  application: string;
  resolution: string;      // ❌ REMOVED
  fps: string;             // ❌ REMOVED
  videoBitrate: string;    // ❌ REMOVED
  usePassword: boolean;
  password: string;
  confirmPassword: string;
}
```

**After**:
```typescript
interface FormData {
  name: string;
  bindAddress: string;
  port: string;
  application: string;
  usePassword: boolean;
  password: string;
  confirmPassword: string;
}
```

**handleSave changes**:
```typescript
// BEFORE - Created output group with encoding settings
const outputGroup = createDefaultOutputGroup();
outputGroup.video = {
  codec: 'libx264',
  width,
  height,
  fps: parseInt(formData.fps),
  bitrate: `${formData.videoBitrate}k`,
  preset: 'veryfast',
  profile: 'high',
};

// AFTER - Uses default passthrough group from factory
const newProfile = createDefaultProfile(formData.name);
newProfile.input = input;
// createDefaultProfile already includes passthrough output group
```

### 2. EncoderSettings View - Protected Default Group

**File**: `src-frontend/views/EncoderSettings.tsx`

**Changes**:
- Conditionally provide `onEdit` callback only for non-default groups
- Ensure duplicates are never marked as default

```typescript
// Before
<EncoderCard
  onEdit={() => openEditModal(group)}
  onDuplicate={() => duplicateGroup(group)}
/>

// After
<EncoderCard
  onEdit={group.isDefault ? undefined : () => openEditModal(group)}
  onDuplicate={() => duplicateGroup(group)}
/>

// Duplicate function
const duplicateGroup = (group: OutputGroup) => {
  const newGroup: OutputGroup = {
    ...group,
    id: crypto.randomUUID(),
    name: `${group.name} (Copy)`,
    isDefault: false, // ✅ Duplicates are never default
    // ...
  };
};
```

### 3. OutputGroups View - Protected Default Group

**File**: `src-frontend/views/OutputGroups.tsx`

**Changes**:
- Conditionally provide `onEdit` and `onDuplicate` only for non-default groups
- Ensure duplicates are never marked as default

```typescript
<OutputGroupCard
  onEdit={group.isDefault ? undefined : () => openEditModal(group)}
  onDuplicate={group.isDefault ? undefined : () => duplicateGroup(group)}
/>

const duplicateGroup = (groupId: string) => {
  const newGroup = {
    ...group,
    isDefault: false, // ✅ Duplicates are never default
  };
};
```

### 4. EncoderCard - Made onEdit Optional

**File**: `src-frontend/components/encoder/EncoderCard.tsx`

**Changes**:
```typescript
// Before
export interface EncoderCardProps {
  onEdit: () => void;
}

// After
export interface EncoderCardProps {
  onEdit?: () => void;  // ✅ Now optional
}

// Button rendering
{!isDefaultGroup && onEdit && (  // ✅ Check both conditions
  <>
    <Button onClick={onEdit}>
      <Pencil />
    </Button>
    {/* ... */}
  </>
)}
```

### 5. OutputGroupModal - Added Clarification

**File**: `src-frontend/components/modals/OutputGroupModal.tsx`

**Added explanation** when creating custom output groups:
```tsx
{mode === 'create' && (
  <div style={{...}}>
    {tDynamic('modals.outputGroupExplanation', {
      defaultValue: 'Custom output groups re-encode your incoming stream to
      different settings. Use these when you need to send different quality
      streams to different platforms. The default passthrough group relays
      your stream as-is without re-encoding.'
    })}
  </div>
)}
```

## User-Facing Changes

### Profile Modal

**Before**:
- User could set resolution, FPS, and bitrate in profile
- Confusing: Are these for incoming or outgoing?
- Created output group with these settings

**After**:
- Profile only asks for:
  - Name
  - RTMP server settings (where to listen)
  - Password (optional encryption)
- Clear message: "Configure your streaming software (OBS, etc.) to send to this RTMP URL"
- Default passthrough group created automatically

### Output Group Modal

**Before**:
- Just showed encoding settings
- No explanation of purpose

**After**:
- Shows encoding settings (unchanged)
- Explains: "Custom output groups re-encode your incoming stream to different settings"
- Makes it clear this is for transcoding

### EncoderCard Display

**Before**:
- Default group showed edit/duplicate/delete buttons
- Default group showed specific encoding settings

**After**:
- Default group shows: "Default RTMP relay - cannot be edited or deleted"
- Edit/Duplicate/Delete buttons hidden for default group
- Default group shows "Source" for all encoding parameters

## Workflow Example

### User: Kali

**Setup in OBS**:
- Resolution: 1080p30
- Codec: H.264
- Bitrate: 240000K
- Stream to: `rtmp://localhost:1935/live`

**Setup in SpiritStream**:

1. **Create Profile**:
   - Name: "Multi-Platform Stream"
   - RTMP Settings: 0.0.0.0:1935/live
   - Default passthrough group created automatically

2. **Add YouTube to Default Group**:
   - Target: YouTube Gaming
   - Uses passthrough (240000K bitrate, same as OBS)

3. **Create Custom Output Group**:
   - Name: "Lower Bitrate"
   - Video: 1080p30 H.264 @ 6000K
   - Re-encodes incoming 240000K → 6000K

4. **Add Targets to Custom Group**:
   - Twitch
   - YouTube Backup

**Result**:
- YouTube Gaming gets original quality (240000K)
- Twitch and YouTube Backup get re-encoded version (6000K)
- SpiritStream acts as both relay and transcoder

## Technical Benefits

1. **Clearer Separation of Concerns**:
   - External encoder (OBS) = incoming stream settings
   - Profile = RTMP server configuration
   - Output groups = outgoing stream settings (relay or transcode)

2. **Simplified Profile Creation**:
   - Fewer fields to fill out
   - Less confusion about what settings mean
   - Default passthrough always available

3. **Better UX**:
   - Explicit explanations at each step
   - Can't accidentally modify default passthrough
   - Clear distinction: default=relay, custom=re-encode

4. **Consistent Architecture**:
   - Matches FFmpeg's behavior (copy vs transcode)
   - Aligns with how streaming actually works
   - Reduces mental overhead for users

## Files Modified

### Frontend (TypeScript/React)
1. `src-frontend/components/modals/ProfileModal.tsx`
   - Removed encoding settings UI
   - Removed encoding-related form fields
   - Added clarification text
   - Simplified handleSave logic

2. `src-frontend/components/modals/OutputGroupModal.tsx`
   - Added clarification text for custom groups

3. `src-frontend/components/encoder/EncoderCard.tsx`
   - Made `onEdit` prop optional
   - Added check for `onEdit` existence

4. `src-frontend/views/EncoderSettings.tsx`
   - Conditionally provide `onEdit` for non-default groups
   - Prevent duplicates from being marked as default

5. `src-frontend/views/OutputGroups.tsx`
   - Conditionally provide `onEdit` and `onDuplicate` for non-default groups
   - Prevent duplicates from being marked as default

### Backend (Rust)
- No backend changes in this phase
- Existing `OutputGroup::new_default()` factory ready for future use

## Testing

- ✅ TypeScript type checking passes
- ✅ Rust compilation successful (1 warning about unused `new_default()` - expected)
- ✅ Profile modal no longer shows encoding settings
- ✅ Default group cannot be edited in UI
- ✅ Clarification text added to both modals
- ✅ Duplicate functionality prevents copying `isDefault` flag

## Related Documents

- `.claude/claudedocs/scratch/passthrough-mode-changes.md` - Phase 1: Copy mode defaults
- `.claude/claudedocs/scratch/immutable-default-group.md` - Phase 2: Immutable default group
- This document - Phase 3: Remove profile encoding settings

## Next Steps (Future Work)

1. **Backend Integration**:
   - Wire up `OutputGroup::new_default()` in profile creation
   - Ensure profiles always have default group when loaded
   - Add migration for existing profiles without `isDefault` flag

2. **UI Polish**:
   - Add tooltips explaining passthrough vs re-encode
   - Show estimated bandwidth for each output group
   - Add visual indicators (icons) for passthrough vs transcode

3. **Testing**:
   - Manual testing with actual OBS → SpiritStream → platforms
   - Verify FFmpeg commands generated correctly
   - Test profile import/export with new structure

4. **Documentation**:
   - Update user guide with new workflow
   - Create quick start guide
   - Add troubleshooting for common issues

---

*Completed: 2026-01-04*
