# Pages and Views Documentation

## Overview

MagillaStream's UI is organized into 8 distinct views, each accessible via the sidebar navigation. The application follows a single-page architecture where views are shown/hidden based on the active navigation item.

## Navigation Structure

```
Sidebar Navigation
├── Main
│   ├── Dashboard (view-dashboard) - Default view
│   ├── Profiles (view-profiles)
│   └── Stream Manager (view-streams)
├── Configuration
│   ├── Encoder Settings (view-encoder)
│   ├── Output Groups (view-outputs)
│   └── Stream Targets (view-targets)
└── System
    ├── Logs (view-logs)
    └── Settings (view-settings)
```

---

## 1. Dashboard (`view-dashboard`)

The main landing page providing an at-a-glance overview of streaming status and quick access to common actions.

### Component Structure

```
Dashboard
├── StatsRow
│   ├── StatBox (Active Streams)
│   ├── StatBox (Total Bitrate)
│   ├── StatBox (Dropped Frames)
│   └── StatBox (Uptime)
├── Grid (2-column)
│   ├── Card (Active Profile)
│   │   └── ProfileCard (selected profile preview)
│   └── Card (Quick Actions)
│       └── Grid (2x2 action buttons)
└── Card (Stream Targets)
    └── Grid (3-column)
        └── StreamCard[] (target cards with status)
```

### Components

#### StatBox
Displays a single metric with icon, value, and change indicator.

```typescript
interface StatBoxProps {
  label: string;          // "Active Streams", "Total Bitrate", etc.
  value: string | number; // Current value
  icon: LucideIcon;       // radio, activity, alert-triangle, clock
  change?: string;        // Status text ("Ready to start", "No issues")
  changeType?: 'positive' | 'neutral'; // For styling
}
```

#### Active Profile Card
Shows the currently selected streaming profile with key specs.

```typescript
interface ActiveProfileCardProps {
  profile: {
    name: string;         // "Gaming Stream - High Quality"
    resolution: string;   // "1080p60"
    bitrate: number;      // 6000
    targetCount: number;  // 3
  };
  onChangeClick: () => void;
}
```

#### Quick Actions Card
Grid of 4 action buttons: New Profile, Import Profile, Add Target, Test Stream.

#### Stream Targets Grid
3-column grid of StreamCard components showing each configured target with live stats.

### State Requirements

```typescript
interface DashboardState {
  // Stats
  activeStreamCount: number;
  totalBitrate: number;
  droppedFrames: number;
  uptime: number;  // seconds

  // Profile
  activeProfile: Profile | null;

  // Targets
  streamTargets: StreamTarget[];
  targetStatuses: Map<string, {
    isLive: boolean;
    viewers?: number;
    bitrate?: number;
    fps?: number;
  }>;
}
```

### Tauri Commands

| Command | Purpose |
|---------|---------|
| `get_active_profile` | Load the current active profile |
| `get_stream_status` | Get live/offline status for all targets |
| `get_stream_stats` | Get real-time stats (bitrate, frames, uptime) |

### Props/Data Flow

```
ProfileStore.current ──► ActiveProfileCard
                    └──► StreamTargets (via outputGroups)

StreamStore.isStreaming ──► StatsRow values
                       └──► StreamCard badges (live/offline)

StreamStore.stats ──► StatBox values
                 └──► StreamCard stat values
```

---

## 2. Profiles (`view-profiles`)

Manages saved streaming configurations (profiles).

### Component Structure

```
Profiles
├── Header
│   ├── Title ("Streaming Profiles")
│   ├── Description
│   └── Button ("New Profile") → opens modal-profile
└── Grid (3-column)
    └── ProfileCard[] (all profiles)
```

### Components

#### ProfileCard
Displays a profile with metadata and action buttons.

```typescript
interface ProfileCardProps {
  id: string;
  name: string;           // "Gaming Stream - High Quality"
  resolution: string;     // "1080p60"
  bitrate: number;        // 6000
  targetCount: number;    // 3
  isActive: boolean;      // Shows "Active" badge
  onEdit: () => void;
  onDuplicate: () => void;
  onClick: () => void;    // Select as active
}
```

### State Requirements

```typescript
interface ProfilesState {
  profiles: ProfileSummary[];
  activeProfileId: string | null;
  isLoading: boolean;
  error: string | null;
}

interface ProfileSummary {
  id: string;
  name: string;
  resolution: string;
  bitrate: number;
  targetCount: number;
}
```

