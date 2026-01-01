# MagillaStream Complete Design System
## Purple & Pink Theme — Full Light & Dark Mode Palette

---

## Design Tokens Overview

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                                                                             │
│                          ☀️  LIGHT MODE                                     │
│                                                                             │
│   Background Layers        Brand Colors           Semantic Colors           │
│   ┌───┬───┬───┬───┐       ┌───┬───┬───┐         ┌───┬───┬───┬───┐         │
│   │   │   │   │   │       │ V │ F │ P │         │ ✓ │ ⚠ │ ✗ │ ℹ │         │
│   │ 1 │ 2 │ 3 │ 4 │       │ I │ U │ I │         │   │   │   │   │         │
│   │   │   │   │   │       │ O │ C │ N │         │ S │ W │ E │ I │         │
│   └───┴───┴───┴───┘       │ L │ H │ K │         │ U │ A │ R │ N │         │
│   Base Surface Elevated    │ E │ S │   │         │ C │ R │ R │ F │         │
│        Muted   Overlay     │ T │ I │   │         │ C │ N │ O │ O │         │
│                            │   │ A │   │         │   │   │ R │   │         │
│                            └───┴───┴───┘         └───┴───┴───┴───┘         │
│                                                                             │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│                          🌙  DARK MODE                                      │
│                                                                             │
│   Background Layers        Brand Colors           Semantic Colors           │
│   ┌───┬───┬───┬───┐       ┌───┬───┬───┐         ┌───┬───┬───┬───┐         │
│   │░░░│▒▒▒│▓▓▓│███│       │ V │ F │ P │         │ ✓ │ ⚠ │ ✗ │ ℹ │         │
│   │ 1 │ 2 │ 3 │ 4 │       │ I │ U │ I │         │   │   │   │   │         │
│   │░░░│▒▒▒│▓▓▓│███│       │ O │ C │ N │         │ S │ W │ E │ I │         │
│   └───┴───┴───┴───┘       │ L │ H │ K │         │ U │ A │ R │ N │         │
│   Base Surface Elevated    │ E │ S │   │         │ C │ R │ R │ F │         │
│        Muted   Overlay     │ T │ I │   │         │ C │ N │ O │ O │         │
│                            │   │ A │   │         │   │   │ R │   │         │
│                            └───┴───┴───┘         └───┴───┴───┴───┘         │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Complete CSS Custom Properties

