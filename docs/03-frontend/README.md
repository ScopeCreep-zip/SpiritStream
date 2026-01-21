# Frontend Documentation (React)

[Documentation](../README.md) > Frontend

---

## Overview

This section documents SpiritStream's React frontend, including component architecture, state management with Zustand, Tauri integration patterns, and theming.

## Documents

| Document | Description | Audience |
|----------|-------------|----------|
| [01. React Architecture](./01-react-architecture.md) | Component hierarchy and patterns | Intermediate+ |
| [02. State Management](./02-state-management.md) | Zustand stores (profile, stream, theme) | Intermediate+ |
| [03. Component Library](./03-component-library.md) | UI components with props and usage | All levels |
| [04. Tauri Integration](./04-tauri-integration.md) | IPC patterns and api wrapper | Advanced |
| [05. Theming and i18n](./05-theming-i18n.md) | Theme system and internationalization | Intermediate+ |

## Key Stores

| Store | Purpose | Key State |
|-------|---------|-----------|
| profileStore | Profile management | profiles, current, loading |
| streamStore | Stream status/stats | isStreaming, activeGroups, stats |
| themeStore | Theme management | currentThemeId, themes |
| languageStore | i18n settings | language |

## Source Structure

```
apps/web/src/
├── App.tsx                 # Root component
├── main.tsx                # Vite entry point
├── components/             # React components
│   ├── ui/                 # Base components
│   ├── layout/             # Layout components
│   ├── modals/             # Modal dialogs
│   └── ...
├── stores/                 # Zustand stores
├── hooks/                  # Custom hooks
├── lib/                    # Utilities
│   ├── tauri.ts            # API wrapper
│   └── ...
├── types/                  # TypeScript types
├── views/                  # Page views
├── locales/                # i18n translations
└── styles/                 # CSS + tokens
```

---

*Section: 03-frontend*