### Tauri Commands

| Command | Purpose |
|---------|---------|
| `get_all_profiles` | List all profile names |
| `load_profile` | Load full profile data |
| `save_profile` | Create/update profile |
| `delete_profile` | Remove a profile |
| `duplicate_profile` | Clone an existing profile |
| `set_active_profile` | Mark profile as active |

### Props/Data Flow

```
ProfileStore.profiles ──► ProfileCard[]
ProfileStore.activeId ──► ProfileCard.isActive

ProfileCard.onClick ──► ProfileStore.setActive()
ProfileCard.onEdit ──► Opens edit modal with profile data
ProfileCard.onDuplicate ──► ProfileStore.duplicate()
```

---

## 3. Stream Manager (`view-streams`)

Controls live streaming operations with per-target toggles.

### Component Structure

```
StreamManager
├── Alert (when not streaming)
│   └── "No Active Streams" message
└── Card (Stream Control)
    ├── CardHeader
    │   ├── Title ("Stream Control")
    │   └── Description
    └── CardBody
        ├── OutputGroup[] (accordions)
        │   ├── OutputGroupHeader (expandable)
        │   │   ├── Icon + Name
        │   │   ├── Info ("3 targets • 1080p60 • 6000 kbps")
        │   │   ├── Badge (Ready/Live)
        │   │   └── ChevronIcon
        │   └── OutputGroupBody (when expanded)
        │       └── Grid (3-column)
        │           └── StreamCard[] (with toggle switches)
        └── Footer
            ├── Button ("Configure")
            └── Button ("Start/Stop All Streams")
```

### Components

#### Alert
Info-style alert shown when no streams are active.

```typescript
interface AlertProps {
  type: 'info' | 'success' | 'warning' | 'error';
  title: string;
  description: string;
  icon?: LucideIcon;
}
```

#### OutputGroup Accordion
Expandable section for each output group.

```typescript
interface OutputGroupAccordionProps {
  id: string;
  name: string;              // "Main Output Group"
  info: string;              // "3 targets • 1080p60 • 6000 kbps"
  status: 'ready' | 'live' | 'error';
  isExpanded: boolean;
  targets: StreamTargetToggle[];
  onToggle: () => void;
}

interface StreamTargetToggle {
  id: string;
  name: string;              // "YouTube"
  platform: Platform;        // youtube, twitch, kick, facebook, custom
  isEnabled: boolean;
  onToggle: (enabled: boolean) => void;
}
```

### State Requirements

```typescript
interface StreamManagerState {
  isStreaming: boolean;
  expandedGroups: Set<string>;

  // Per-target enable/disable for this session
  enabledTargets: Set<string>;

  // Streaming status per group
  groupStatuses: Map<string, 'ready' | 'live' | 'error' | 'starting' | 'stopping'>;
}
```

### Tauri Commands

| Command | Purpose |
|---------|---------|
| `start_stream` | Start streaming to an output group |
| `stop_stream` | Stop a specific output group |
| `start_all_streams` | Start all enabled targets |
| `stop_all_streams` | Stop all active streams |
| `get_stream_status` | Poll current streaming status |

### Props/Data Flow

```
ProfileStore.current.outputGroups ──► OutputGroupAccordion[]

StreamStore.isStreaming ──► Alert visibility
                       └──► Button text ("Start" vs "Stop")

StreamStore.enabledTargets ──► Toggle switch states

Toggle.onChange ──► StreamStore.toggleTarget()
StartAllButton.onClick ──► StreamStore.startAll()
```

---

## 4. Encoder Settings (`view-encoder`)

Configures video/audio encoding parameters.

### Component Structure

```
EncoderSettings
└── Card
    ├── CardHeader
    │   ├── Title ("Encoder Configuration")
    │   └── Description
    ├── CardBody
    │   └── Grid (2-column)
    │       ├── Column (Video Encoder)
    │       │   ├── FormGroup (Encoder select)
    │       │   ├── FormGroup (Preset select)
    │       │   └── FormGroup (Rate Control select)
    │       └── Column (Output Settings)
    │           ├── FormGroup (Resolution select)
    │           ├── FormGroup (Frame Rate select)
    │           ├── FormGroup (Video Bitrate input)
    │           └── FormGroup (Keyframe Interval input)
    └── CardFooter
        ├── Button ("Reset to Defaults")
        └── Button ("Save Settings")
```