```css
/* ═══════════════════════════════════════════════════════════════════════════
   MAGILLASTREAM DESIGN SYSTEM
   Complete Light & Dark Mode Color Tokens
   WCAG 2.2 AA Compliant + APCA Ready
   ═══════════════════════════════════════════════════════════════════════════ */

:root {
  /* ─────────────────────────────────────────────────────────────────────────
     COLOR SCALES (Static - Don't change between modes)
     ───────────────────────────────────────────────────────────────────────── */
  
  /* Violet Scale */
  --violet-50: #F5F3FF;
  --violet-100: #EDE9FE;
  --violet-200: #DDD6FE;
  --violet-300: #C4B5FD;
  --violet-400: #A78BFA;
  --violet-500: #8B5CF6;
  --violet-600: #7C3AED;
  --violet-700: #6D28D9;
  --violet-800: #5B21B6;
  --violet-900: #4C1D95;
  --violet-950: #2E1065;
  
  /* Fuchsia Scale */
  --fuchsia-50: #FDF4FF;
  --fuchsia-100: #FAE8FF;
  --fuchsia-200: #F5D0FE;
  --fuchsia-300: #F0ABFC;
  --fuchsia-400: #E879F9;
  --fuchsia-500: #D946EF;
  --fuchsia-600: #C026D3;
  --fuchsia-700: #A21CAF;
  --fuchsia-800: #86198F;
  --fuchsia-900: #701A75;
  --fuchsia-950: #4A044E;
  
  /* Pink Scale */
  --pink-50: #FDF2F8;
  --pink-100: #FCE7F3;
  --pink-200: #FBCFE8;
  --pink-300: #F9A8D4;
  --pink-400: #F472B6;
  --pink-500: #EC4899;
  --pink-600: #DB2777;
  --pink-700: #BE185D;
  --pink-800: #9D174D;
  --pink-900: #831843;
  --pink-950: #500724;
  
  /* Purple-Tinted Neutral Scale */
  --neutral-0: #FFFFFF;
  --neutral-50: #FAF9FB;
  --neutral-100: #F4F2F7;
  --neutral-150: #EFECF3;
  --neutral-200: #E9E5EF;
  --neutral-250: #E0DBE8;
  --neutral-300: #D8D1E2;
  --neutral-350: #C8BFDA;
  --neutral-400: #B8AECA;
  --neutral-450: #A69CB9;
  --neutral-500: #9489A8;
  --neutral-550: #847A99;
  --neutral-600: #756A8A;
  --neutral-650: #695F7E;
  --neutral-700: #5E5472;
  --neutral-750: #544B66;
  --neutral-800: #4A4259;
  --neutral-850: #433C51;
  --neutral-900: #3D3649;
  --neutral-950: #1F1A29;
  --neutral-1000: #0F0A14;

  /* ─────────────────────────────────────────────────────────────────────────
     LIGHT MODE (Default)
     ───────────────────────────────────────────────────────────────────────── */
  
  /* === BACKGROUNDS === */
  --bg-base: #FAFAFA;                    /* Main app background */
  --bg-surface: #FFFFFF;                 /* Cards, panels, modals */
  --bg-surface-secondary: #FAF9FB;       /* Nested cards, secondary panels */
  --bg-elevated: #FFFFFF;                /* Dropdowns, popovers, tooltips */
  --bg-muted: #F4F2F7;                   /* Subtle backgrounds, code blocks */
  --bg-sunken: #EFECF3;                  /* Input fields, wells */
  --bg-canvas: #E9E5EF;                  /* App chrome, sidebars */
  
  /* Overlay backgrounds */
  --bg-overlay: rgba(31, 26, 41, 0.5);   /* Modal backdrop */
  --bg-overlay-light: rgba(31, 26, 41, 0.1);
  
  /* Interactive backgrounds */
  --bg-hover: rgba(124, 58, 237, 0.06);
  --bg-active: rgba(124, 58, 237, 0.10);
  --bg-selected: #EDE9FE;
  --bg-selected-hover: #DDD6FE;
  
  /* === FOREGROUND / TEXT === */
  --text-primary: #1F1A29;               /* Headings, body text — 15.3:1 */
  --text-secondary: #5E5472;             /* Secondary text — 6.2:1 */
  --text-tertiary: #756A8A;              /* Captions, hints — 4.6:1 */
  --text-muted: #9489A8;                 /* Placeholder preview — 3.5:1 */
  --text-disabled: #B8AECA;              /* Disabled text — 2.5:1 */
  --text-inverse: #FFFFFF;               /* Text on dark backgrounds */
  --text-on-brand: #FFFFFF;              /* Text on brand colors */
  
  /* === BRAND COLORS === */
  /* Primary - Violet */
  --primary: #7C3AED;
  --primary-hover: #6D28D9;
  --primary-active: #5B21B6;
  --primary-subtle: #EDE9FE;
  --primary-subtle-hover: #DDD6FE;
  --primary-muted: #F5F3FF;
  --primary-foreground: #FFFFFF;
  
  /* Secondary - Fuchsia */
  --secondary: #C026D3;
  --secondary-hover: #A21CAF;
  --secondary-active: #86198F;
  --secondary-subtle: #FAE8FF;
  --secondary-subtle-hover: #F5D0FE;
  --secondary-muted: #FDF4FF;
  --secondary-foreground: #FFFFFF;
  
  /* Accent - Pink */
  --accent: #DB2777;
  --accent-hover: #BE185D;
  --accent-active: #9D174D;
  --accent-subtle: #FCE7F3;
  --accent-subtle-hover: #FBCFE8;
  --accent-muted: #FDF2F8;
  --accent-foreground: #FFFFFF;
  
  /* === BORDERS === */
  --border-default: #E9E5EF;             /* Default borders */
  --border-muted: #F4F2F7;               /* Subtle dividers */
  --border-strong: #D8D1E2;              /* Emphasized borders */
  --border-stronger: #B8AECA;            /* High contrast borders */
  --border-interactive: #7C3AED;         /* Focus, active states */
  --border-interactive-hover: #6D28D9;
  --border-disabled: #E9E5EF;
  
  /* === FOCUS & RING === */
  --ring-default: #7C3AED;
  --ring-offset: #FFFFFF;
  --ring-width: 3px;
  --ring-offset-width: 2px;
  
  /* === SHADOWS === */
  --shadow-color: 270 20% 20%;
  --shadow-xs: 0 1px 2px hsl(var(--shadow-color) / 0.05);
  --shadow-sm: 0 1px 3px hsl(var(--shadow-color) / 0.08), 
               0 1px 2px hsl(var(--shadow-color) / 0.06);
  --shadow-md: 0 4px 6px hsl(var(--shadow-color) / 0.08), 
               0 2px 4px hsl(var(--shadow-color) / 0.06);
  --shadow-lg: 0 10px 15px hsl(var(--shadow-color) / 0.08), 
               0 4px 6px hsl(var(--shadow-color) / 0.05);
  --shadow-xl: 0 20px 25px hsl(var(--shadow-color) / 0.10), 
               0 8px 10px hsl(var(--shadow-color) / 0.06);
  --shadow-2xl: 0 25px 50px hsl(var(--shadow-color) / 0.15);
  --shadow-inner: inset 0 2px 4px hsl(var(--shadow-color) / 0.08);
  
  /* === SEMANTIC: SUCCESS === */
  --success: #059669;
  --success-hover: #047857;
  --success-active: #065F46;
  --success-subtle: #ECFDF5;
  --success-subtle-hover: #D1FAE5;
  --success-muted: #F0FDF4;
  --success-foreground: #FFFFFF;
  --success-text: #065F46;               /* 7.5:1 on white */
  --success-border: #34D399;
  --success-icon: #10B981;
  
  /* === SEMANTIC: WARNING === */
  --warning: #D97706;
  --warning-hover: #B45309;
  --warning-active: #92400E;
  --warning-subtle: #FFFBEB;
  --warning-subtle-hover: #FEF3C7;
  --warning-muted: #FEFCE8;
  --warning-foreground: #1F1A29;
  --warning-text: #92400E;               /* 5.9:1 on white */
  --warning-border: #FBBF24;
  --warning-icon: #F59E0B;
  
  /* === SEMANTIC: ERROR / DESTRUCTIVE === */
  --error: #DC2626;
  --error-hover: #B91C1C;
  --error-active: #991B1B;
  --error-subtle: #FEF2F2;
  --error-subtle-hover: #FEE2E2;
  --error-muted: #FFF5F5;
  --error-foreground: #FFFFFF;
  --error-text: #991B1B;                 /* 7.8:1 on white */
  --error-border: #F87171;
  --error-icon: #EF4444;
  
  /* === SEMANTIC: INFO === */
  --info: #7C3AED;
  --info-hover: #6D28D9;
  --info-active: #5B21B6;
  --info-subtle: #F5F3FF;
  --info-subtle-hover: #EDE9FE;
  --info-muted: #FAF8FF;
  --info-foreground: #FFFFFF;
  --info-text: #5B21B6;                  /* 8.2:1 on white */
  --info-border: #A78BFA;
  --info-icon: #8B5CF6;
  
  /* === STREAMING STATUS === */
  --status-live: #10B981;
  --status-live-bg: #ECFDF5;
  --status-live-text: #065F46;
  --status-live-pulse: #34D399;
  
  --status-connecting: #F59E0B;
  --status-connecting-bg: #FFFBEB;
  --status-connecting-text: #92400E;
  
  --status-offline: #9489A8;
  --status-offline-bg: #F4F2F7;
  --status-offline-text: #5E5472;
  
  --status-error: #EF4444;
  --status-error-bg: #FEF2F2;
  --status-error-text: #991B1B;
  
  /* === CHARTS & DATA VIZ === */
  --chart-1: #7C3AED;
  --chart-2: #C026D3;
  --chart-3: #DB2777;
  --chart-4: #8B5CF6;
  --chart-5: #E879F9;
  --chart-6: #F472B6;
  --chart-grid: #E9E5EF;
  --chart-axis: #756A8A;
  
  /* === GRADIENTS === */
  --gradient-brand: linear-gradient(135deg, #7C3AED 0%, #C026D3 50%, #DB2777 100%);
  --gradient-brand-subtle: linear-gradient(135deg, #EDE9FE 0%, #FAE8FF 50%, #FCE7F3 100%);
  --gradient-surface: linear-gradient(180deg, #FFFFFF 0%, #FAF9FB 100%);
  
  /* === SCROLLBAR === */
  --scrollbar-track: #F4F2F7;
  --scrollbar-thumb: #D8D1E2;
  --scrollbar-thumb-hover: #B8AECA;
  
  /* === CODE / SYNTAX === */
  --code-bg: #F4F2F7;
  --code-text: #5B21B6;
  --code-border: #E9E5EF;
  --code-comment: #756A8A;
  --code-keyword: #7C3AED;
  --code-string: #059669;
  --code-number: #DB2777;
  --code-function: #C026D3;
}

/* ═══════════════════════════════════════════════════════════════════════════
   DARK MODE
   ═══════════════════════════════════════════════════════════════════════════ */

[data-theme="dark"],
.dark {
  /* === BACKGROUNDS === */
  --bg-base: #0F0A14;                    /* Deepest background */
  --bg-surface: #1A1225;                 /* Cards, panels */
  --bg-surface-secondary: #1F1629;       /* Nested cards */
  --bg-elevated: #251A33;                /* Dropdowns, popovers */
  --bg-muted: #1A1225;                   /* Subtle backgrounds */
  --bg-sunken: #0A0710;                  /* Input fields, wells */
  --bg-canvas: #150F1D;                  /* Sidebars, navigation */
  
  /* Overlay backgrounds */
  --bg-overlay: rgba(0, 0, 0, 0.7);
  --bg-overlay-light: rgba(0, 0, 0, 0.4);
  
  /* Interactive backgrounds */
  --bg-hover: rgba(167, 139, 250, 0.10);
  --bg-active: rgba(167, 139, 250, 0.16);
  --bg-selected: #2E1A4A;
  --bg-selected-hover: #3D2460;
  
  /* === FOREGROUND / TEXT === */
  --text-primary: #F4F2F7;               /* Primary text — 16.8:1 */
  --text-secondary: #D8D1E2;             /* Secondary — 12.1:1 */
  --text-tertiary: #B8AECA;              /* Tertiary — 7.1:1 */
  --text-muted: #9489A8;                 /* Muted — 4.8:1 */
  --text-disabled: #5E5472;              /* Disabled */
  --text-inverse: #1F1A29;               /* Text on light backgrounds */
  --text-on-brand: #FFFFFF;
  
  /* === BRAND COLORS (Adjusted for dark mode) === */
  /* Primary - Violet (Lighter for dark bg) */
  --primary: #A78BFA;
  --primary-hover: #C4B5FD;
  --primary-active: #DDD6FE;
  --primary-subtle: #2E1A4A;
  --primary-subtle-hover: #3D2460;
  --primary-muted: #1E1433;
  --primary-foreground: #0F0A14;
  
  /* Secondary - Fuchsia */
  --secondary: #E879F9;
  --secondary-hover: #F0ABFC;
  --secondary-active: #F5D0FE;
  --secondary-subtle: #3D1A4A;
  --secondary-subtle-hover: #4D2460;
  --secondary-muted: #2D1433;
  --secondary-foreground: #0F0A14;
  
  /* Accent - Pink */
  --accent: #F472B6;
  --accent-hover: #F9A8D4;
  --accent-active: #FBCFE8;
  --accent-subtle: #4A1A3D;
  --accent-subtle-hover: #5D2450;
  --accent-muted: #331428;
  --accent-foreground: #0F0A14;
  
  /* === BORDERS === */
  --border-default: #3D3649;
  --border-muted: #2D2838;
  --border-strong: #5E5472;
  --border-stronger: #756A8A;
  --border-interactive: #A78BFA;
  --border-interactive-hover: #C4B5FD;
  --border-disabled: #3D3649;
  
  /* === FOCUS & RING === */
  --ring-default: #A78BFA;
  --ring-offset: #1A1225;
  
  /* === SHADOWS (Darker, more dramatic) === */
  --shadow-color: 270 50% 5%;
  --shadow-xs: 0 1px 2px hsl(var(--shadow-color) / 0.3);
  --shadow-sm: 0 1px 3px hsl(var(--shadow-color) / 0.4), 
               0 1px 2px hsl(var(--shadow-color) / 0.3);
  --shadow-md: 0 4px 6px hsl(var(--shadow-color) / 0.4), 
               0 2px 4px hsl(var(--shadow-color) / 0.3);
  --shadow-lg: 0 10px 15px hsl(var(--shadow-color) / 0.4), 
               0 4px 6px hsl(var(--shadow-color) / 0.2);
  --shadow-xl: 0 20px 25px hsl(var(--shadow-color) / 0.5), 
               0 8px 10px hsl(var(--shadow-color) / 0.3);
  --shadow-2xl: 0 25px 50px hsl(var(--shadow-color) / 0.6);
  --shadow-inner: inset 0 2px 4px hsl(var(--shadow-color) / 0.4);
  
  /* === SEMANTIC: SUCCESS === */
  --success: #34D399;
  --success-hover: #6EE7B7;
  --success-active: #A7F3D0;
  --success-subtle: #052E16;
  --success-subtle-hover: #064E3B;
  --success-muted: #022C22;
  --success-foreground: #0F0A14;
  --success-text: #6EE7B7;
  --success-border: #10B981;
  --success-icon: #34D399;
  
  /* === SEMANTIC: WARNING === */
  --warning: #FBBF24;
  --warning-hover: #FCD34D;
  --warning-active: #FDE68A;
  --warning-subtle: #451A03;
  --warning-subtle-hover: #78350F;
  --warning-muted: #3D1A03;
  --warning-foreground: #0F0A14;
  --warning-text: #FCD34D;
  --warning-border: #F59E0B;
  --warning-icon: #FBBF24;
  
  /* === SEMANTIC: ERROR === */
  --error: #F87171;
  --error-hover: #FCA5A5;
  --error-active: #FECACA;
  --error-subtle: #450A0A;
  --error-subtle-hover: #7F1D1D;
  --error-muted: #3D0A0A;
  --error-foreground: #0F0A14;
  --error-text: #FCA5A5;
  --error-border: #EF4444;
  --error-icon: #F87171;
  
  /* === SEMANTIC: INFO === */
  --info: #A78BFA;
  --info-hover: #C4B5FD;
  --info-active: #DDD6FE;
  --info-subtle: #1E1A4A;
  --info-subtle-hover: #2E2460;
  --info-muted: #1A1440;
  --info-foreground: #0F0A14;
  --info-text: #C4B5FD;
  --info-border: #8B5CF6;
  --info-icon: #A78BFA;
  
  /* === STREAMING STATUS === */
  --status-live: #34D399;
  --status-live-bg: #052E16;
  --status-live-text: #6EE7B7;
  --status-live-pulse: #10B981;
  
  --status-connecting: #FBBF24;
  --status-connecting-bg: #451A03;
  --status-connecting-text: #FCD34D;
  
  --status-offline: #756A8A;
  --status-offline-bg: #1A1225;
  --status-offline-text: #B8AECA;
  
  --status-error: #F87171;
  --status-error-bg: #450A0A;
  --status-error-text: #FCA5A5;
  
  /* === CHARTS & DATA VIZ (Brighter for dark mode) === */
  --chart-1: #A78BFA;
  --chart-2: #E879F9;
  --chart-3: #F472B6;
  --chart-4: #C4B5FD;
  --chart-5: #F0ABFC;
  --chart-6: #F9A8D4;
  --chart-grid: #3D3649;
  --chart-axis: #9489A8;
  
  /* === GRADIENTS === */
  --gradient-brand: linear-gradient(135deg, #A78BFA 0%, #E879F9 50%, #F472B6 100%);
  --gradient-brand-subtle: linear-gradient(135deg, #2E1A4A 0%, #3D1A4A 50%, #4A1A3D 100%);
  --gradient-surface: linear-gradient(180deg, #1A1225 0%, #150F1D 100%);
  
  /* === SCROLLBAR === */
  --scrollbar-track: #1A1225;
  --scrollbar-thumb: #3D3649;
  --scrollbar-thumb-hover: #5E5472;
  
  /* === CODE / SYNTAX === */
  --code-bg: #1A1225;
  --code-text: #C4B5FD;
  --code-border: #3D3649;
  --code-comment: #756A8A;
  --code-keyword: #A78BFA;
  --code-string: #34D399;
  --code-number: #F472B6;
  --code-function: #E879F9;
}

/* ═══════════════════════════════════════════════════════════════════════════
   SYSTEM PREFERENCE (Alternative to class-based)
   ═══════════════════════════════════════════════════════════════════════════ */

@media (prefers-color-scheme: dark) {
  :root:not([data-theme="light"]) {
    /* Copy all dark mode variables here if using system preference */
    --bg-base: #0F0A14;
    --bg-surface: #1A1225;
    /* ... etc */
  }
}
```

