# Component Library

[Documentation](../README.md) > [Frontend](./README.md) > Component Library

---

This document catalogs SpiritStream's React component library, covering base components, patterns, and usage examples.

---

## Overview

The component library is built with:
- React 18+
- Tailwind CSS v4
- CSS custom properties (design tokens)
- Lucide icons

```
src-frontend/components/
├── ui/              # Base components
├── layout/          # App structure
├── navigation/      # Nav components
├── profile/         # Profile features
├── stream/          # Streaming features
└── settings/        # Settings panels
```

---

## Base Components

### Button

Multi-variant button component.

```typescript
// components/ui/Button.tsx
interface ButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: 'primary' | 'secondary' | 'ghost' | 'outline' | 'destructive';
  size?: 'sm' | 'md' | 'lg' | 'icon';
  loading?: boolean;
}
```

**Variants:**

| Variant | Use Case |
|---------|----------|
| `primary` | Main actions (Start Stream, Save) |
| `secondary` | Alternative actions |
| `ghost` | Subtle actions (icon buttons) |
| `outline` | Secondary emphasis |
| `destructive` | Delete, Stop actions |

**Usage:**

```tsx
<Button variant="primary" onClick={handleStart}>
  Start Stream
</Button>

<Button variant="destructive" size="sm">
  <Trash2 className="w-4 h-4" />
  Delete
</Button>

<Button variant="ghost" size="icon">
  <Settings className="w-4 h-4" />
</Button>
```

---

### Card

Container component with header, body, and footer sections.

```typescript
// components/ui/Card.tsx
interface CardProps {
  children: React.ReactNode;
  className?: string;
  variant?: 'default' | 'elevated' | 'interactive';
}
```

**Sub-components:**
- `CardHeader` - Title and actions
- `CardTitle` - Heading text
- `CardDescription` - Subtitle text
- `CardBody` - Main content
- `CardFooter` - Action buttons

**Usage:**

```tsx
<Card>
  <CardHeader>
    <CardTitle>Output Group</CardTitle>
    <CardDescription>Configure encoding settings</CardDescription>
  </CardHeader>
  <CardBody>
    <Form>...</Form>
  </CardBody>
  <CardFooter>
    <Button variant="ghost">Cancel</Button>
    <Button variant="primary">Save</Button>
  </CardFooter>
</Card>
```

---

### Input

Text input with label, helper text, and error states.

```typescript
// components/ui/Input.tsx
interface InputProps extends React.InputHTMLAttributes<HTMLInputElement> {
  label?: string;
  error?: string;
  helper?: string;
}
```

**Usage:**

```tsx
<Input
  label="Profile Name"
  placeholder="Enter profile name"
  value={name}
  onChange={(e) => setName(e.target.value)}
  error={errors.name}
/>

<Input
  label="Video Bitrate"
  type="number"
  min={1000}
  max={50000}
  helper="Recommended: 6000 kbps for 1080p60"
/>
```

---

### Select

Dropdown selection component.

```typescript
// components/ui/Select.tsx
interface SelectProps extends React.SelectHTMLAttributes<HTMLSelectElement> {
  label?: string;
  options: { value: string; label: string }[];
  error?: string;
  helper?: string;
}
```

**Usage:**

```tsx
<Select
  label="Video Encoder"
  value={encoder}
  onChange={(e) => setEncoder(e.target.value)}
  options={[
    { value: 'libx264', label: 'x264 (CPU)' },
    { value: 'h264_nvenc', label: 'NVENC (NVIDIA)' },
    { value: 'h264_qsv', label: 'QuickSync (Intel)' },
  ]}
/>
```

---

### Toggle

Switch component for boolean settings.

```typescript
// components/ui/Toggle.tsx
interface ToggleProps {
  checked: boolean;
  onChange: (checked: boolean) => void;
  label?: string;
  disabled?: boolean;
}
```

**Usage:**

```tsx
<Toggle
  checked={settings.startMinimized}
  onChange={(checked) => updateSetting('startMinimized', checked)}
  label="Start minimized"
/>
```

---

### Modal

Dialog overlay component.

```typescript
// components/ui/Modal.tsx
interface ModalProps {
  open: boolean;
  onClose: () => void;
  title: string;
  children: React.ReactNode;
  footer?: React.ReactNode;
  maxWidth?: string;
}
```

**Usage:**