### Components

#### FormGroup with Select
Labeled dropdown for encoder options.

```typescript
interface EncoderSelectProps {
  label: string;
  value: string;
  options: EncoderOption[];
  helperText?: string;
  onChange: (value: string) => void;
}

interface EncoderOption {
  value: string;
  label: string;
  disabled?: boolean;
}
```

#### FormGroup with NumberInput
Labeled numeric input with constraints.

```typescript
interface BitrateInputProps {
  label: string;
  value: number;
  min: number;
  max: number;
  helperText?: string;
  onChange: (value: number) => void;
}
```

### State Requirements

```typescript
interface EncoderSettingsState {
  // Video encoder
  encoder: string;          // 'x264', 'nvenc_h264', 'qsv_h264', 'amf_h264'
  preset: string;           // 'quality', 'balanced', 'performance', 'low_latency'
  rateControl: string;      // 'cbr', 'vbr', 'cqp'

  // Output settings
  resolution: string;       // '1920x1080', '1280x720', etc.
  frameRate: number;        // 60, 30, 24
  videoBitrate: number;     // kbps
  keyframeInterval: number; // seconds

  // Available options (from system detection)
  availableEncoders: EncoderOption[];

  // UI state
  isDirty: boolean;
  isSaving: boolean;
}
```

### Tauri Commands

| Command | Purpose |
|---------|---------|
| `get_video_encoders` | Detect available hardware/software encoders |
| `get_audio_encoders` | Detect available audio codecs |
| `save_encoder_settings` | Persist encoder configuration |
| `get_encoder_settings` | Load current encoder configuration |
| `reset_encoder_settings` | Reset to defaults |

### Props/Data Flow

```
SystemStore.availableEncoders ──► Encoder select options

EncoderStore.settings ──► Form field values

Form.onChange ──► EncoderStore.updateField()
SaveButton.onClick ──► EncoderStore.save()
ResetButton.onClick ──► EncoderStore.reset()
```

---

## 5. Output Groups (`view-outputs`)

Manages output group configurations (encoding + targets bundles).

### Component Structure

```
OutputGroups
├── Header
│   ├── Title ("Output Groups")
│   ├── Description
│   └── Button ("New Output Group")
└── OutputGroupCard[] (vertical stack)
    └── Card
        ├── CardHeader
        │   ├── Icon (layers)
        │   ├── Title + Description
        │   └── Actions (Edit, Duplicate buttons)
        └── CardBody
            ├── Grid (4-column specs)
            │   ├── Resolution
            │   ├── Frame Rate
            │   ├── Video Bitrate
            │   └── Encoder
            └── Targets list (text)
```

### Components

#### OutputGroupCard
Full card display for an output group.

```typescript
interface OutputGroupCardProps {
  id: string;
  name: string;              // "Main Output Group"
  description: string;       // "High quality stream for all platforms"
  resolution: string;        // "1920x1080"
  frameRate: number;         // 60
  videoBitrate: number;      // 6000
  encoder: string;           // "NVENC H.264"
  targets: string[];         // ["YouTube Gaming", "Twitch", "Kick"]
  iconColor: 'primary' | 'secondary';
  onEdit: () => void;
  onDuplicate: () => void;
  onDelete: () => void;
}
```

### State Requirements

```typescript
interface OutputGroupsState {
  groups: OutputGroup[];
  isLoading: boolean;
  selectedGroupId: string | null;  // For editing
}

interface OutputGroup {
  id: string;
  name: string;
  description?: string;
  videoEncoder: string;
  resolution: string;
  videoBitrate: number;
  fps: number;
  audioCodec: string;
  audioBitrate: number;
  streamTargets: StreamTarget[];
}
```

### Tauri Commands

| Command | Purpose |
|---------|---------|
| `get_output_groups` | Load all output groups from active profile |
| `save_output_group` | Create/update an output group |
| `delete_output_group` | Remove an output group |
| `duplicate_output_group` | Clone an output group |

### Props/Data Flow

