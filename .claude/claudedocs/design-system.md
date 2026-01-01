# Design System Reference

## Overview

MagillaStream uses a Purple & Pink theme with full light/dark mode support. All colors are WCAG 2.2 AA compliant.

For the complete token definitions, see: `.claude/claudedocs/research/magillastream-complete-design-system.md`

## Brand Colors

### Primary (Violet)

| Token | Light | Dark | Usage |
|-------|-------|------|-------|
| `--primary` | #7C3AED | #A78BFA | Primary buttons, links |
| `--primary-hover` | #6D28D9 | #C4B5FD | Hover states |
| `--primary-active` | #5B21B6 | #DDD6FE | Active/pressed |
| `--primary-subtle` | #EDE9FE | #2E1A4A | Subtle backgrounds |
| `--primary-foreground` | #FFFFFF | #0F0A14 | Text on primary |

### Secondary (Fuchsia)

| Token | Light | Dark | Usage |
|-------|-------|------|-------|
| `--secondary` | #C026D3 | #E879F9 | Secondary actions |
| `--secondary-hover` | #A21CAF | #F0ABFC | Hover states |
| `--secondary-subtle` | #FAE8FF | #3D1A4A | Subtle backgrounds |

### Accent (Pink)

| Token | Light | Dark | Usage |
|-------|-------|------|-------|
| `--accent` | #DB2777 | #F472B6 | Accent elements |
| `--accent-hover` | #BE185D | #F9A8D4 | Hover states |
| `--accent-subtle` | #FCE7F3 | #4A1A3D | Subtle backgrounds |

## Neutral Colors

Purple-tinted gray scale for backgrounds, text, and borders:

| Token | Light | Dark | Usage |
|-------|-------|------|-------|
| `--bg-base` | #FAFAFA | #0F0A14 | App background |
| `--bg-surface` | #FFFFFF | #1A1225 | Cards, panels |
| `--bg-elevated` | #FFFFFF | #251A33 | Dropdowns, popovers |
| `--bg-muted` | #F4F2F7 | #1A1225 | Subtle backgrounds |
| `--bg-sunken` | #EFECF3 | #0A0710 | Input fields |

## Text Colors

| Token | Light | Dark | Contrast | Usage |
|-------|-------|------|----------|-------|
| `--text-primary` | #1F1A29 | #F4F2F7 | 15.3:1 / 16.8:1 | Headings, body |
| `--text-secondary` | #5E5472 | #D8D1E2 | 6.2:1 / 12.1:1 | Secondary text |
| `--text-tertiary` | #756A8A | #B8AECA | 4.6:1 / 6.4:1 | Hints, captions |
| `--text-muted` | #9489A8 | #9489A8 | 3.5:1 / 4.8:1 | Placeholders |
| `--text-disabled` | #B8AECA | #5E5472 | - | Disabled text |

## Semantic Colors

### Success

| Token | Light | Dark |
|-------|-------|------|
| `--success` | #059669 | #34D399 |
| `--success-subtle` | #ECFDF5 | #052E16 |
| `--success-text` | #065F46 | #6EE7B7 |

### Warning

| Token | Light | Dark |
|-------|-------|------|
| `--warning` | #D97706 | #FBBF24 |
| `--warning-subtle` | #FFFBEB | #451A03 |
| `--warning-text` | #92400E | #FCD34D |

### Error

| Token | Light | Dark |
|-------|-------|------|
| `--error` | #DC2626 | #F87171 |
| `--error-subtle` | #FEF2F2 | #450A0A |
| `--error-text` | #991B1B | #FCA5A5 |

### Info

| Token | Light | Dark |
|-------|-------|------|
| `--info` | #7C3AED | #A78BFA |
| `--info-subtle` | #F5F3FF | #1E1A4A |
| `--info-text` | #5B21B6 | #C4B5FD |

## Stream Status Colors

| Status | Color | Background | Text |
|--------|-------|------------|------|
| Live | `--status-live` | `--status-live-bg` | `--status-live-text` |
| Connecting | `--status-connecting` | `--status-connecting-bg` | `--status-connecting-text` |
| Offline | `--status-offline` | `--status-offline-bg` | `--status-offline-text` |
| Error | `--status-error` | `--status-error-bg` | `--status-error-text` |

## Border Colors

| Token | Light | Dark | Usage |
|-------|-------|------|-------|
| `--border-default` | #E9E5EF | #3D3649 | Default borders |
| `--border-muted` | #F4F2F7 | #2D2838 | Subtle dividers |
| `--border-strong` | #D8D1E2 | #5E5472 | Emphasized borders |
| `--border-interactive` | #7C3AED | #A78BFA | Focus, active |

## Shadows

```css
--shadow-xs: 0 1px 2px ...
--shadow-sm: 0 1px 3px ...
--shadow-md: 0 4px 6px ...
--shadow-lg: 0 10px 15px ...
--shadow-xl: 0 20px 25px ...
--shadow-2xl: 0 25px 50px ...
--shadow-inner: inset 0 2px 4px ...
```

## Focus Ring

```css
--ring-default: #7C3AED (light) / #A78BFA (dark)
--ring-offset: #FFFFFF (light) / #1A1225 (dark)
--ring-width: 3px
--ring-offset-width: 2px
```

## Gradients

```css
--gradient-brand: linear-gradient(135deg, violet → fuchsia → pink)
--gradient-brand-subtle: linear-gradient(135deg, violet-subtle → fuchsia-subtle → pink-subtle)
--gradient-surface: linear-gradient(180deg, surface → surface-secondary)
```

## Usage Examples

### Button

```tsx
<button className="
  bg-[var(--primary)]
  text-[var(--primary-foreground)]
  hover:bg-[var(--primary-hover)]
  focus-visible:ring-[3px]
  focus-visible:ring-[var(--ring-default)]
">
  Start Stream
</button>
```

### Card

```tsx
<div className="
  bg-[var(--bg-surface)]
  border border-[var(--border-default)]
  shadow-[var(--shadow-sm)]
  rounded-xl
">
  ...
</div>
```

### Input

```tsx
<input className="
  bg-[var(--bg-sunken)]
  border-2 border-[var(--border-strong)]
  text-[var(--text-primary)]
  placeholder:text-[var(--text-muted)]
  focus:border-[var(--border-interactive)]
" />
```

### Status Badge

```tsx
<span className="
  bg-[var(--status-live-bg)]
  text-[var(--status-live-text)]
  px-2.5 py-1 rounded-full text-xs font-medium
">
  <span className="w-2 h-2 rounded-full bg-[var(--status-live)] animate-pulse" />
  Live
</span>
```

## Theme Switching

```tsx
// Set theme via data attribute
document.documentElement.setAttribute('data-theme', 'dark');

// Or via class
document.documentElement.classList.add('dark');
```

## Tailwind Integration

Design tokens integrate with Tailwind via CSS custom properties:

```javascript
// tailwind.config.js
module.exports = {
  theme: {
    extend: {
      colors: {
        primary: {
          DEFAULT: 'var(--primary)',
          hover: 'var(--primary-hover)',
          // ...
        },
      },
    },
  },
};
```

Then use in templates:

```tsx
<button className="bg-primary hover:bg-primary-hover">
  Click me
</button>
```