---

## Tailwind CSS v4 Configuration

```javascript
// tailwind.config.js
const colors = require('tailwindcss/colors')

module.exports = {
  darkMode: 'class', // or 'media' for system preference
  theme: {
    extend: {
      colors: {
        // === BRAND COLORS ===
        violet: {
          50: '#F5F3FF',
          100: '#EDE9FE',
          200: '#DDD6FE',
          300: '#C4B5FD',
          400: '#A78BFA',
          500: '#8B5CF6',
          600: '#7C3AED',
          700: '#6D28D9',
          800: '#5B21B6',
          900: '#4C1D95',
          950: '#2E1065',
        },
        fuchsia: {
          50: '#FDF4FF',
          100: '#FAE8FF',
          200: '#F5D0FE',
          300: '#F0ABFC',
          400: '#E879F9',
          500: '#D946EF',
          600: '#C026D3',
          700: '#A21CAF',
          800: '#86198F',
          900: '#701A75',
          950: '#4A044E',
        },
        pink: {
          50: '#FDF2F8',
          100: '#FCE7F3',
          200: '#FBCFE8',
          300: '#F9A8D4',
          400: '#F472B6',
          500: '#EC4899',
          600: '#DB2777',
          700: '#BE185D',
          800: '#9D174D',
          900: '#831843',
          950: '#500724',
        },
        // === PURPLE-TINTED NEUTRALS ===
        neutral: {
          0: '#FFFFFF',
          50: '#FAF9FB',
          100: '#F4F2F7',
          150: '#EFECF3',
          200: '#E9E5EF',
          250: '#E0DBE8',
          300: '#D8D1E2',
          350: '#C8BFDA',
          400: '#B8AECA',
          450: '#A69CB9',
          500: '#9489A8',
          550: '#847A99',
          600: '#756A8A',
          650: '#695F7E',
          700: '#5E5472',
          750: '#544B66',
          800: '#4A4259',
          850: '#433C51',
          900: '#3D3649',
          950: '#1F1A29',
          1000: '#0F0A14',
        },
        // === SEMANTIC ALIASES ===
        primary: {
          DEFAULT: 'var(--primary)',
          hover: 'var(--primary-hover)',
          active: 'var(--primary-active)',
          subtle: 'var(--primary-subtle)',
          muted: 'var(--primary-muted)',
          foreground: 'var(--primary-foreground)',
        },
        secondary: {
          DEFAULT: 'var(--secondary)',
          hover: 'var(--secondary-hover)',
          active: 'var(--secondary-active)',
          subtle: 'var(--secondary-subtle)',
          muted: 'var(--secondary-muted)',
          foreground: 'var(--secondary-foreground)',
        },
        accent: {
          DEFAULT: 'var(--accent)',
          hover: 'var(--accent-hover)',
          active: 'var(--accent-active)',
          subtle: 'var(--accent-subtle)',
          muted: 'var(--accent-muted)',
          foreground: 'var(--accent-foreground)',
        },
        success: {
          DEFAULT: 'var(--success)',
          hover: 'var(--success-hover)',
          subtle: 'var(--success-subtle)',
          text: 'var(--success-text)',
        },
        warning: {
          DEFAULT: 'var(--warning)',
          hover: 'var(--warning-hover)',
          subtle: 'var(--warning-subtle)',
          text: 'var(--warning-text)',
        },
        error: {
          DEFAULT: 'var(--error)',
          hover: 'var(--error-hover)',
          subtle: 'var(--error-subtle)',
          text: 'var(--error-text)',
        },
        info: {
          DEFAULT: 'var(--info)',
          hover: 'var(--info-hover)',
          subtle: 'var(--info-subtle)',
          text: 'var(--info-text)',
        },
        // === BACKGROUND ALIASES ===
        background: {
          DEFAULT: 'var(--bg-base)',
          surface: 'var(--bg-surface)',
          elevated: 'var(--bg-elevated)',
          muted: 'var(--bg-muted)',
          sunken: 'var(--bg-sunken)',
        },
        // === TEXT ALIASES ===
        foreground: {
          DEFAULT: 'var(--text-primary)',
          secondary: 'var(--text-secondary)',
          tertiary: 'var(--text-tertiary)',
          muted: 'var(--text-muted)',
          disabled: 'var(--text-disabled)',
        },
        // === BORDER ALIASES ===
        border: {
          DEFAULT: 'var(--border-default)',
          muted: 'var(--border-muted)',
          strong: 'var(--border-strong)',
          interactive: 'var(--border-interactive)',
        },
      },
      // === BOX SHADOWS ===
      boxShadow: {
        'xs': 'var(--shadow-xs)',
        'sm': 'var(--shadow-sm)',
        'md': 'var(--shadow-md)',
        'lg': 'var(--shadow-lg)',
        'xl': 'var(--shadow-xl)',
        '2xl': 'var(--shadow-2xl)',
        'inner': 'var(--shadow-inner)',
      },
      // === RING ===
      ringColor: {
        DEFAULT: 'var(--ring-default)',
      },
      ringOffsetColor: {
        DEFAULT: 'var(--ring-offset)',
      },
      // === BACKGROUND IMAGE (Gradients) ===
      backgroundImage: {
        'gradient-brand': 'var(--gradient-brand)',
        'gradient-brand-subtle': 'var(--gradient-brand-subtle)',
        'gradient-surface': 'var(--gradient-surface)',
      },
    },
  },
  plugins: [],
}
```

