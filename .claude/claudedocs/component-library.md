# Component Library

## Overview

MagillaStream uses a custom component library built with React, Tailwind CSS v4, and the design system tokens. Components are built for accessibility using Radix UI primitives where appropriate.

## Design Principles

1. **Token-first**: All colors, spacing, and effects use design tokens
2. **Accessible**: WCAG 2.2 AA compliant, keyboard navigable
3. **Themeable**: Full light/dark mode support via CSS custom properties
4. **Composable**: Small, focused components that compose well
5. **Type-safe**: Full TypeScript support with strict prop types

## Component Categories

```
components/
├── ui/              # Base UI components
├── layout/          # Layout and structure
├── navigation/      # Navigation components
├── dashboard/       # Dashboard-specific components
├── profile/         # Profile management features
├── stream/          # Streaming features
├── feedback/        # Alerts, logs, and feedback
└── settings/        # Settings and configuration
```

---

## Layout Components

### Sidebar

Fixed-position sidebar with header, navigation, and footer sections.

```tsx
// components/layout/Sidebar.tsx
import { cn } from '@/lib/cn';

interface SidebarProps {
  children: React.ReactNode;
  className?: string;
}

export function Sidebar({ children, className }: SidebarProps) {
  return (
    <aside
      className={cn(
        'w-[260px] bg-[var(--bg-surface)] border-r border-[var(--border-default)]',
        'flex flex-col fixed top-0 left-0 bottom-0 z-[100]',
        className
      )}
    >
      {children}
    </aside>
  );
}

export function SidebarHeader({ children, className }: { children: React.ReactNode; className?: string }) {
  return (
    <div
      className={cn(
        'px-4 py-5 border-b border-[var(--border-muted)]',
        'flex items-center gap-3',
        className
      )}
    >
      {children}
    </div>
  );
}

export function SidebarNav({ children, className }: { children: React.ReactNode; className?: string }) {
  return (
    <nav className={cn('flex-1 px-3 py-4 overflow-y-auto', className)}>
      {children}
    </nav>
  );
}

export function SidebarFooter({ children, className }: { children: React.ReactNode; className?: string }) {
  return (
    <div className={cn('p-4 border-t border-[var(--border-muted)]', className)}>
      {children}
    </div>
  );
}
```

### Logo

Gradient logo component with icon.

```tsx
// components/layout/Logo.tsx
import { cn } from '@/lib/cn';

interface LogoProps {
  size?: 'sm' | 'md' | 'lg';
  showText?: boolean;
}

export function Logo({ size = 'md', showText = true }: LogoProps) {
  const sizes = {
    sm: 'w-8 h-8 text-base',
    md: 'w-10 h-10 text-xl',
    lg: 'w-12 h-12 text-2xl',
  };

  return (
    <div className="flex items-center gap-3">
      <div
        className={cn(
          'bg-[var(--gradient-brand)] rounded-[10px]',
          'flex items-center justify-center',
          'text-white font-bold shadow-[var(--shadow-md)]',
          sizes[size]
        )}
      >
        M
      </div>
      {showText && (
        <span
          className="font-bold text-lg bg-[var(--gradient-brand)] bg-clip-text"
          style={{ WebkitTextFillColor: 'transparent' }}
        >
          MagillaStream
        </span>
      )}
    </div>
  );
}
```

### Header

Sticky page header with title and action buttons.

```tsx
// components/layout/Header.tsx
import { cn } from '@/lib/cn';

interface HeaderProps {
  title: string;
  children?: React.ReactNode;
  className?: string;
}

export function Header({ title, children, className }: HeaderProps) {
  return (
    <header
      className={cn(
        'bg-[var(--bg-surface)] border-b border-[var(--border-default)]',
        'px-6 py-4 flex items-center justify-between',
        'sticky top-0 z-50',
        className
      )}
    >
      <div className="flex items-center gap-4">
        <h1 className="text-xl font-semibold text-[var(--text-primary)]">{title}</h1>
      </div>
      {children && (
        <div className="flex items-center gap-3">{children}</div>
      )}
    </header>
  );
}
```

### MainContent

Main content wrapper that accounts for sidebar offset.

```tsx
// components/layout/MainContent.tsx
import { cn } from '@/lib/cn';

interface MainContentProps {
  children: React.ReactNode;
  className?: string;
}

export function MainContent({ children, className }: MainContentProps) {
  return (
    <main className={cn('flex-1 ml-[260px] flex flex-col', className)}>
      {children}
    </main>
  );
}

export function ContentArea({ children, className }: { children: React.ReactNode; className?: string }) {
  return (
    <div className={cn('flex-1 p-6 overflow-y-auto', className)}>
      {children}
    </div>
  );
}
```

### AppShell

Complete app shell combining sidebar and main content.

```tsx
// components/layout/AppShell.tsx
import { cn } from '@/lib/cn';

interface AppShellProps {
  children: React.ReactNode;
  className?: string;
}

export function AppShell({ children, className }: AppShellProps) {
  return (
    <div className={cn('flex min-h-screen', className)}>
      {children}
    </div>
  );
}
```

---

## Navigation Components

### NavSection

Grouped navigation section with title.