```
ProfileStore.current.outputGroups ──► OutputGroupCard[]

OutputGroupCard.onEdit ──► Opens edit modal
OutputGroupCard.onDuplicate ──► ProfileStore.duplicateGroup()
OutputGroupCard.onDelete ──► ProfileStore.removeGroup()

NewButton.onClick ──► Opens create modal
```

---

## 6. Stream Targets (`view-targets`)

Configures RTMP destinations (platforms and custom servers).

### Component Structure

```
StreamTargets
├── Header
│   ├── Title ("Stream Targets")
│   ├── Description
│   └── Button ("Add Target") → opens modal-target
└── Grid (2-column)
    ├── TargetCard[] (existing targets)
    │   └── Card
    │       └── CardBody
    │           ├── Header (icon + name + URL)
    │           ├── FormGroup (Stream Key - masked)
    │           │   └── Input + Eye button + Copy button
    │           └── Footer (Edit + Delete buttons)
    └── AddNewCard (dashed border placeholder)
        └── Button ("Add New Target")
```

### Components

#### TargetCard
Full card for a stream target with masked key.

```typescript
interface TargetCardProps {
  id: string;
  platform: Platform;        // youtube, twitch, kick, facebook, custom
  name: string;              // "YouTube Gaming"
  url: string;               // "rtmp://a.rtmp.youtube.com/live2"
  streamKey: string;         // Masked by default
  onEdit: () => void;
  onDelete: () => void;
  onCopyKey: () => void;
}

type Platform = 'youtube' | 'twitch' | 'kick' | 'facebook' | 'custom';
```

#### Platform Icon
Colored icon box for each platform.

```typescript
const platformStyles: Record<Platform, { bg: string; label: string }> = {
  youtube: { bg: '#FF0000', label: 'YT' },
  twitch: { bg: '#9146FF', label: 'TW' },
  kick: { bg: '#53FC18', label: 'K' },
  facebook: { bg: '#1877F2', label: 'FB' },
  custom: { bg: 'var(--primary)', label: 'RT' },
};
```

#### Masked Stream Key Input
Password input with reveal toggle and copy button.

```typescript
interface StreamKeyInputProps {
  value: string;
  isRevealed: boolean;
  onToggleReveal: () => void;
  onCopy: () => void;
}
```

### State Requirements

```typescript
interface StreamTargetsState {
  targets: StreamTarget[];
  revealedKeys: Set<string>;  // Track which keys are visible
  isLoading: boolean;
}

interface StreamTarget {
  id: string;
  platform: Platform;
  name: string;
  url: string;
  streamKey: string;  // Encrypted at rest
  port: number;
}
```

### Tauri Commands

| Command | Purpose |
|---------|---------|
| `get_stream_targets` | Load all targets from active profile |
| `save_stream_target` | Create/update a target |
| `delete_stream_target` | Remove a target |
| `decrypt_stream_key` | Temporarily decrypt a key for display |
| `copy_to_clipboard` | Securely copy key to clipboard |

### Props/Data Flow

```
ProfileStore.current.outputGroups[*].streamTargets ──► TargetCard[]

TargetCard.onEdit ──► Opens edit modal with target data
TargetCard.onDelete ──► Confirmation → ProfileStore.removeTarget()
TargetCard.onCopyKey ──► Tauri clipboard command

RevealButton.onClick ──► TargetsStore.toggleReveal(id)
```

---

## 7. Logs (`view-logs`)

Displays real-time application logs with filtering.

### Component Structure

```
Logs
└── Card
    ├── CardHeader
    │   ├── Title ("Application Logs")
    │   ├── Description
    │   └── Controls
    │       ├── Select (filter by level)
    │       ├── Button ("Export")
    │       └── Button ("Clear")
    └── CardBody (no padding)
        └── LogConsole
            └── LogEntry[] (scrollable list)
                ├── Time (HH:MM:SS)
                ├── Level (INFO/WARN/ERROR/DEBUG)
                └── Message
```

### Components

#### LogConsole
Monospace scrollable container for log entries.

```typescript
interface LogConsoleProps {
  entries: LogEntry[];
  filter: LogLevel | 'all';
  maxEntries?: number;  // Default 1000
}

interface LogEntry {
  id: string;
  timestamp: Date;
  level: LogLevel;
  message: string;
  source?: string;
}

type LogLevel = 'info' | 'warn' | 'error' | 'debug';
```

#### LogEntry
Single log line with color-coded level.