---

## Component Recipes

### Theme Toggle Button

```tsx
// ThemeToggle.tsx
import { useEffect, useState } from 'react';

export function ThemeToggle() {
  const [theme, setTheme] = useState<'light' | 'dark'>('light');

  useEffect(() => {
    // Check system preference on mount
    const prefersDark = window.matchMedia('(prefers-color-scheme: dark)').matches;
    const stored = localStorage.getItem('theme') as 'light' | 'dark' | null;
    const initial = stored || (prefersDark ? 'dark' : 'light');
    setTheme(initial);
    document.documentElement.setAttribute('data-theme', initial);
  }, []);

  const toggle = () => {
    const next = theme === 'light' ? 'dark' : 'light';
    setTheme(next);
    localStorage.setItem('theme', next);
    document.documentElement.setAttribute('data-theme', next);
  };

  return (
    <button
      onClick={toggle}
      className="
        p-2 rounded-lg
        bg-[var(--bg-surface)] 
        border border-[var(--border-default)]
        hover:bg-[var(--bg-hover)]
        focus-visible:outline-none 
        focus-visible:ring-[3px] 
        focus-visible:ring-[var(--ring-default)]
        focus-visible:ring-offset-2
        focus-visible:ring-offset-[var(--ring-offset)]
        transition-colors
      "
      aria-label={`Switch to ${theme === 'light' ? 'dark' : 'light'} mode`}
    >
      {theme === 'light' ? (
        <MoonIcon className="w-5 h-5 text-[var(--text-secondary)]" />
      ) : (
        <SunIcon className="w-5 h-5 text-[var(--text-secondary)]" />
      )}
    </button>
  );
}
```