```tsx
// components/navigation/NavSection.tsx
import { cn } from '@/lib/cn';

interface NavSectionProps {
  title: string;
  children: React.ReactNode;
  className?: string;
}

export function NavSection({ title, children, className }: NavSectionProps) {
  return (
    <div className={cn('mb-6', className)}>
      <div
        className={cn(
          'text-[0.6875rem] font-semibold uppercase tracking-wider',
          'text-[var(--text-tertiary)] px-3 mb-2'
        )}
      >
        {title}
      </div>
      <div className="space-y-1">{children}</div>
    </div>
  );
}
```

### NavItem

Navigation item with icon, label, and optional badge.

```tsx
// components/navigation/NavItem.tsx
import { cn } from '@/lib/cn';

interface NavItemProps {
  icon: React.ReactNode;
  label: string;
  active?: boolean;
  badge?: number;
  onClick?: () => void;
}

export function NavItem({ icon, label, active, badge, onClick }: NavItemProps) {
  return (
    <button
      onClick={onClick}
      className={cn(
        'w-full flex items-center gap-3 px-3 py-2.5 rounded-lg',
        'text-sm font-medium transition-all duration-150',
        'border-none bg-transparent text-left cursor-pointer',
        active
          ? 'bg-[var(--primary-subtle)] text-[var(--primary)]'
          : 'text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]'
      )}
    >
      <span className="w-5 h-5 flex items-center justify-center">{icon}</span>
      <span className="flex-1">{label}</span>
      {badge !== undefined && <NavBadge count={badge} />}
    </button>
  );
}
```

### NavBadge

Count badge for navigation items.

```tsx
// components/navigation/NavBadge.tsx
import { cn } from '@/lib/cn';

interface NavBadgeProps {
  count: number;
  className?: string;
}

export function NavBadge({ count, className }: NavBadgeProps) {
  return (
    <span
      className={cn(
        'bg-[var(--primary)] text-white',
        'text-[0.6875rem] font-semibold',
        'px-2 py-0.5 rounded-full',
        className
      )}
    >
      {count}
    </span>
  );
}
```

---

## Dashboard Components

### StatBox

Statistics display box with icon, label, value, and change indicator.

```tsx
// components/dashboard/StatBox.tsx
import { cn } from '@/lib/cn';

interface StatBoxProps {
  icon: React.ReactNode;
  label: string;
  value: string | number;
  change?: string;
  changeType?: 'positive' | 'neutral';
  className?: string;
}

export function StatBox({ icon, label, value, change, changeType = 'neutral', className }: StatBoxProps) {
  return (
    <div
      className={cn(
        'bg-[var(--bg-surface)] border border-[var(--border-default)]',
        'rounded-xl p-5',
        className
      )}
    >
      <div className="flex items-center justify-between mb-2">
        <span className="text-[0.8125rem] text-[var(--text-secondary)]">{label}</span>
        <span className="text-[var(--text-tertiary)]">{icon}</span>
      </div>
      <div className="text-2xl font-bold text-[var(--text-primary)]">{value}</div>
      {change && (
        <div
          className={cn(
            'text-xs mt-1',
            changeType === 'positive' ? 'text-[var(--success-text)]' : 'text-[var(--text-tertiary)]'
          )}
        >
          {change}
        </div>
      )}
    </div>
  );
}
```

### StatsRow

Container for a row of StatBox components.

```tsx
// components/dashboard/StatsRow.tsx
import { cn } from '@/lib/cn';

interface StatsRowProps {
  children: React.ReactNode;
  className?: string;
}

export function StatsRow({ children, className }: StatsRowProps) {
  return (
    <div className={cn('grid grid-cols-4 gap-4 mb-6', className)}>
      {children}
    </div>
  );
}
```

### ProfileCard

Profile selection card with name, meta info, and active state.

```tsx
// components/dashboard/ProfileCard.tsx
import { cn } from '@/lib/cn';

interface ProfileCardMeta {
  icon: React.ReactNode;
  label: string;
}

interface ProfileCardProps {
  name: string;
  meta: ProfileCardMeta[];
  active?: boolean;
  onClick?: () => void;
  actions?: React.ReactNode;
  className?: string;
}

export function ProfileCard({ name, meta, active, onClick, actions, className }: ProfileCardProps) {
  return (
    <div
      onClick={onClick}
      className={cn(
        'bg-[var(--bg-surface)] border-2 rounded-xl p-5 cursor-pointer',
        'transition-all duration-150',
        active
          ? 'border-[var(--primary)] bg-[var(--primary-muted)]'
          : 'border-[var(--border-default)] hover:border-[var(--border-interactive)] hover:-translate-y-0.5 hover:shadow-[var(--shadow-md)]',
        className
      )}
    >
      <div className="flex items-center justify-between mb-3">
        <span className="font-semibold text-[var(--text-primary)]">{name}</span>
        {active && (
          <StreamStatus status="live" label="Active" />
        )}
        {actions}
      </div>
      <div className="flex gap-4 text-[0.8125rem] text-[var(--text-secondary)]">
        {meta.map((item, index) => (
          <span key={index} className="flex items-center gap-1">
            {item.icon}
            {item.label}
          </span>
        ))}
      </div>
    </div>
  );
}
```