```tsx
<Modal
  open={isOpen}
  onClose={() => setIsOpen(false)}
  title="Create Profile"
  footer={
    <>
      <Button variant="ghost" onClick={() => setIsOpen(false)}>
        Cancel
      </Button>
      <Button variant="primary" onClick={handleSave}>
        Create
      </Button>
    </>
  }
>
  <ProfileForm />
</Modal>
```

---

## Layout Components

### AppShell

Root layout combining sidebar and main content.

```tsx
// components/layout/AppShell.tsx
function AppShell({ children }: { children: React.ReactNode }) {
  return (
    <div className="flex min-h-screen bg-[var(--bg-base)]">
      {children}
    </div>
  );
}
```

### Sidebar

Fixed navigation sidebar.

```tsx
// components/layout/Sidebar.tsx
<Sidebar>
  <SidebarHeader>
    <Logo />
  </SidebarHeader>
  <SidebarNav>
    <NavSection title="Main">
      <NavItem icon={<LayoutDashboard />} label="Dashboard" />
      <NavItem icon={<User />} label="Profiles" />
    </NavSection>
  </SidebarNav>
  <SidebarFooter>
    <ThemeToggle />
  </SidebarFooter>
</Sidebar>
```

### MainContent

Content area with header.

```tsx
// components/layout/MainContent.tsx
<MainContent>
  <Header title="Dashboard">
    <Button>New Profile</Button>
  </Header>
  <ContentArea>
    {/* Page content */}
  </ContentArea>
</MainContent>
```

---

## Navigation Components

### NavItem

Sidebar navigation item.

```typescript
// components/navigation/NavItem.tsx
interface NavItemProps {
  icon: React.ReactNode;
  label: string;
  active?: boolean;
  badge?: number;
  onClick?: () => void;
}
```

**Usage:**

```tsx
<NavItem
  icon={<Radio />}
  label="Stream Manager"
  active={currentView === 'streams'}
  badge={activeStreamCount}
  onClick={() => setView('streams')}
/>
```

### NavSection

Grouped navigation section.

```tsx
<NavSection title="Configuration">
  <NavItem icon={<Settings2 />} label="Encoder" />
  <NavItem icon={<Share2 />} label="Outputs" />
  <NavItem icon={<Target />} label="Targets" />
</NavSection>
```

---

## Stream Components

### StreamStatus

Status indicator badge.

```typescript
// components/ui/StreamStatus.tsx
interface StreamStatusProps {
  status: 'live' | 'connecting' | 'offline' | 'error';
  label?: string;
  showPulse?: boolean;
}
```

**Usage:**

```tsx
<StreamStatus status="live" />
<StreamStatus status="offline" label="Ready" />
<StreamStatus status="error" label="Connection failed" />
```

### PlatformIcon

Platform brand icon.

```typescript
// components/stream/PlatformIcon.tsx
interface PlatformIconProps {
  platform: 'youtube' | 'twitch' | 'kick' | 'facebook' | 'custom';
  size?: 'sm' | 'md' | 'lg';
}
```

**Usage:**

```tsx
<PlatformIcon platform="twitch" size="md" />
```

### StreamCard

Stream target display card.

```tsx
<StreamCard
  platform="youtube"
  name="YouTube Gaming"
  status="live"
  stats={[
    { label: 'Viewers', value: '1,234' },
    { label: 'Bitrate', value: '6000' },
    { label: 'FPS', value: '60' },
  ]}
/>
```

### OutputGroup

Expandable output group accordion.

```tsx
<OutputGroup
  name="Main Output"
  info="3 targets • 1080p60 • 6000 kbps"
  status="live"
  defaultExpanded
>
  <StreamCard platform="youtube" ... />
  <StreamCard platform="twitch" ... />
</OutputGroup>
```

---

## Dashboard Components

### StatBox

Statistics display component.

```typescript
// components/dashboard/StatBox.tsx
interface StatBoxProps {
  icon: React.ReactNode;
  label: string;
  value: string | number;
  change?: string;
  changeType?: 'positive' | 'neutral';
}
```

**Usage:**

```tsx
<StatBox
  icon={<Radio />}
  label="Active Streams"
  value={3}
  change="All targets live"
  changeType="positive"
/>
```

### ProfileCard

Profile selection card.

```tsx
<ProfileCard
  name="Gaming Stream"
  meta={[
    { icon: <Monitor />, label: '1080p60' },
    { icon: <Gauge />, label: '6000 kbps' },
    { icon: <Target />, label: '3 targets' },
  ]}
  active={isActive}
  onClick={() => selectProfile(id)}
/>
```