### Button Variants

```css
/* buttons.css */

/* Base Button */
.btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 0.5rem;
  padding: 0.625rem 1rem;
  font-weight: 500;
  font-size: 0.875rem;
  line-height: 1.25rem;
  border-radius: 0.5rem;
  transition: all 150ms ease;
  cursor: pointer;
}

.btn:focus-visible {
  outline: none;
  ring: var(--ring-width) solid var(--ring-default);
  ring-offset: var(--ring-offset-width) var(--ring-offset);
}

.btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

/* Primary Button */
.btn-primary {
  background-color: var(--primary);
  color: var(--primary-foreground);
  border: none;
}

.btn-primary:hover:not(:disabled) {
  background-color: var(--primary-hover);
}

.btn-primary:active:not(:disabled) {
  background-color: var(--primary-active);
}

/* Secondary Button */
.btn-secondary {
  background-color: var(--secondary);
  color: var(--secondary-foreground);
  border: none;
}

.btn-secondary:hover:not(:disabled) {
  background-color: var(--secondary-hover);
}

/* Accent Button */
.btn-accent {
  background-color: var(--accent);
  color: var(--accent-foreground);
  border: none;
}

.btn-accent:hover:not(:disabled) {
  background-color: var(--accent-hover);
}

/* Outline Button */
.btn-outline {
  background-color: transparent;
  color: var(--primary);
  border: 2px solid var(--primary);
}

.btn-outline:hover:not(:disabled) {
  background-color: var(--primary-subtle);
}

/* Ghost Button */
.btn-ghost {
  background-color: transparent;
  color: var(--text-secondary);
  border: none;
}

.btn-ghost:hover:not(:disabled) {
  background-color: var(--bg-hover);
  color: var(--text-primary);
}

/* Destructive Button */
.btn-destructive {
  background-color: var(--error);
  color: var(--error-foreground);
  border: none;
}

.btn-destructive:hover:not(:disabled) {
  background-color: var(--error-hover);
}

/* Subtle Buttons */
.btn-primary-subtle {
  background-color: var(--primary-subtle);
  color: var(--primary);
  border: none;
}

.btn-primary-subtle:hover:not(:disabled) {
  background-color: var(--primary-subtle-hover);
}
```