### StreamCard

Stream target card with platform icon, name, status, and stats.

```tsx
// components/dashboard/StreamCard.tsx
import { cn } from '@/lib/cn';
import { StreamStatus } from '@/components/ui/StreamStatus';
import { PlatformIcon } from '@/components/stream/PlatformIcon';

interface StreamStat {
  label: string;
  value: string | number;
}

interface StreamCardProps {
  platform: 'youtube' | 'twitch' | 'kick' | 'facebook' | 'custom';
  name: string;
  status: 'live' | 'offline' | 'error';
  stats?: StreamStat[];
  onClick?: () => void;
  className?: string;
}

export function StreamCard({ platform, name, status, stats, onClick, className }: StreamCardProps) {
  return (
    <div
      onClick={onClick}
      className={cn(
        'bg-[var(--bg-surface)] border border-[var(--border-default)]',
        'rounded-xl p-4 transition-all duration-150',
        'hover:border-[var(--border-interactive)] hover:shadow-[var(--shadow-md)]',
        onClick && 'cursor-pointer',
        className
      )}
    >
      <div className="flex items-center justify-between mb-3">
        <div className="flex items-center gap-2">
          <PlatformIcon platform={platform} />
          <span className="font-semibold text-sm text-[var(--text-primary)]">{name}</span>
        </div>
        <StreamStatus status={status} />
      </div>
      {stats && stats.length > 0 && (
        <div className="grid grid-cols-3 gap-3 pt-3 border-t border-[var(--border-muted)]">
          {stats.map((stat, index) => (
            <div key={index} className="text-center">
              <div className="text-sm font-semibold text-[var(--text-primary)]">{stat.value}</div>
              <div className="text-[0.6875rem] uppercase text-[var(--text-tertiary)]">{stat.label}</div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
```

---

## Stream Components

### OutputGroup

Accordion-style output group with header and expandable targets.

```tsx
// components/stream/OutputGroup.tsx
import { useState } from 'react';
import { ChevronDownIcon, Layers } from 'lucide-react';
import { cn } from '@/lib/cn';
import { StreamStatus } from '@/components/ui/StreamStatus';

interface OutputGroupProps {
  name: string;
  info: string;
  status: 'live' | 'offline' | 'error';
  defaultExpanded?: boolean;
  children: React.ReactNode;
  className?: string;
}

export function OutputGroup({
  name,
  info,
  status,
  defaultExpanded = false,
  children,
  className
}: OutputGroupProps) {
  const [expanded, setExpanded] = useState(defaultExpanded);

  return (
    <div
      className={cn(
        'bg-[var(--bg-muted)] border border-[var(--border-default)]',
        'rounded-xl mb-4',
        className
      )}
    >
      <div
        className="p-4 px-5 flex items-center justify-between cursor-pointer"
        onClick={() => setExpanded(!expanded)}
      >
        <div className="flex items-center gap-3">
          <Layers className="w-[18px] h-[18px] text-[var(--primary)]" />
          <div>
            <div className="font-semibold text-[var(--text-primary)]">{name}</div>
            <div className="text-[0.8125rem] text-[var(--text-secondary)]">{info}</div>
          </div>
        </div>
        <div className="flex items-center gap-3">
          <StreamStatus status={status} label={status === 'offline' ? 'Ready' : undefined} />
          <ChevronDownIcon
            className={cn(
              'w-[18px] h-[18px] text-[var(--text-tertiary)] transition-transform duration-200',
              expanded && 'rotate-180'
            )}
          />
        </div>
      </div>
      {expanded && (
        <div className="px-5 pb-5">
          {children}
        </div>
      )}
    </div>
  );
}
```

### StreamStatus

Status badge with dot indicator and pulse animation.

```tsx
// components/ui/StreamStatus.tsx
import { cn } from '@/lib/cn';

type Status = 'live' | 'connecting' | 'offline' | 'error';

interface StreamStatusProps {
  status: Status;
  label?: string;
  showPulse?: boolean;
}

export function StreamStatus({ status, label, showPulse = true }: StreamStatusProps) {
  const config = {
    live: {
      bg: 'bg-[var(--status-live-bg)]',
      text: 'text-[var(--status-live-text)]',
      dot: 'bg-[var(--status-live)]',
      pulse: showPulse,
      defaultLabel: 'Live',
    },
    connecting: {
      bg: 'bg-[var(--status-connecting-bg)]',
      text: 'text-[var(--status-connecting-text)]',
      dot: 'bg-[var(--status-connecting)]',
      pulse: showPulse,
      defaultLabel: 'Connecting',
    },
    offline: {
      bg: 'bg-[var(--status-offline-bg)]',
      text: 'text-[var(--status-offline-text)]',
      dot: 'bg-[var(--status-offline)]',
      pulse: false,
      defaultLabel: 'Offline',
    },
    error: {
      bg: 'bg-[var(--error-subtle)]',
      text: 'text-[var(--error-text)]',
      dot: 'bg-[var(--error)]',
      pulse: false,
      defaultLabel: 'Error',
    },
  };

  const { bg, text, dot, pulse, defaultLabel } = config[status];

  return (
    <span
      className={cn(
        'inline-flex items-center gap-1.5 px-2.5 py-1 rounded-full text-xs font-medium',
        bg,
        text
      )}
    >
      <span
        className={cn(
          'w-1.5 h-1.5 rounded-full',
          dot,
          pulse && 'animate-pulse'
        )}
      />
      {label || defaultLabel}
    </span>
  );
}
```