```typescript
const levelStyles: Record<LogLevel, string> = {
  info: 'color: var(--primary)',
  warn: 'color: var(--warning-text)',
  error: 'color: var(--error-text)',
  debug: 'color: var(--text-tertiary)',
};
```

### State Requirements

```typescript
interface LogsState {
  entries: LogEntry[];
  filter: LogLevel | 'all';
  isAutoScroll: boolean;
}
```

### Tauri Commands

| Command | Purpose |
|---------|---------|
| `get_logs` | Load log history |
| `clear_logs` | Clear log buffer |
| `export_logs` | Save logs to file |

### Tauri Events (Listen)

| Event | Purpose |
|-------|---------|
| `log_entry` | Real-time log streaming from backend |

### Props/Data Flow

```
LogStore.entries ──► LogConsole
LogStore.filter ──► Filters displayed entries

FilterSelect.onChange ──► LogStore.setFilter()
ExportButton.onClick ──► LogStore.export()
ClearButton.onClick ──► LogStore.clear()

Tauri.listen('log_entry') ──► LogStore.addEntry()
```

---

## 8. Settings (`view-settings`)

Application-wide configuration options.

### Component Structure

```
Settings
└── Grid (2x2)
    ├── Card (General Settings)
    │   ├── FormGroup (Language select)
    │   ├── FormGroup (Start Minimized toggle)
    │   └── FormGroup (Show Notifications toggle)
    ├── Card (FFmpeg Configuration)
    │   ├── FormGroup (FFmpeg Path + Browse)
    │   ├── FormGroup (FFmpeg Version - readonly)
    │   └── FormGroup (Auto-Download toggle)
    ├── Card (Data & Privacy)
    │   ├── FormGroup (Profile Storage path)
    │   ├── FormGroup (Encrypt Stream Keys toggle)
    │   └── Buttons (Export Data, Clear All Data)
    └── Card (About)
        ├── Logo + App Name + Version
        ├── Description text
        └── Buttons (GitHub, Docs, Updates)
```

### Components

#### SettingsCard
Card wrapper for a settings section.

```typescript
interface SettingsCardProps {
  title: string;
  children: React.ReactNode;
}
```

#### SettingsToggle
Toggle switch with label and description.

```typescript
interface SettingsToggleProps {
  label: string;
  description?: string;
  value: boolean;
  onChange: (value: boolean) => void;
}
```

#### PathInput
Read-only path input with browse/open button.

```typescript
interface PathInputProps {
  label: string;
  value: string;
  helperText?: string;
  action: 'browse' | 'open';
  onAction: () => void;
}
```

### State Requirements

```typescript
interface SettingsState {
  // General
  language: string;
  startMinimized: boolean;
  showNotifications: boolean;

  // FFmpeg
  ffmpegPath: string;
  ffmpegVersion: string;
  autoDownloadFfmpeg: boolean;

  // Data & Privacy
  profileStoragePath: string;
  encryptStreamKeys: boolean;

  // About
  appVersion: string;
}
```

### Tauri Commands

| Command | Purpose |
|---------|---------|
| `get_settings` | Load all settings |
| `save_settings` | Persist settings changes |
| `get_ffmpeg_path` | Get detected FFmpeg location |
| `get_ffmpeg_version` | Get FFmpeg version string |
| `browse_for_folder` | Open folder picker dialog |
| `open_folder` | Open folder in file manager |
| `export_all_data` | Export profiles/settings |
| `clear_all_data` | Wipe all app data |
| `check_for_updates` | Query update server |

### Props/Data Flow

```
SettingsStore.settings ──► All form fields

Toggle.onChange ──► SettingsStore.update()
BrowseButton.onClick ──► Tauri dialog → SettingsStore.setPath()
ExportButton.onClick ──► SettingsStore.exportData()
ClearButton.onClick ──► Confirmation → SettingsStore.clearData()
```

---

## Modals

### New Profile Modal (`modal-profile`)

Used from: Dashboard (Quick Actions), Profiles (New Profile button)

```typescript
interface ProfileModalProps {
  mode: 'create' | 'edit';
  profile?: Profile;
  onSave: (profile: ProfileDTO) => void;
  onClose: () => void;
}

// Form fields
interface ProfileFormData {
  name: string;
  resolution: string;
  frameRate: number;
  videoBitrate: number;
}
```