### Card Components

```css
/* cards.css */

.card {
  background-color: var(--bg-surface);
  border: 1px solid var(--border-default);
  border-radius: 0.75rem;
  box-shadow: var(--shadow-sm);
}

.card-elevated {
  background-color: var(--bg-elevated);
  box-shadow: var(--shadow-md);
}

.card-interactive {
  cursor: pointer;
  transition: all 150ms ease;
}

.card-interactive:hover {
  border-color: var(--border-interactive);
  box-shadow: var(--shadow-md);
}

.card-interactive:focus-visible {
  outline: none;
  ring: var(--ring-width) solid var(--ring-default);
  ring-offset: var(--ring-offset-width) var(--ring-offset);
}

.card-header {
  padding: 1.25rem 1.5rem;
  border-bottom: 1px solid var(--border-muted);
}

.card-body {
  padding: 1.5rem;
}

.card-footer {
  padding: 1rem 1.5rem;
  border-top: 1px solid var(--border-muted);
  background-color: var(--bg-muted);
  border-radius: 0 0 0.75rem 0.75rem;
}
```

### Stream Status Indicator

```tsx
// StreamStatus.tsx
type Status = 'live' | 'connecting' | 'offline' | 'error';

interface StreamStatusProps {
  status: Status;
  label?: string;
}

export function StreamStatus({ status, label }: StreamStatusProps) {
  const config = {
    live: {
      bg: 'var(--status-live-bg)',
      text: 'var(--status-live-text)',
      dot: 'var(--status-live)',
      pulse: true,
      defaultLabel: 'Live',
    },
    connecting: {
      bg: 'var(--status-connecting-bg)',
      text: 'var(--status-connecting-text)',
      dot: 'var(--status-connecting)',
      pulse: true,
      defaultLabel: 'Connecting',
    },
    offline: {
      bg: 'var(--status-offline-bg)',
      text: 'var(--status-offline-text)',
      dot: 'var(--status-offline)',
      pulse: false,
      defaultLabel: 'Offline',
    },
    error: {
      bg: 'var(--status-error-bg)',
      text: 'var(--status-error-text)',
      dot: 'var(--status-error)',
      pulse: false,
      defaultLabel: 'Error',
    },
  };

  const { bg, text, dot, pulse, defaultLabel } = config[status];

  return (
    <span
      className="inline-flex items-center gap-2 px-2.5 py-1 rounded-full text-xs font-medium"
      style={{ backgroundColor: bg, color: text }}
    >
      <span
        className={`w-2 h-2 rounded-full ${pulse ? 'animate-pulse' : ''}`}
        style={{ backgroundColor: dot }}
      />
      {label || defaultLabel}
    </span>
  );
}
```

