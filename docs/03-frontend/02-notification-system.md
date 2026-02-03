# SpiritStream Notification System

## Overview

SpiritStream provides a robust notification system to keep users informed about important events, errors, and status changes. The system consists of two main types of notifications:

- **Toast Notifications**: In-app, transient messages shown in the UI (bottom corner), used for errors, info, and success messages.
- **System Notifications**: Native OS notifications (via Tauri), used for critical events like stream start/stop, connection loss, or OBS status changes.

Both types respect the user's `showNotifications` setting, except for error toasts, which are always shown.

---

## Toast Notifications

- Implemented in `apps/web/src/hooks/useToast.ts` using Zustand for state management.
- Types: `success`, `error`, `info`.
- Usage: `toast.success(message)`, `toast.error(message)`, `toast.info(message)`.
- Auto-dismiss after 3 seconds.
- Only shown if `showNotifications` is enabled (except errors).
- Used throughout the frontend for API errors, validation issues, and status updates.

**Example:**
```typescript
import { toast } from '@/hooks/useToast';
toast.success('Profile saved!');
toast.error('Failed to load data');
```

---

## System Notifications

- Implemented in `apps/web/src/lib/notification.ts` using the Tauri notification plugin.
- Only available in desktop builds (Tauri mode).
- Permission is requested on first use.
- Used for major events: stream started/stopped, OBS connected/disconnected, backend reconnected/disconnected.
- Triggered in stores like `streamStore.ts`, `obsStore.ts`, and hooks like `useConnectionStatus.ts`.
- Respect the `showNotifications` setting.

**Example:**
```typescript
import { showSystemNotification } from '@/lib/notification';
showSystemNotification('Stream Started', 'Your stream is now live.');
```

---

## User Settings

- The `showNotifications` setting is stored in the global settings (see `settingsStore.ts`).
- Controlled via the Settings UI (`Show Notifications`).
- Synced from backend settings on load.

---

## Notification Triggers

- **Stream Status**: When going live or offline, both toast and system notifications are shown.
- **OBS Connection**: Connect/disconnect events trigger notifications.
- **Backend Connection**: Loss or restoration of backend connection triggers notifications.
- **Errors**: All errors are shown as toast notifications, regardless of settings.

---

## Code References

- Toast logic: `apps/web/src/hooks/useToast.ts`
- System notification logic: `apps/web/src/lib/notification.ts`
- Settings: `apps/web/src/stores/settingsStore.ts`, `apps/web/src/hooks/useSettings.ts`
- Usage examples: `apps/web/src/stores/streamStore.ts`, `apps/web/src/stores/obsStore.ts`, `apps/web/src/hooks/useConnectionStatus.ts`

---

## Extending the System

- To add a new notification, import the `toast` or `showSystemNotification` helpers and call them where needed.
- Always check if the event is user-facing and important before adding a system notification.
- For new event types, ensure translations are added for notification titles/bodies.

---

## Summary

SpiritStream's notification system ensures users are promptly informed of important events, errors, and status changes, with respect for user preferences and platform capabilities.