### Add Target Modal (`modal-target`)

Used from: Dashboard (Quick Actions + Add Target), Stream Targets (Add Target)

```typescript
interface TargetModalProps {
  mode: 'create' | 'edit';
  target?: StreamTarget;
  onSave: (target: StreamTargetDTO) => void;
  onClose: () => void;
}

// Form fields
interface TargetFormData {
  platform: Platform;
  name: string;
  serverUrl: string;
  streamKey: string;
}
```

---

## Global State Stores

### ProfileStore (Zustand)

```typescript
interface ProfileStore {
  // State
  profiles: ProfileSummary[];
  current: Profile | null;
  loading: boolean;
  error: string | null;

  // Actions
  loadProfiles: () => Promise<void>;
  loadProfile: (name: string, password?: string) => Promise<void>;
  saveProfile: (profile: ProfileDTO) => Promise<void>;
  deleteProfile: (name: string) => Promise<void>;
  setActive: (id: string) => void;

  // Output Group actions
  addOutputGroup: (group: OutputGroupDTO) => void;
  updateOutputGroup: (id: string, updates: Partial<OutputGroupDTO>) => void;
  removeOutputGroup: (id: string) => void;

  // Target actions
  addTarget: (groupId: string, target: StreamTargetDTO) => void;
  updateTarget: (groupId: string, targetId: string, updates: Partial<StreamTargetDTO>) => void;
  removeTarget: (groupId: string, targetId: string) => void;
}
```

### StreamStore (Zustand)

```typescript
interface StreamStore {
  // State
  isStreaming: boolean;
  activeGroups: Set<string>;
  enabledTargets: Set<string>;
  stats: StreamStats;
  uptime: number;

  // Actions
  startAll: () => Promise<void>;
  stopAll: () => Promise<void>;
  startGroup: (groupId: string) => Promise<void>;
  stopGroup: (groupId: string) => Promise<void>;
  toggleTarget: (targetId: string, enabled: boolean) => void;

  // Stats polling
  startStatsPolling: () => void;
  stopStatsPolling: () => void;
}

interface StreamStats {
  totalBitrate: number;
  droppedFrames: number;
  targetStats: Map<string, TargetStats>;
}
```

### SettingsStore (Zustand)

```typescript
interface SettingsStore {
  // State
  settings: AppSettings;
  loading: boolean;

  // Actions
  load: () => Promise<void>;
  update: (key: keyof AppSettings, value: any) => void;
  save: () => Promise<void>;
  reset: () => Promise<void>;
  exportData: () => Promise<void>;
  clearData: () => Promise<void>;
}
```

### LogStore (Zustand)

```typescript
interface LogStore {
  // State
  entries: LogEntry[];
  filter: LogLevel | 'all';

  // Actions
  addEntry: (entry: LogEntry) => void;
  setFilter: (filter: LogLevel | 'all') => void;
  clear: () => void;
  export: () => Promise<void>;
}
```

---

## Tauri Commands Summary

### Profile Management
- `get_all_profiles` - List profile names
- `load_profile` - Load full profile
- `save_profile` - Create/update profile
- `delete_profile` - Remove profile
- `duplicate_profile` - Clone profile
- `set_active_profile` - Mark as active
- `get_last_used_profile` - Get last used

### Stream Control
- `start_stream` - Start output group
- `stop_stream` - Stop output group
- `start_all_streams` - Start all enabled
- `stop_all_streams` - Stop all
- `get_stream_status` - Get live status
- `get_stream_stats` - Get metrics

### System
- `get_video_encoders` - Detect video encoders
- `get_audio_encoders` - Detect audio codecs
- `get_ffmpeg_path` - FFmpeg location
- `get_ffmpeg_version` - FFmpeg version
- `test_ffmpeg` - Verify FFmpeg works

### Settings
- `get_settings` - Load settings
- `save_settings` - Save settings
- `export_all_data` - Export to file
- `clear_all_data` - Wipe data
- `check_for_updates` - Query updates

### Logging
- `get_logs` - Load log history
- `clear_logs` - Clear logs
- `export_logs` - Save to file

### Security
- `decrypt_stream_key` - Decrypt for display
- `copy_to_clipboard` - Secure clipboard

### Dialogs
- `browse_for_folder` - Folder picker
- `open_folder` - Open in file manager