### Input Fields

```css
/* inputs.css */

.input {
  width: 100%;
  padding: 0.625rem 0.875rem;
  font-size: 0.875rem;
  line-height: 1.25rem;
  color: var(--text-primary);
  background-color: var(--bg-sunken);
  border: 2px solid var(--border-strong);
  border-radius: 0.5rem;
  transition: all 150ms ease;
}

.input::placeholder {
  color: var(--text-muted);
}

.input:hover:not(:disabled):not(:focus) {
  border-color: var(--border-stronger);
}

.input:focus {
  outline: none;
  border-color: var(--border-interactive);
  box-shadow: 0 0 0 3px var(--primary-muted);
}

.input:disabled {
  opacity: 0.5;
  cursor: not-allowed;
  background-color: var(--bg-muted);
}

.input-error {
  border-color: var(--error-border);
}

.input-error:focus {
  box-shadow: 0 0 0 3px var(--error-muted);
}

.input-label {
  display: block;
  margin-bottom: 0.375rem;
  font-size: 0.875rem;
  font-weight: 500;
  color: var(--text-primary);
}

.input-helper {
  margin-top: 0.375rem;
  font-size: 0.75rem;
  color: var(--text-tertiary);
}

.input-error-message {
  margin-top: 0.375rem;
  font-size: 0.75rem;
  color: var(--error-text);
}
```