### PlatformIcon

Platform-specific icon with brand colors.

```tsx
// components/stream/PlatformIcon.tsx
import { cn } from '@/lib/cn';

type Platform = 'youtube' | 'twitch' | 'kick' | 'facebook' | 'custom';

interface PlatformIconProps {
  platform: Platform;
  size?: 'sm' | 'md' | 'lg';
  className?: string;
}

export function PlatformIcon({ platform, size = 'md', className }: PlatformIconProps) {
  const sizes = {
    sm: 'w-6 h-6 text-xs',
    md: 'w-8 h-8 text-xs',
    lg: 'w-10 h-10 text-sm',
  };

  const platforms = {
    youtube: { bg: 'bg-[#FF0000]', text: 'text-white', label: 'YT' },
    twitch: { bg: 'bg-[#9146FF]', text: 'text-white', label: 'TW' },
    kick: { bg: 'bg-[#53FC18]', text: 'text-black', label: 'K' },
    facebook: { bg: 'bg-[#1877F2]', text: 'text-white', label: 'FB' },
    custom: { bg: 'bg-[var(--primary)]', text: 'text-white', label: 'C' },
  };

  const { bg, text, label } = platforms[platform];

  return (
    <div
      className={cn(
        'rounded-md flex items-center justify-center font-semibold',
        bg,
        text,
        sizes[size],
        className
      )}
    >
      {label}
    </div>
  );
}
```

---

## Base UI Components

### Button

```tsx
// components/ui/Button.tsx
import { forwardRef } from 'react';
import { cn } from '@/lib/cn';

interface ButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: 'primary' | 'secondary' | 'accent' | 'ghost' | 'outline' | 'destructive';
  size?: 'sm' | 'md' | 'lg' | 'icon';
  loading?: boolean;
}

export const Button = forwardRef<HTMLButtonElement, ButtonProps>(
  ({ className, variant = 'primary', size = 'md', loading, children, disabled, ...props }, ref) => {
    const variants = {
      primary: 'bg-[var(--primary)] text-white hover:bg-[var(--primary-hover)]',
      secondary: 'bg-[var(--secondary)] text-white hover:opacity-90',
      accent: 'bg-[var(--accent)] text-white hover:opacity-90',
      ghost: 'bg-transparent text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]',
      outline: 'bg-transparent border-2 border-[var(--primary)] text-[var(--primary)] hover:bg-[var(--primary-subtle)]',
      destructive: 'bg-[var(--error)] text-white hover:opacity-90',
    };

    const sizes = {
      sm: 'h-8 px-3 text-[0.8125rem]',
      md: 'h-10 px-4 text-sm',
      lg: 'h-12 px-6 text-base',
      icon: 'w-9 h-9 p-0',
    };

    return (
      <button
        ref={ref}
        className={cn(
          'inline-flex items-center justify-center gap-2 font-medium rounded-lg transition-all duration-150',
          'focus-visible:outline-none focus-visible:ring-[3px] focus-visible:ring-[var(--primary)]',
          'focus-visible:ring-offset-2',
          'disabled:opacity-50 disabled:cursor-not-allowed',
          'border-none cursor-pointer font-[inherit]',
          variants[variant],
          sizes[size],
          className
        )}
        disabled={disabled || loading}
        {...props}
      >
        {loading && <Spinner className="w-4 h-4" />}
        {children}
      </button>
    );
  }
);
```

### Card

```tsx
// components/ui/Card.tsx
import { cn } from '@/lib/cn';

interface CardProps extends React.HTMLAttributes<HTMLDivElement> {
  variant?: 'default' | 'elevated' | 'interactive';
}

export function Card({ className, variant = 'default', ...props }: CardProps) {
  const variants = {
    default: 'bg-[var(--bg-surface)] shadow-[var(--shadow-sm)]',
    elevated: 'bg-[var(--bg-elevated)] shadow-[var(--shadow-md)]',
    interactive: 'bg-[var(--bg-surface)] shadow-[var(--shadow-sm)] hover:shadow-[var(--shadow-md)] hover:border-[var(--border-interactive)] cursor-pointer transition-all',
  };

  return (
    <div
      className={cn(
        'rounded-xl border border-[var(--border-default)]',
        variants[variant],
        className
      )}
      {...props}
    />
  );
}

export function CardHeader({ className, ...props }: React.HTMLAttributes<HTMLDivElement>) {
  return (
    <div
      className={cn(
        'px-6 py-5 border-b border-[var(--border-muted)]',
        'flex items-center justify-between',
        className
      )}
      {...props}
    />
  );
}

export function CardTitle({ className, ...props }: React.HTMLAttributes<HTMLHeadingElement>) {
  return (
    <h3 className={cn('text-base font-semibold text-[var(--text-primary)]', className)} {...props} />
  );
}

export function CardDescription({ className, ...props }: React.HTMLAttributes<HTMLParagraphElement>) {
  return (
    <p className={cn('text-sm text-[var(--text-secondary)] mt-1', className)} {...props} />
  );
}

export function CardBody({ className, ...props }: React.HTMLAttributes<HTMLDivElement>) {
  return <div className={cn('p-6', className)} {...props} />;
}

export function CardFooter({ className, ...props }: React.HTMLAttributes<HTMLDivElement>) {
  return (
    <div
      className={cn(
        'px-6 py-4 border-t border-[var(--border-muted)] bg-[var(--bg-muted)] rounded-b-xl',
        'flex justify-end gap-3',
        className
      )}
      {...props}
    />
  );
}
```

