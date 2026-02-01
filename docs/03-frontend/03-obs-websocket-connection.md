# OBS WebSocket Connection in SpiritStream

## Overview

SpiritStream integrates with OBS Studio via the OBS WebSocket protocol to enable real-time control and monitoring of streaming sessions. This connection is essential for features like starting/stopping streams, monitoring stream status, and synchronizing application state with OBS.

---

## How It Works

- The frontend communicates with OBS through a WebSocket connection, typically managed by the backend (Rust/Tauri) or directly in the web app (HTTP mode).
- Connection state is tracked in a global store (`obsStore.ts`), which manages status, error messages, and version info.
- The connection is established using the OBS WebSocket URL and (optionally) a password, as configured in user settings.
- Connection and disconnection events trigger both UI updates and notifications.

---

## Connection Lifecycle

1. **Connect**: On app startup or when the user requests, the app attempts to connect to OBS using the configured host, port, and password.
2. **Status Tracking**: The connection status (`connected`, `disconnected`, `connecting`, etc.) is updated in the store and reflected in the UI.
3. **Event Handling**: The app listens for OBS events (e.g., stream started/stopped, scene changes) and updates state accordingly.
4. **Reconnect Logic**: If the connection drops, the app attempts to reconnect and notifies the user.

---

## Notifications

- When OBS connects or disconnects, notifications are shown to the user (both toast and system notifications, if enabled).
- Example triggers:
  - Successful connection: "OBS Connected"
  - Disconnection: "OBS Disconnected"
- These notifications respect the `showNotifications` user setting.

---

## Code References

- **Store Logic**: `apps/web/src/stores/obsStore.ts`
  - Manages connection state, error messages, and triggers notifications on state changes.
- **System Notifications**: `apps/web/src/lib/notification.ts`
  - Used for native OS notifications in desktop mode.
- **Settings**: `apps/web/src/stores/settingsStore.ts`
  - Stores OBS connection parameters and notification preferences.
- **Connection UI**: Typically reflected in the dashboard or status bar components.

---

## Example: Connection State Change

```typescript
// In obsStore.ts
const showNotifications = useSettingsStore.getState().showNotifications;
if (showNotifications && newConnectionStatus !== prevConnectionStatus) {
  if (newConnectionStatus === 'connected') {
    showSystemNotification('OBS Connected', 'Successfully connected to OBS WebSocket.');
  } else if (newConnectionStatus === 'disconnected') {
    showSystemNotification('OBS Disconnected', 'Disconnected from OBS WebSocket.');
  }
}
```

---

## Error Handling

- Connection errors are captured and shown as toast notifications.
- The app attempts to reconnect automatically if the connection is lost.
- Error messages are user-friendly and localized.

---

## Extending/Customizing

- To add new OBS event handling, update the event listeners in `obsStore.ts`.
- For new notification types, use the `toast` and `showSystemNotification` helpers.
- Ensure new events are reflected in the UI and have appropriate translations.

---

## Summary

The OBS WebSocket connection is a core integration in SpiritStream, enabling real-time control and feedback for streaming workflows. Its state is managed globally, with robust notification and error handling to keep users informed and in control.