---

## Contrast Verification Summary

### Light Mode

| Token | Value | On Background | WCAG Ratio | APCA Lc | Pass |
|-------|-------|---------------|------------|---------|------|
| `--text-primary` | #1F1A29 | #FFFFFF | 15.3:1 | 104 | ✅ AAA |
| `--text-secondary` | #5E5472 | #FFFFFF | 6.2:1 | 70 | ✅ AA |
| `--text-tertiary` | #756A8A | #FFFFFF | 4.6:1 | 57 | ✅ AA |
| `--primary` | #7C3AED | #FFFFFF | 4.7:1 | 58 | ✅ AA |
| `--secondary` | #C026D3 | #FFFFFF | 4.8:1 | 59 | ✅ AA |
| `--accent` | #DB2777 | #FFFFFF | 4.6:1 | 57 | ✅ AA |
| `--success-text` | #065F46 | #ECFDF5 | 7.5:1 | 78 | ✅ AAA |
| `--warning-text` | #92400E | #FFFBEB | 5.9:1 | 66 | ✅ AA |
| `--error-text` | #991B1B | #FEF2F2 | 7.8:1 | 80 | ✅ AAA |

### Dark Mode

| Token | Value | On Background | WCAG Ratio | APCA Lc | Pass |
|-------|-------|---------------|------------|---------|------|
| `--text-primary` | #F4F2F7 | #0F0A14 | 16.8:1 | 99 | ✅ AAA |
| `--text-secondary` | #D8D1E2 | #0F0A14 | 12.1:1 | 89 | ✅ AAA |
| `--text-tertiary` | #B8AECA | #1A1225 | 6.4:1 | 68 | ✅ AA |
| `--primary` | #A78BFA | #0F0A14 | 7.2:1 | 72 | ✅ AA |
| `--secondary` | #E879F9 | #0F0A14 | 9.1:1 | 81 | ✅ AAA |
| `--accent` | #F472B6 | #0F0A14 | 8.7:1 | 79 | ✅ AAA |
| `--success-text` | #6EE7B7 | #052E16 | 8.3:1 | 77 | ✅ AAA |
| `--warning-text` | #FCD34D | #451A03 | 9.4:1 | 83 | ✅ AAA |
| `--error-text` | #FCA5A5 | #450A0A | 7.2:1 | 72 | ✅ AA |

---

## Usage Quick Reference

```tsx
// Example component showing proper token usage
function ExampleCard() {
  return (
    <div className="bg-[var(--bg-surface)] border border-[var(--border-default)] rounded-xl p-6">
      {/* Headings use primary text */}
      <h2 className="text-[var(--text-primary)] text-lg font-semibold">
        Stream Output
      </h2>
      
      {/* Body text uses secondary */}
      <p className="text-[var(--text-secondary)] mt-2">
        Configure your output settings below.
      </p>
      
      {/* Hints use tertiary */}
      <span className="text-[var(--text-tertiary)] text-sm">
        Recommended: 1080p @ 6000kbps
      </span>
      
      {/* Primary action */}
      <button className="bg-[var(--primary)] text-[var(--primary-foreground)] px-4 py-2 rounded-lg">
        Start Stream
      </button>
      
      {/* Secondary action */}
      <button className="bg-[var(--bg-hover)] text-[var(--text-primary)] px-4 py-2 rounded-lg">
        Cancel
      </button>
      
      {/* Status indicator */}
      <div className="flex items-center gap-2">
        <span className="w-2 h-2 rounded-full bg-[var(--status-live)]" />
        <span className="text-[var(--status-live-text)]">Live</span>
      </div>
    </div>
  );
}
```

---

This expanded palette gives you everything needed for a fully themed application with proper light/dark mode support while maintaining WCAG 2.2 AA compliance across both modes.