---

## Form Components

### FormGroup, FormLabel, FormHelper

Form layout components for consistent form structure.

```tsx
// components/ui/Form.tsx
import { cn } from '@/lib/cn';

export function FormGroup({ className, ...props }: React.HTMLAttributes<HTMLDivElement>) {
  return <div className={cn('mb-4', className)} {...props} />;
}

export function FormLabel({ className, ...props }: React.LabelHTMLAttributes<HTMLLabelElement>) {
  return (
    <label
      className={cn('block mb-1.5 text-sm font-medium text-[var(--text-primary)]', className)}
      {...props}
    />
  );
}

export function FormHelper({ className, ...props }: React.HTMLAttributes<HTMLParagraphElement>) {
  return (
    <p className={cn('mt-1.5 text-xs text-[var(--text-tertiary)]', className)} {...props} />
  );
}

export function FormError({ className, ...props }: React.HTMLAttributes<HTMLParagraphElement>) {
  return (
    <p className={cn('mt-1.5 text-xs text-[var(--error-text)]', className)} {...props} />
  );
}
```

### Input

```tsx
// components/ui/Input.tsx
import { forwardRef } from 'react';
import { cn } from '@/lib/cn';

interface InputProps extends React.InputHTMLAttributes<HTMLInputElement> {
  label?: string;
  error?: string;
  helper?: string;
}

export const Input = forwardRef<HTMLInputElement, InputProps>(
  ({ className, label, error, helper, id, ...props }, ref) => {
    const inputId = id || label?.toLowerCase().replace(/\s/g, '-');

    return (
      <div className="space-y-1.5">
        {label && (
          <label
            htmlFor={inputId}
            className="block text-sm font-medium text-[var(--text-primary)]"
          >
            {label}
          </label>
        )}
        <input
          ref={ref}
          id={inputId}
          className={cn(
            'w-full px-3.5 py-2.5 text-sm rounded-lg transition-all duration-150',
            'bg-[var(--bg-sunken)] text-[var(--text-primary)]',
            'border-2 border-[var(--border-strong)]',
            'placeholder:text-[var(--text-muted)]',
            'hover:border-[var(--border-stronger)]',
            'focus:outline-none focus:border-[var(--border-interactive)]',
            'focus:shadow-[0_0_0_3px_var(--primary-muted)]',
            'disabled:opacity-50 disabled:cursor-not-allowed disabled:bg-[var(--bg-muted)]',
            'font-[inherit]',
            error && 'border-[var(--error-border)] focus:shadow-[0_0_0_3px_var(--error-subtle)]',
            className
          )}
          {...props}
        />
        {helper && !error && (
          <p className="text-xs text-[var(--text-tertiary)]">{helper}</p>
        )}
        {error && (
          <p className="text-xs text-[var(--error-text)]">{error}</p>
        )}
      </div>
    );
  }
);
```

### Select

```tsx
// components/ui/Select.tsx
import { forwardRef } from 'react';
import { cn } from '@/lib/cn';

interface SelectProps extends React.SelectHTMLAttributes<HTMLSelectElement> {
  label?: string;
  error?: string;
  helper?: string;
  options: { value: string; label: string }[];
}

export const Select = forwardRef<HTMLSelectElement, SelectProps>(
  ({ className, label, error, helper, options, id, ...props }, ref) => {
    const selectId = id || label?.toLowerCase().replace(/\s/g, '-');

    return (
      <div className="space-y-1.5">
        {label && (
          <label
            htmlFor={selectId}
            className="block text-sm font-medium text-[var(--text-primary)]"
          >
            {label}
          </label>
        )}
        <select
          ref={ref}
          id={selectId}
          className={cn(
            'w-full px-3.5 py-2.5 text-sm rounded-lg transition-all duration-150',
            'bg-[var(--bg-sunken)] text-[var(--text-primary)]',
            'border-2 border-[var(--border-strong)]',
            'hover:border-[var(--border-stronger)]',
            'focus:outline-none focus:border-[var(--border-interactive)]',
            'focus:shadow-[0_0_0_3px_var(--primary-muted)]',
            'disabled:opacity-50 disabled:cursor-not-allowed disabled:bg-[var(--bg-muted)]',
            'font-[inherit] appearance-none cursor-pointer',
            'bg-[length:16px] bg-no-repeat bg-[right_0.75rem_center]',
            "bg-[url(\"data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='16' height='16' viewBox='0 0 24 24' fill='none' stroke='%23756A8A' stroke-width='2'%3E%3Cpolyline points='6 9 12 15 18 9'/%3E%3C/svg%3E\")]",
            'pr-10',
            error && 'border-[var(--error-border)]',
            className
          )}
          {...props}
        >
          {options.map((option) => (
            <option key={option.value} value={option.value}>
              {option.label}
            </option>
          ))}
        </select>
        {helper && !error && (
          <p className="text-xs text-[var(--text-tertiary)]">{helper}</p>
        )}
        {error && (
          <p className="text-xs text-[var(--error-text)]">{error}</p>
        )}
      </div>
    );
  }
);
```

