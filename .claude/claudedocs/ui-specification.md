# MagillaStream UI/UX Specification

> Comprehensive design system and component specification for React development

---

## Table of Contents

1. [Page/View Structure](#1-pageview-structure)
2. [Component Inventory](#2-component-inventory)
3. [Layout Specifications](#3-layout-specifications)
4. [Typography](#4-typography)
5. [Color System](#5-color-system)
6. [Platform Icons](#6-platform-icons)
7. [Interactive States](#7-interactive-states)
8. [Animations](#8-animations)

---

## 1. Page/View Structure

### 1.1 Dashboard (`view-dashboard`)

**Purpose:** Primary overview showing streaming status and quick actions.

**Components:**
- Stats Row (4 stat boxes)
  - Active Streams
  - Total Bitrate
  - Dropped Frames
  - Uptime
- Active Profile Card
- Quick Actions Card (4 buttons: New Profile, Import Profile, Add Target, Test Stream)
- Stream Targets Grid (3 stream cards)

**Data Requirements:**
```typescript
interface DashboardData {
  activeStreams: number;
  totalBitrate: string;
  droppedFrames: number;
  uptime: string;
  activeProfile: Profile | null;
  streamTargets: StreamTarget[];
}
```

---

### 1.2 Profiles (`view-profiles`)

**Purpose:** Manage saved streaming configurations.

**Components:**
- Page Header with "New Profile" button
- Profile Cards Grid (3 columns)
  - Each card shows: name, resolution, bitrate, target count
  - Action buttons: Edit, Duplicate

**Data Requirements:**
```typescript
interface Profile {
  id: string;
  name: string;
  resolution: string;       // e.g., "1080p60"
  bitrate: number;          // in kbps
  targetCount: number;
  isActive: boolean;
}
```

---

### 1.3 Stream Manager (`view-streams`)

**Purpose:** Control live streaming to all targets.

**Components:**
- Alert Banner (info state when not streaming)
- Stream Control Card
  - Output Group (expandable accordion)
    - Stream target toggles within
  - Action buttons: Configure, Start All Streams

**Data Requirements:**
```typescript
interface OutputGroupDisplay {
  id: string;
  name: string;
  targets: StreamTarget[];
  resolution: string;
  bitrate: number;
  isExpanded: boolean;
  status: 'ready' | 'live' | 'error';
}
```

---

### 1.4 Encoder Settings (`view-encoder`)

**Purpose:** Configure video/audio encoding parameters.

**Components:**
- Two-column form layout
- Left Column: Video Encoder Settings
  - Encoder select (x264, NVENC, QuickSync, AMF)
  - Preset select (Quality, Balanced, Performance, Low Latency)
  - Rate Control select (CBR, VBR, CQP)
- Right Column: Output Settings
  - Resolution select
  - Frame Rate select
  - Video Bitrate input
  - Keyframe Interval input
- Card Footer with Reset/Save buttons

**Form Fields:**
```typescript
interface EncoderSettings {
  encoder: 'x264' | 'nvenc' | 'quicksync' | 'amf';
  preset: 'quality' | 'balanced' | 'performance' | 'low-latency';
  rateControl: 'cbr' | 'vbr' | 'cqp';
  resolution: '1080p' | '720p' | '1440p' | '4k';
  frameRate: 60 | 30 | 24;
  videoBitrate: number;
  keyframeInterval: number;
}
```

---

### 1.5 Output Groups (`view-outputs`)

**Purpose:** Configure multiple output configurations with different settings.

**Components:**
- Page Header with "New Output Group" button
- Output Group Cards (stacked)
  - Header: icon, name, description, Edit/Duplicate buttons
  - Body: 4-column specs grid (Resolution, Frame Rate, Bitrate, Encoder)
  - Targets list

**Data Requirements:**
```typescript
interface OutputGroup {
  id: string;
  name: string;
  description: string;
  resolution: string;
  frameRate: number;
  videoBitrate: number;
  encoder: string;
  targets: string[];
  color?: 'primary' | 'secondary';
}
```

---

### 1.6 Stream Targets (`view-targets`)

**Purpose:** Configure streaming destinations.

**Components:**
- Page Header with "Add Target" button
- Target Cards Grid (2 columns)
  - Platform icon and info
  - Server URL display
  - Stream Key field (password with show/copy buttons)
  - Edit/Delete actions
- Add New Target placeholder card (dashed border)

**Data Requirements:**
```typescript
interface StreamTarget {
  id: string;
  platform: 'youtube' | 'twitch' | 'kick' | 'facebook' | 'custom';
  name: string;
  serverUrl: string;
  streamKey: string;
  status: 'offline' | 'live' | 'error';
}
```

---

### 1.7 Logs (`view-logs`)

**Purpose:** Real-time application logging.

**Components:**
- Card with header controls
  - Level filter select
  - Export button
  - Clear button
- Log Console (monospace, scrollable)
  - Log entries with timestamp, level, message

**Log Entry Structure:**
```typescript
interface LogEntry {
  timestamp: string;    // "HH:MM:SS" format
  level: 'info' | 'warn' | 'error' | 'debug';
  message: string;
}
```

---

### 1.8 Settings (`view-settings`)

**Purpose:** Application configuration.

**Components (4 cards in 2x2 grid):**

1. **General Settings**
   - Language select
   - Start Minimized toggle
   - Show Notifications toggle

2. **FFmpeg Configuration**
   - FFmpeg Path input with Browse button
   - FFmpeg Version (disabled)
   - Auto-Download FFmpeg toggle

3. **Data & Privacy**
   - Profile Storage path with Open button
   - Encrypt Stream Keys toggle
   - Export Data button
   - Clear All Data button (destructive)

4. **About**
   - Logo and version info
   - GitHub, Docs, Updates buttons

---

## 2. Component Inventory

### 2.1 Navigation Components

#### Sidebar
```typescript
interface SidebarProps {
  currentView: string;
  onViewChange: (view: string) => void;
}
```

**Structure:**
- Width: 260px (fixed)
- Position: Fixed, left, full height
- Background: `var(--bg-surface)`
- Border: 1px solid `var(--border-default)` on right

**Sub-components:**
- `SidebarHeader` - Logo + app name
- `SidebarNav` - Navigation sections
- `SidebarFooter` - Theme toggle

#### NavItem
```typescript
interface NavItemProps {
  icon: LucideIcon;
  label: string;
  view: string;
  isActive: boolean;
  badge?: number;
  onClick: () => void;
}
```

**CSS Classes:**
- `.nav-item` - Base styles
- `.nav-item.active` - Active state (purple background)
- `.nav-item:hover` - Hover state

#### NavBadge
```typescript
interface NavBadgeProps {
  count: number;
}
```

**Styling:**
- Background: `var(--primary)`
- Color: white
- Border-radius: 9999px (pill)
- Font-size: 0.6875rem

---

### 2.2 Card Components

#### StatBox
```typescript
interface StatBoxProps {
  label: string;
  value: string | number;
  icon: LucideIcon;
  change?: string;
  changeType?: 'positive' | 'neutral';
}
```

**Structure:**
```
[Icon]    [Label]
[Value - large]
[Change text - small]
```

#### ProfileCard
```typescript
interface ProfileCardProps {
  name: string;
  resolution: string;
  bitrate: number;
  targetCount: number;
  isActive: boolean;
  onEdit: () => void;
  onDuplicate: () => void;
  onClick: () => void;
}
```

**States:**
- Default: `border: 2px solid var(--border-default)`
- Active: `border-color: var(--primary)`, `background: var(--primary-muted)`
- Hover: `transform: translateY(-2px)`, `box-shadow: var(--shadow-md)`

#### StreamCard
```typescript
interface StreamCardProps {
  platform: Platform;
  name: string;
  status: 'offline' | 'live' | 'error';
  stats?: {
    viewers: number;
    bitrate: string;
    fps: string;
  };
  showToggle?: boolean;
  enabled?: boolean;
  onToggle?: (enabled: boolean) => void;
}
```

#### OutputGroupCard
```typescript
interface OutputGroupCardProps {
  name: string;
  description: string;
  resolution: string;
  frameRate: number;
  bitrate: number;
  encoder: string;
  targets: string[];
  iconColor?: 'primary' | 'secondary';
  onEdit: () => void;
  onDuplicate: () => void;
}
```

#### Card (Generic)
```typescript
interface CardProps {
  children: React.ReactNode;
  className?: string;
}

interface CardHeaderProps {
  title: string;
  description?: string;
  actions?: React.ReactNode;
}

interface CardBodyProps {
  children: React.ReactNode;
  noPadding?: boolean;
}

interface CardFooterProps {
  children: React.ReactNode;
}
```

---

### 2.3 Form Components

#### FormGroup
```typescript
interface FormGroupProps {
  children: React.ReactNode;
}
```

#### FormLabel
```typescript
interface FormLabelProps {
  children: React.ReactNode;
  htmlFor?: string;
}
```

#### FormInput
```typescript
interface FormInputProps extends React.InputHTMLAttributes<HTMLInputElement> {
  error?: string;
  helper?: string;
}
```

**States:**
- Default: `border: 2px solid var(--border-strong)`
- Focus: `border-color: var(--border-interactive)`, `box-shadow: 0 0 0 3px var(--primary-muted)`
- Disabled: reduced opacity
- Error: red border

#### FormSelect
```typescript
interface FormSelectProps extends React.SelectHTMLAttributes<HTMLSelectElement> {
  options: { value: string; label: string }[];
  helper?: string;
}
```

**Custom styling:** Custom dropdown arrow via background-image SVG

#### Toggle
```typescript
interface ToggleProps {
  checked: boolean;
  onChange: (checked: boolean) => void;
  disabled?: boolean;
}
```

**Dimensions:**
- Width: 44px
- Height: 24px
- Knob: 18px diameter

**States:**
- Off: `background: var(--border-strong)`
- On: `background: var(--primary)`
- Transition: 0.2s

---

### 2.4 Button Components

#### Button Variants

```typescript
interface ButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  variant: 'primary' | 'secondary' | 'outline' | 'ghost' | 'destructive';
  size?: 'sm' | 'md' | 'lg';
  icon?: boolean;  // Icon-only button
  children: React.ReactNode;
}
```

**Variant Styles:**

| Variant | Background | Color | Border |
|---------|------------|-------|--------|
| primary | `var(--primary)` | white | none |
| secondary | `var(--secondary)` | white | none |
| outline | transparent | `var(--primary)` | 2px solid `var(--primary)` |
| ghost | transparent | `var(--text-secondary)` | none |
| destructive | `var(--error)` | white | none |

**Size Styles:**

| Size | Padding | Font-size |
|------|---------|-----------|
| sm | 0.375rem 0.75rem | 0.8125rem |
| md | 0.625rem 1rem | 0.875rem |
| lg | 0.75rem 1.5rem | 1rem |

**Icon Button:**
- Width/Height: 36px
- Padding: 0

---

### 2.5 Modal Components

#### Modal
```typescript
interface ModalProps {
  isOpen: boolean;
  onClose: () => void;
  title: string;
  children: React.ReactNode;
  footer?: React.ReactNode;
  maxWidth?: number;  // default 500px
}
```

**Structure:**
- Overlay: fixed, inset 0, semi-transparent background
- Modal: centered, max-width 500px, max-height 90vh
- Animation: scale 0.95 -> 1.0 on open

**Sub-components:**
- `ModalHeader` - Title + close button
- `ModalBody` - Content area (scrollable)
- `ModalFooter` - Action buttons

#### Specific Modals:

**CreateProfileModal:**
- Profile Name input
- Resolution select
- Frame Rate select
- Video Bitrate input

**AddTargetModal:**
- Platform select (YouTube, Twitch, Kick, Facebook, Custom RTMP)
- Name input
- Server URL input
- Stream Key input (password)

---

### 2.6 Alert Components

```typescript
interface AlertProps {
  variant: 'info' | 'success' | 'warning' | 'error';
  title: string;
  description?: string;
  icon?: LucideIcon;
}
```

**Variant Styles:**

| Variant | Background | Border | Color |
|---------|------------|--------|-------|
| info | `var(--primary-muted)` | `var(--primary)` | `var(--primary)` |
| success | `var(--success-subtle)` | `var(--success-border)` | `var(--success-text)` |
| warning | `var(--warning-subtle)` | `var(--warning-border)` | `var(--warning-text)` |
| error | `var(--error-subtle)` | `var(--error-border)` | `var(--error-text)` |

---

### 2.7 Status Indicators

#### Badge
```typescript
interface BadgeProps {
  variant: 'live' | 'offline' | 'error';
  children?: React.ReactNode;
}
```

**Structure:**
```jsx
<span className="badge badge-{variant}">
  <span className="badge-dot"></span>
  {children}
</span>
```

**Badge Dot Animation (live):**
```css
@keyframes pulse {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.5; }
}
animation: pulse 2s infinite;
```

---

### 2.8 Log Console

```typescript
interface LogConsoleProps {
  entries: LogEntry[];
  maxHeight?: number;  // default 300px
}

interface LogEntry {
  timestamp: string;
  level: 'info' | 'warn' | 'error' | 'debug';
  message: string;
}
```

**Log Level Colors:**
- info: `var(--primary)`
- warn: `var(--warning-text)`
- error: `var(--error-text)`
- debug: `var(--text-tertiary)`

**Font:** JetBrains Mono, 0.75rem

---

### 2.9 Output Group (Accordion)

```typescript
interface OutputGroupAccordionProps {
  name: string;
  info: string;  // e.g., "3 targets - 1080p60 - 6000 kbps"
  status: 'ready' | 'live' | 'error';
  isExpanded: boolean;
  onToggle: () => void;
  children: React.ReactNode;
}
```

**Animation:**
- Chevron rotation: 0deg -> 180deg on expand
- Body: display none/block (or height animation)

---

## 3. Layout Specifications

### 3.1 App Shell

```
+------------------+----------------------------------------+
|     SIDEBAR      |              MAIN                      |
|     (260px)      |                                        |
|                  |  +----------------------------------+  |
|   [Logo]         |  |           HEADER (sticky)        |  |
|   [App Name]     |  |  [Title]           [Actions]     |  |
|                  |  +----------------------------------+  |
|   MAIN           |                                        |
|   - Dashboard    |  +----------------------------------+  |
|   - Profiles     |  |                                  |  |
|   - Stream Mgr   |  |           CONTENT                |  |
|                  |  |         (scrollable)             |  |
|   CONFIG         |  |                                  |  |
|   - Encoder      |  |                                  |  |
|   - Outputs      |  |                                  |  |
|   - Targets      |  |                                  |  |
|                  |  |                                  |  |
|   SYSTEM         |  |                                  |  |
|   - Logs         |  |                                  |  |
|   - Settings     |  +----------------------------------+  |
|                  |                                        |
|   [Theme Toggle] |                                        |
+------------------+----------------------------------------+
```

### 3.2 Sidebar

```css
.sidebar {
  width: 260px;  /* var(--sidebar-width) */
  position: fixed;
  top: 0;
  left: 0;
  bottom: 0;
  z-index: 100;
  display: flex;
  flex-direction: column;
}
```

**Sections:**
- Header: 1.25rem 1rem padding
- Nav: 1rem 0.75rem padding, flex: 1, overflow-y: auto
- Footer: 1rem padding

### 3.3 Main Content Area

```css
.main {
  flex: 1;
  margin-left: 260px;  /* var(--sidebar-width) */
  display: flex;
  flex-direction: column;
}
```

### 3.4 Header

```css
.header {
  position: sticky;
  top: 0;
  z-index: 50;
  padding: 1rem 1.5rem;
  display: flex;
  align-items: center;
  justify-content: space-between;
}
```

### 3.5 Content

```css
.content {
  flex: 1;
  padding: 1.5rem;
  overflow-y: auto;
}
```

### 3.6 Grid Systems

```css
.grid { display: grid; gap: 1.5rem; }
.grid-2 { grid-template-columns: repeat(2, 1fr); }
.grid-3 { grid-template-columns: repeat(3, 1fr); }
.grid-4 { grid-template-columns: repeat(4, 1fr); }

/* Responsive breakpoints */
@media (max-width: 1200px) {
  .grid-4, .grid-3 { grid-template-columns: repeat(2, 1fr); }
}
@media (max-width: 768px) {
  .grid-4, .grid-3, .grid-2 { grid-template-columns: 1fr; }
}
```

### 3.7 Stats Row (Dashboard)

```css
.stats-row {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: 1rem;
  margin-bottom: 1.5rem;
}
```

---

## 4. Typography

### 4.1 Font Families

```css
/* Primary UI Font */
font-family: 'Space Grotesk', system-ui, sans-serif;

/* Code/Logs Font */
font-family: 'JetBrains Mono', monospace;
```

### 4.2 Font Loading

```html
<link href="https://fonts.googleapis.com/css2?family=Space+Grotesk:wght@400;500;600;700&family=JetBrains+Mono:wght@400;500&display=swap" rel="stylesheet">
```

### 4.3 Type Scale

| Element | Size | Weight | Line Height |
|---------|------|--------|-------------|
| Page Title | 1.25rem (20px) | 600 | 1.5 |
| Card Title | 1rem (16px) | 600 | 1.5 |
| Modal Title | 1.125rem (18px) | 600 | 1.5 |
| Body | 0.875rem (14px) | 400 | 1.5 |
| Small | 0.8125rem (13px) | 400-500 | 1.5 |
| Caption | 0.75rem (12px) | 400 | 1.5 |
| Tiny | 0.6875rem (11px) | 500-600 | 1.5 |
| Stat Value | 1.5rem (24px) | 700 | 1.2 |

### 4.4 Text Colors

```css
--text-primary    /* Main content */
--text-secondary  /* Secondary content, descriptions */
--text-tertiary   /* Labels, helpers */
--text-muted      /* Placeholders, disabled */
--text-disabled   /* Disabled content */
--text-inverse    /* On dark backgrounds */
```

---

## 5. Color System

### 5.1 Light Theme

```css
/* Backgrounds */
--bg-base: #FAFAFA;
--bg-surface: #FFFFFF;
--bg-elevated: #FFFFFF;
--bg-muted: #F4F2F7;
--bg-sunken: #EFECF3;
--bg-overlay: rgba(31, 26, 41, 0.6);
--bg-hover: rgba(124, 58, 237, 0.06);
--bg-selected: #EDE9FE;

/* Text */
--text-primary: #1F1A29;
--text-secondary: #5E5472;
--text-tertiary: #756A8A;
--text-muted: #9489A8;
--text-disabled: #B8AECA;
--text-inverse: #FFFFFF;

/* Primary (Violet) */
--primary: #7C3AED;
--primary-hover: #6D28D9;
--primary-subtle: #EDE9FE;
--primary-muted: #F5F3FF;

/* Secondary (Fuchsia) */
--secondary: #C026D3;

/* Accent (Pink) */
--accent: #DB2777;

/* Borders */
--border-default: #E9E5EF;
--border-muted: #F4F2F7;
--border-strong: #D8D1E2;
--border-interactive: #7C3AED;

/* Shadows */
--shadow-sm: 0 1px 3px rgba(31, 26, 41, 0.08);
--shadow-md: 0 4px 6px rgba(31, 26, 41, 0.1);
--shadow-lg: 0 10px 20px rgba(31, 26, 41, 0.12);
--shadow-xl: 0 20px 40px rgba(31, 26, 41, 0.15);

/* Semantic Colors */
--success: #059669;
--success-subtle: #ECFDF5;
--success-text: #065F46;
--success-border: #34D399;

--warning: #D97706;
--warning-subtle: #FFFBEB;
--warning-text: #92400E;
--warning-border: #FBBF24;

--error: #DC2626;
--error-subtle: #FEF2F2;
--error-text: #991B1B;
--error-border: #F87171;

/* Status */
--status-live: #10B981;
--status-live-bg: #ECFDF5;
--status-live-text: #065F46;

--status-offline: #9489A8;
--status-offline-bg: #F4F2F7;
--status-offline-text: #5E5472;

/* Gradient */
--gradient-brand: linear-gradient(135deg, #7C3AED 0%, #C026D3 50%, #DB2777 100%);
```

### 5.2 Dark Theme

```css
[data-theme="dark"] {
  /* Backgrounds */
  --bg-base: #0F0A14;
  --bg-surface: #1A1225;
  --bg-elevated: #251A33;
  --bg-muted: #1A1225;
  --bg-sunken: #0A0710;
  --bg-overlay: rgba(0, 0, 0, 0.75);
  --bg-hover: rgba(167, 139, 250, 0.1);
  --bg-selected: #2E1A4A;

  /* Text */
  --text-primary: #F4F2F7;
  --text-secondary: #D8D1E2;
  --text-tertiary: #B8AECA;
  --text-muted: #9489A8;
  --text-disabled: #5E5472;
  --text-inverse: #1F1A29;

  /* Primary (Violet - lighter for dark mode) */
  --primary: #A78BFA;
  --primary-hover: #C4B5FD;
  --primary-subtle: #2E1A4A;
  --primary-muted: #1E1433;

  /* Secondary/Accent (lighter) */
  --secondary: #E879F9;
  --accent: #F472B6;

  /* Borders */
  --border-default: #3D3649;
  --border-muted: #2D2838;
  --border-strong: #5E5472;
  --border-interactive: #A78BFA;

  /* Shadows (darker) */
  --shadow-sm: 0 1px 3px rgba(0, 0, 0, 0.4);
  --shadow-md: 0 4px 6px rgba(0, 0, 0, 0.5);
  --shadow-lg: 0 10px 20px rgba(0, 0, 0, 0.5);
  --shadow-xl: 0 20px 40px rgba(0, 0, 0, 0.6);

  /* Semantic (adjusted for dark) */
  --success: #34D399;
  --success-subtle: #052E16;
  --success-text: #6EE7B7;
  --success-border: #10B981;

  --warning: #FBBF24;
  --warning-subtle: #451A03;
  --warning-text: #FCD34D;
  --warning-border: #F59E0B;

  --error: #F87171;
  --error-subtle: #450A0A;
  --error-text: #FCA5A5;
  --error-border: #EF4444;

  /* Status */
  --status-live: #34D399;
  --status-live-bg: #052E16;
  --status-live-text: #6EE7B7;

  --status-offline: #756A8A;
  --status-offline-bg: #1A1225;
  --status-offline-text: #B8AECA;

  /* Gradient (adjusted) */
  --gradient-brand: linear-gradient(135deg, #A78BFA 0%, #E879F9 50%, #F472B6 100%);
}
```

---

## 6. Platform Icons

### 6.1 Platform Color Definitions

| Platform | Background | Text Color | Abbreviation |
|----------|------------|------------|--------------|
| YouTube | `#FF0000` | white | YT |
| Twitch | `#9146FF` | white | TW |
| Kick | `#53FC18` | `#000000` | K |
| Facebook | `#1877F2` | white | FB |
| Custom | `var(--primary)` | white | (user defined) |

### 6.2 Stream Icon Component

```typescript
interface StreamIconProps {
  platform: 'youtube' | 'twitch' | 'kick' | 'facebook' | 'custom';
  size?: 'sm' | 'md' | 'lg';  // 24px, 32px, 40px
}
```

**CSS:**
```css
.stream-icon {
  width: 32px;
  height: 32px;
  border-radius: 0.375rem;
  display: flex;
  align-items: center;
  justify-content: center;
  font-weight: 600;
  font-size: 0.75rem;
  color: white;
}

.stream-icon.youtube { background: #FF0000; }
.stream-icon.twitch { background: #9146FF; }
.stream-icon.kick { background: #53FC18; color: #000; }
.stream-icon.facebook { background: #1877F2; }
.stream-icon.custom { background: var(--primary); }
```

### 6.3 Platform Constants

```typescript
const PLATFORMS = {
  youtube: {
    name: 'YouTube',
    abbreviation: 'YT',
    color: '#FF0000',
    textColor: '#FFFFFF',
    defaultServer: 'rtmp://a.rtmp.youtube.com/live2'
  },
  twitch: {
    name: 'Twitch',
    abbreviation: 'TW',
    color: '#9146FF',
    textColor: '#FFFFFF',
    defaultServer: 'rtmp://live.twitch.tv/app'
  },
  kick: {
    name: 'Kick',
    abbreviation: 'K',
    color: '#53FC18',
    textColor: '#000000',
    defaultServer: 'rtmps://fa723fc1b171.global-contribute.live-video.net/app'
  },
  facebook: {
    name: 'Facebook Live',
    abbreviation: 'FB',
    color: '#1877F2',
    textColor: '#FFFFFF',
    defaultServer: 'rtmps://live-api-s.facebook.com:443/rtmp'
  },
  custom: {
    name: 'Custom RTMP',
    abbreviation: 'RT',
    color: 'var(--primary)',
    textColor: '#FFFFFF',
    defaultServer: ''
  }
} as const;
```

---

## 7. Interactive States

### 7.1 Streaming vs Not Streaming

#### Header Button States

**Not Streaming:**
```jsx
<button className="btn btn-primary">
  <PlayIcon /> Start Stream
</button>
```

**Streaming:**
```jsx
<button className="btn btn-destructive">
  <SquareIcon /> Stop Stream
</button>
```

#### Status Badge States

**Not Streaming:**
```jsx
<span className="badge badge-offline">
  <span className="badge-dot"></span>
  Offline  // or "Ready"
</span>
```

**Streaming:**
```jsx
<span className="badge badge-live">
  <span className="badge-dot"></span>  {/* Animated pulse */}
  Live
</span>
```

#### Dashboard Stats (Streaming)

| Stat | Not Streaming | Streaming |
|------|---------------|-----------|
| Active Streams | 0 | 3 (or actual count) |
| Total Bitrate | 0 kbps | 18000 kbps (sum) |
| Dropped Frames | 0 | (actual count) |
| Uptime | 00:00:00 | HH:MM:SS (counting) |

#### Stream Card Stats (Streaming)

| Stat | Not Streaming | Streaming |
|------|---------------|-----------|
| Viewers | 0 | (actual) |
| Bitrate | -- | 6000 |
| FPS | -- | 60 |

### 7.2 State Management

```typescript
interface StreamingState {
  isStreaming: boolean;
  streamStartTime: Date | null;
  activeTargets: string[];  // target IDs that are live
  stats: {
    totalBitrate: number;
    droppedFrames: number;
    uptime: string;
  };
  targetStats: Record<string, {
    viewers: number;
    bitrate: number;
    fps: number;
    status: 'offline' | 'live' | 'error';
  }>;
}
```

### 7.3 UI Updates on Stream Toggle

```typescript
function toggleStream() {
  if (!isStreaming) {
    // Start streaming
    setIsStreaming(true);
    startUptimeTimer();
    updateTargetStatuses('live');
    updateDashboardStats();
  } else {
    // Stop streaming
    setIsStreaming(false);
    stopUptimeTimer();
    updateTargetStatuses('offline');
    resetDashboardStats();
  }
}
```

---

## 8. Animations

### 8.1 Transitions

```css
/* Default transition for interactive elements */
transition: all 0.15s;

/* Slower transition for theme/mode changes */
transition: all 0.2s;

/* Toggle slider */
transition: 0.2s;
```

### 8.2 Keyframe Animations

```css
/* Live badge pulse */
@keyframes pulse {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.5; }
}
.badge-live .badge-dot {
  animation: pulse 2s infinite;
}
```

### 8.3 Modal Animation

```css
/* Overlay fade in */
.modal-overlay {
  opacity: 0;
  visibility: hidden;
  transition: all 0.2s;
}
.modal-overlay.active {
  opacity: 1;
  visibility: visible;
}

/* Modal scale in */
.modal {
  transform: scale(0.95);
  transition: transform 0.2s;
}
.modal-overlay.active .modal {
  transform: scale(1);
}
```

### 8.4 Hover Effects

```css
/* Card lift */
.profile-card:hover {
  transform: translateY(-2px);
  box-shadow: var(--shadow-md);
}

/* Stream card border highlight */
.stream-card:hover {
  border-color: var(--border-interactive);
  box-shadow: var(--shadow-md);
}
```

### 8.5 Accordion Animation

```css
.output-group-chevron {
  transition: transform 0.2s;
}
.output-group.expanded .output-group-chevron {
  transform: rotate(180deg);
}
```

---

## 9. Icon Reference (Lucide)

### Navigation Icons
- `layout-dashboard` - Dashboard
- `user-circle` - Profiles
- `radio` - Stream Manager
- `settings-2` - Encoder Settings
- `share-2` - Output Groups
- `target` - Stream Targets
- `file-text` - Logs
- `cog` - Settings

### Action Icons
- `play` - Start/Play
- `square` - Stop
- `plus` - Add/Create
- `pencil` - Edit
- `copy` - Duplicate
- `trash-2` - Delete
- `upload` - Import
- `download` - Export
- `eye` - Show (password)
- `x` - Close
- `bell` - Notifications
- `refresh-cw` - Updates

### Status Icons
- `activity` - Bitrate
- `alert-triangle` - Dropped Frames/Warning
- `clock` - Uptime
- `monitor` - Resolution
- `gauge` - Bitrate (in cards)
- `layers` - Output Groups
- `chevron-down` - Expand/Collapse
- `info` - Info alert

### External Icons
- `github` - GitHub link
- `book-open` - Documentation
- `play-circle` - Test Stream

---

## 10. Accessibility Requirements

### 10.1 Focus States

```css
.btn:focus-visible {
  outline: 3px solid var(--primary);
  outline-offset: 2px;
}

.form-input:focus {
  outline: none;
  border-color: var(--border-interactive);
  box-shadow: 0 0 0 3px var(--primary-muted);
}
```

### 10.2 Color Contrast

- All text colors meet WCAG AA contrast requirements
- Status colors have sufficient contrast in both themes
- Interactive elements have clear visual distinction

### 10.3 Keyboard Navigation

- All interactive elements are focusable
- Tab order follows visual layout
- Escape closes modals
- Enter/Space activates buttons

### 10.4 Screen Reader Considerations

- Use semantic HTML (`<nav>`, `<main>`, `<header>`, etc.)
- Add `aria-label` to icon-only buttons
- Use `aria-expanded` for accordions
- Use `aria-hidden` for decorative icons

---

## 11. Responsive Behavior

### 11.1 Breakpoints

```css
/* Desktop (default) */
/* All features available */

/* Tablet (max-width: 1200px) */
.grid-4, .grid-3 { grid-template-columns: repeat(2, 1fr); }

/* Mobile (max-width: 768px) */
.grid-4, .grid-3, .grid-2 { grid-template-columns: 1fr; }
/* Consider collapsible sidebar */
```

### 11.2 Mobile Considerations (Future)

- Collapsible sidebar with hamburger menu
- Full-width cards
- Stacked form layouts
- Bottom sheet modals
- Touch-friendly button sizes (min 44px)

---

## 12. Component File Structure (Recommended)

```
src/components/
  layout/
    AppShell.tsx
    Sidebar.tsx
    Header.tsx
    MainContent.tsx
  navigation/
    NavItem.tsx
    NavBadge.tsx
    NavSection.tsx
  cards/
    Card.tsx
    StatBox.tsx
    ProfileCard.tsx
    StreamCard.tsx
    OutputGroupCard.tsx
  forms/
    FormGroup.tsx
    FormLabel.tsx
    FormInput.tsx
    FormSelect.tsx
    Toggle.tsx
  buttons/
    Button.tsx
  feedback/
    Alert.tsx
    Badge.tsx
    Modal.tsx
  data-display/
    LogConsole.tsx
    OutputGroupAccordion.tsx
  platform/
    StreamIcon.tsx
  views/
    Dashboard.tsx
    Profiles.tsx
    StreamManager.tsx
    EncoderSettings.tsx
    OutputGroups.tsx
    StreamTargets.tsx
    Logs.tsx
    Settings.tsx
```

---

*Document Version: 1.0*
*Based on: magillastream-mockup.html*
*Last Updated: 2026-01-01*