---

## Form Components

### FormGroup

Form field wrapper.

```tsx
<FormGroup>
  <FormLabel htmlFor="bitrate">Video Bitrate</FormLabel>
  <Input id="bitrate" type="number" />
  <FormHelper>Recommended: 6000 kbps</FormHelper>
</FormGroup>
```

### FormRow

Horizontal form layout.

```tsx
<FormRow>
  <Input label="Width" />
  <Input label="Height" />
</FormRow>
```

---

## Feedback Components

### Alert

Alert banner component.

```typescript
// components/ui/Alert.tsx
interface AlertProps {
  variant: 'info' | 'success' | 'warning' | 'error';
  title?: string;
  children: React.ReactNode;
}
```

**Usage:**

```tsx
<Alert variant="info" title="No Active Streams">
  Start streaming to see live statistics.
</Alert>

<Alert variant="error" title="Connection Failed">
  Unable to connect to YouTube. Check your stream key.
</Alert>
```

### LogConsole

Log display component.

```tsx
<LogConsole maxHeight="300px">
  <LogEntry time="14:32:01" level="info" message="Stream started" />
  <LogEntry time="14:32:02" level="warn" message="Dropped 3 frames" />
  <LogEntry time="14:32:05" level="error" message="Connection lost" />
</LogConsole>
```

---

## Utility Components

### Grid

Responsive grid layout.

```tsx
<Grid cols={3} gap="md">
  <ProfileCard ... />
  <ProfileCard ... />
  <ProfileCard ... />
</Grid>
```

### Spinner

Loading indicator.

```tsx
<Spinner size="sm" />
<Button loading>Saving...</Button>
```

### ThemeToggle

Light/dark mode switch.

```tsx
<ThemeToggle />
```

---

## Component Patterns

### Composition

```tsx
<Card>
  <CardHeader>
    <CardTitle>Settings</CardTitle>
  </CardHeader>
  <CardBody>
    <FormGroup>
      <Toggle label="Enable notifications" />
    </FormGroup>
  </CardBody>
</Card>
```

### Controlled vs Uncontrolled

```tsx
// Controlled (recommended)
<Input value={value} onChange={(e) => setValue(e.target.value)} />

// Uncontrolled
<Input defaultValue="initial" ref={inputRef} />
```

### Forwarding Refs

```tsx
const Button = forwardRef<HTMLButtonElement, ButtonProps>(
  ({ children, ...props }, ref) => (
    <button ref={ref} {...props}>{children}</button>
  )
);
```

---

## Styling Patterns

### Using Design Tokens

```tsx
<div className="bg-[var(--bg-surface)] border border-[var(--border-default)]">
  <span className="text-[var(--text-primary)]">Content</span>
</div>
```

### Conditional Styles

```tsx
import { cn } from '@/lib/cn';

<button
  className={cn(
    'px-4 py-2 rounded-lg',
    active && 'bg-[var(--primary)] text-white',
    disabled && 'opacity-50 cursor-not-allowed'
  )}
/>
```

### Variants with CVA

```tsx
import { cva } from 'class-variance-authority';

const buttonVariants = cva('px-4 py-2 rounded-lg font-medium', {
  variants: {
    variant: {
      primary: 'bg-[var(--primary)] text-white',
      ghost: 'bg-transparent text-[var(--text-secondary)]',
    },
    size: {
      sm: 'text-sm px-3 py-1.5',
      md: 'text-base px-4 py-2',
    },
  },
  defaultVariants: {
    variant: 'primary',
    size: 'md',
  },
});
```

---

## Accessibility

### Focus Management

```tsx
<button
  className={cn(
    'focus-visible:outline-none',
    'focus-visible:ring-[3px] focus-visible:ring-[var(--primary)]',
    'focus-visible:ring-offset-2'
  )}
/>
```

### ARIA Attributes

```tsx
<button
  aria-label="Close dialog"
  aria-expanded={isOpen}
  aria-controls="menu"
/>
```

### Keyboard Navigation

```tsx
<div
  role="listbox"
  tabIndex={0}
  onKeyDown={(e) => {
    if (e.key === 'ArrowDown') selectNext();
    if (e.key === 'ArrowUp') selectPrevious();
    if (e.key === 'Enter') confirmSelection();
  }}
/>
```

---

**Related:** [React Architecture](./01-react-architecture.md) | [Theming & i18n](./05-theming-i18n.md) | [State Management](./02-state-management.md)