### Toggle

Toggle switch component.

```tsx
// components/ui/Toggle.tsx
import { cn } from '@/lib/cn';

interface ToggleProps {
  checked?: boolean;
  onChange?: (checked: boolean) => void;
  disabled?: boolean;
  label?: string;
  className?: string;
}

export function Toggle({ checked, onChange, disabled, label, className }: ToggleProps) {
  return (
    <label className={cn('inline-flex items-center gap-3 cursor-pointer', disabled && 'opacity-50 cursor-not-allowed', className)}>
      <span className="relative w-11 h-6">
        <input
          type="checkbox"
          checked={checked}
          onChange={(e) => onChange?.(e.target.checked)}
          disabled={disabled}
          className="sr-only peer"
        />
        <span
          className={cn(
            'absolute inset-0 rounded-full transition-colors duration-200',
            'bg-[var(--border-strong)]',
            'peer-checked:bg-[var(--primary)]'
          )}
        />
        <span
          className={cn(
            'absolute w-[18px] h-[18px] left-[3px] bottom-[3px]',
            'bg-white rounded-full shadow-[var(--shadow-sm)]',
            'transition-transform duration-200',
            'peer-checked:translate-x-5'
          )}
        />
      </span>
      {label && <span className="text-sm text-[var(--text-primary)]">{label}</span>}
    </label>
  );
}
```

---

## Feedback Components

### Alert

Alert component with info, success, warning, and error variants.

```tsx
// components/ui/Alert.tsx
import { Info, CheckCircle, AlertTriangle, XCircle } from 'lucide-react';
import { cn } from '@/lib/cn';

type AlertVariant = 'info' | 'success' | 'warning' | 'error';

interface AlertProps {
  variant: AlertVariant;
  title?: string;
  children: React.ReactNode;
  className?: string;
}

export function Alert({ variant, title, children, className }: AlertProps) {
  const variants = {
    info: {
      wrapper: 'bg-[var(--primary-muted)] border-[var(--primary)] text-[var(--primary)]',
      icon: Info,
    },
    success: {
      wrapper: 'bg-[var(--success-subtle)] border-[var(--success-border)] text-[var(--success-text)]',
      icon: CheckCircle,
    },
    warning: {
      wrapper: 'bg-[var(--warning-subtle)] border-[var(--warning-border)] text-[var(--warning-text)]',
      icon: AlertTriangle,
    },
    error: {
      wrapper: 'bg-[var(--error-subtle)] border-[var(--error-border)] text-[var(--error-text)]',
      icon: XCircle,
    },
  };

  const { wrapper, icon: Icon } = variants[variant];

  return (
    <div
      className={cn(
        'p-4 rounded-lg border flex gap-3 mb-4',
        wrapper,
        className
      )}
    >
      <Icon className="w-5 h-5 flex-shrink-0" />
      <div className="flex-1">
        {title && <div className="font-semibold mb-1">{title}</div>}
        <div className="text-sm">{children}</div>
      </div>
    </div>
  );
}
```

### LogConsole

Log console display with log entries.

```tsx
// components/feedback/LogConsole.tsx
import { cn } from '@/lib/cn';

interface LogConsoleProps {
  children: React.ReactNode;
  maxHeight?: string;
  className?: string;
}

export function LogConsole({ children, maxHeight = '300px', className }: LogConsoleProps) {
  return (
    <div
      className={cn(
        'bg-[var(--bg-sunken)] border border-[var(--border-default)] rounded-lg',
        "font-['JetBrains_Mono',monospace] text-xs overflow-y-auto",
        className
      )}
      style={{ maxHeight }}
    >
      {children}
    </div>
  );
}
```

### LogEntry

Individual log entry with timestamp, level, and message.

```tsx
// components/feedback/LogEntry.tsx
import { cn } from '@/lib/cn';

type LogLevel = 'info' | 'warn' | 'error' | 'debug';

interface LogEntryProps {
  time: string;
  level: LogLevel;
  message: string;
}

export function LogEntry({ time, level, message }: LogEntryProps) {
  const levelStyles = {
    info: 'text-[var(--primary)]',
    warn: 'text-[var(--warning-text)]',
    error: 'text-[var(--error-text)]',
    debug: 'text-[var(--text-tertiary)]',
  };

  const levelLabels = {
    info: 'INFO',
    warn: 'WARN',
    error: 'ERROR',
    debug: 'DEBUG',
  };

  return (
    <div className="px-3 py-1.5 flex gap-3 border-b border-[var(--border-muted)] last:border-b-0">
      <span className="text-[var(--text-muted)] whitespace-nowrap">{time}</span>
      <span className={cn('font-semibold w-12', levelStyles[level])}>{levelLabels[level]}</span>
      <span className="text-[var(--text-primary)] break-words">{message}</span>
    </div>
  );
}
```

### Modal

Modal dialog with overlay, header, body, footer, and close button.

```tsx
// components/ui/Modal.tsx
import { X } from 'lucide-react';
import { cn } from '@/lib/cn';

interface ModalProps {
  open: boolean;
  onClose: () => void;
  title: string;
  children: React.ReactNode;
  footer?: React.ReactNode;
  maxWidth?: string;
}

export function Modal({ open, onClose, title, children, footer, maxWidth = '500px' }: ModalProps) {
  if (!open) return null;

  return (
    <div
      className={cn(
        'fixed inset-0 z-[1000] flex items-center justify-center',
        'bg-[var(--bg-overlay)]',
        'animate-in fade-in duration-200'
      )}
      onClick={(e) => e.target === e.currentTarget && onClose()}
    >
      <div
        className={cn(
          'bg-[var(--bg-surface)] rounded-xl shadow-[var(--shadow-xl)]',
          'w-full max-h-[90vh] overflow-hidden',
          'animate-in zoom-in-95 duration-200'
        )}
        style={{ maxWidth }}
      >
        <ModalHeader title={title} onClose={onClose} />
        <ModalBody>{children}</ModalBody>
        {footer && <ModalFooter>{footer}</ModalFooter>}
      </div>
    </div>
  );
}

export function ModalHeader({ title, onClose }: { title: string; onClose: () => void }) {
  return (
    <div className="px-6 py-5 border-b border-[var(--border-muted)] flex items-center justify-between">
      <h3 className="text-lg font-semibold text-[var(--text-primary)]">{title}</h3>
      <button
        onClick={onClose}
        className={cn(
          'w-8 h-8 flex items-center justify-center rounded-md',
          'text-[var(--text-tertiary)] bg-transparent border-none cursor-pointer',
          'hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]',
          'transition-all duration-150'
        )}
      >
        <X className="w-4 h-4" />
      </button>
    </div>
  );
}

export function ModalBody({ children, className }: { children: React.ReactNode; className?: string }) {
  return <div className={cn('p-6 overflow-y-auto', className)}>{children}</div>;
}

export function ModalFooter({ children, className }: { children: React.ReactNode; className?: string }) {
  return (
    <div className={cn('px-6 py-4 border-t border-[var(--border-muted)] flex justify-end gap-3', className)}>
      {children}
    </div>
  );
}
```

---

## Grid System

### Grid

Responsive grid component with column options.

```tsx
// components/ui/Grid.tsx
import { cn } from '@/lib/cn';

interface GridProps extends React.HTMLAttributes<HTMLDivElement> {
  cols?: 2 | 3 | 4;
  gap?: 'sm' | 'md' | 'lg';
}

export function Grid({ cols = 2, gap = 'md', className, ...props }: GridProps) {
  const colStyles = {
    2: 'grid-cols-2 max-lg:grid-cols-1',
    3: 'grid-cols-3 max-xl:grid-cols-2 max-md:grid-cols-1',
    4: 'grid-cols-4 max-xl:grid-cols-2 max-md:grid-cols-1',
  };

  const gapStyles = {
    sm: 'gap-3',
    md: 'gap-4',
    lg: 'gap-6',
  };

  return (
    <div
      className={cn('grid', colStyles[cols], gapStyles[gap], className)}
      {...props}
    />
  );
}
```

---

## Theme Toggle

```tsx
// components/ui/ThemeToggle.tsx
import { useThemeStore } from '@/stores/themeStore';
import { Sun, Moon } from 'lucide-react';
import { cn } from '@/lib/cn';

export function ThemeToggle() {
  const { theme, toggleTheme } = useThemeStore();

  return (
    <button
      onClick={toggleTheme}
      className={cn(
        'p-2 rounded-lg transition-colors',
        'bg-[var(--bg-surface)] border border-[var(--border-default)]',
        'hover:bg-[var(--bg-hover)]',
        'focus-visible:outline-none focus-visible:ring-[3px]',
        'focus-visible:ring-[var(--primary)] focus-visible:ring-offset-2'
      )}
      aria-label={`Switch to ${theme === 'light' ? 'dark' : 'light'} mode`}
    >
      {theme === 'light' ? (
        <Moon className="w-5 h-5 text-[var(--text-secondary)]" />
      ) : (
        <Sun className="w-5 h-5 text-[var(--text-secondary)]" />
      )}
    </button>
  );
}
```

---

## Feature Components

### OutputGroupCard

Complete output group configuration card.

```tsx
// components/stream/OutputGroupCard.tsx
import { Card, CardHeader, CardBody } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { Select } from '@/components/ui/Select';
import { Input } from '@/components/ui/Input';
import { Toggle } from '@/components/ui/Toggle';
import { StreamStatus } from '@/components/ui/StreamStatus';
import { Trash2 } from 'lucide-react';

interface OutputGroupCardProps {
  group: OutputGroup;
  index: number;
  encoders: { video: string[]; audio: string[] };
  status: 'live' | 'connecting' | 'offline' | 'error';
  onUpdate: (updates: Partial<OutputGroup>) => void;
  onRemove: () => void;
}

export function OutputGroupCard({
  group,
  index,
  encoders,
  status,
  onUpdate,
  onRemove,
}: OutputGroupCardProps) {
  return (
    <Card>
      <CardHeader className="flex items-center justify-between">
        <div className="flex items-center gap-3">
          <h3 className="font-semibold text-[var(--text-primary)]">
            Output Group {index + 1}
          </h3>
          <StreamStatus status={status} />
        </div>
        <Button variant="ghost" size="sm" onClick={onRemove}>
          <Trash2 className="w-4 h-4" />
        </Button>
      </CardHeader>
      <CardBody className="space-y-4">
        <div className="grid grid-cols-2 gap-4">
          <Select
            label="Video Encoder"
            value={group.videoEncoder}
            onChange={(e) => onUpdate({ videoEncoder: e.target.value })}
            options={encoders.video.map((e) => ({ value: e, label: e }))}
          />
          <Select
            label="Resolution"
            value={group.resolution}
            onChange={(e) => onUpdate({ resolution: e.target.value })}
            options={[
              { value: '1920x1080', label: '1080p' },
              { value: '1280x720', label: '720p' },
              { value: '854x480', label: '480p' },
            ]}
          />
        </div>
        <div className="grid grid-cols-3 gap-4">
          <Input
            label="Video Bitrate (kbps)"
            type="number"
            value={group.videoBitrate}
            onChange={(e) => onUpdate({ videoBitrate: parseInt(e.target.value) })}
          />
          <Input
            label="FPS"
            type="number"
            value={group.fps}
            onChange={(e) => onUpdate({ fps: parseInt(e.target.value) })}
          />
          <Input
            label="Audio Bitrate (kbps)"
            type="number"
            value={group.audioBitrate}
            onChange={(e) => onUpdate({ audioBitrate: parseInt(e.target.value) })}
          />
        </div>
        <Toggle
          label="Generate PTS timestamps"
          checked={group.generatePts}
          onChange={(checked) => onUpdate({ generatePts: checked })}
        />
      </CardBody>
    </Card>
  );
}
```

---

## Utility: cn()

```typescript
// lib/cn.ts
import { clsx, type ClassValue } from 'clsx';
import { twMerge } from 'tailwind-merge';

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}
```

---

## Dependencies

```json
{
  "dependencies": {
    "react": "^18.2.0",
    "react-dom": "^18.2.0",
    "@tauri-apps/api": "^2.0.0",
    "lucide-react": "^0.263.0",
    "zustand": "^4.4.0",
    "clsx": "^2.0.0",
    "tailwind-merge": "^2.0.0",
    "framer-motion": "^10.16.0"
  },
  "devDependencies": {
    "@types/react": "^18.2.0",
    "@types/react-dom": "^18.2.0",
    "typescript": "^5.0.0",
    "tailwindcss": "^4.0.0",
    "vite": "^5.0.0",
    "@vitejs/plugin-react": "^4.0.0"
  }
}
```

---

## Component Index

| Category | Component | Description |
|----------|-----------|-------------|
| **Layout** | `Sidebar` | Fixed 260px sidebar with header, nav, footer |
| | `SidebarHeader` | Sidebar header section |
| | `SidebarNav` | Scrollable navigation area |
| | `SidebarFooter` | Sidebar footer with actions |
| | `Logo` | Brand logo with gradient |
| | `Header` | Sticky page header with title and actions |
| | `MainContent` | Main content wrapper with sidebar offset |
| | `ContentArea` | Scrollable content area |
| | `AppShell` | Complete app layout shell |
| **Navigation** | `NavSection` | Grouped nav section with title |
| | `NavItem` | Navigation item with icon and badge |
| | `NavBadge` | Count badge for nav items |
| **Dashboard** | `StatBox` | Stats display with icon and change |
| | `StatsRow` | Container for stat boxes |
| | `ProfileCard` | Profile selection card |
| | `StreamCard` | Stream target card with stats |
| **Stream** | `OutputGroup` | Accordion output group |
| | `StreamStatus` | Status badge with pulse |
| | `PlatformIcon` | Platform-specific icon |
| | `OutputGroupCard` | Full output group config |
| **UI** | `Button` | Button with variants and sizes |
| | `Card` | Card container with sections |
| | `Input` | Text input with validation |
| | `Select` | Dropdown select with arrow |
| | `Toggle` | Toggle switch |
| | `ThemeToggle` | Light/dark mode toggle |
| **Form** | `FormGroup` | Form field wrapper |
| | `FormLabel` | Form field label |
| | `FormHelper` | Helper text |
| | `FormError` | Error message |
| **Feedback** | `Alert` | Alert with variants |
| | `LogConsole` | Log display container |
| | `LogEntry` | Individual log entry |
| | `Modal` | Modal dialog |
| **Grid** | `Grid` | Responsive grid with columns |
