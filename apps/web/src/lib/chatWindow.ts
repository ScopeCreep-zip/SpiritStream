import { isTauri } from './backend/env';
import { useChatStore } from '@/stores/chatStore';

const CHAT_OVERLAY_LABEL = 'chat-overlay';
const CHAT_OVERLAY_PATH = '/?overlay=chat';

function getOverlayUrl(): string {
  if (typeof window === 'undefined') return CHAT_OVERLAY_PATH;
  try {
    return new URL(CHAT_OVERLAY_PATH, window.location.origin).toString();
  } catch {
    return CHAT_OVERLAY_PATH;
  }
}

// Track browser popup window reference
let browserPopup: Window | null = null;

async function openTauriOverlay() {
  const { WebviewWindow } = await import('@tauri-apps/api/webviewWindow');

  const existing = await WebviewWindow.getByLabel(CHAT_OVERLAY_LABEL);
  if (existing) {
    await existing.show();
    await existing.setFocus();
    return;
  }

  // Get always on top setting from store
  const alwaysOnTop = useChatStore.getState().overlayAlwaysOnTop;

  const overlay = new WebviewWindow(CHAT_OVERLAY_LABEL, {
    title: 'SpiritStream Chat',
    width: 420,
    height: 720,
    resizable: true,
    decorations: false,
    transparent: true,
    center: true,
    alwaysOnTop,
    url: getOverlayUrl(),
  });

  overlay.once('tauri://created', () => {
    overlay.show().catch((error) => {
      console.error('Failed to show chat overlay window:', error);
    });
    overlay.setFocus().catch((error) => {
      console.error('Failed to focus chat overlay window:', error);
    });
  });

  overlay.once('tauri://error', (error) => {
    console.error('Failed to create chat overlay window:', error);
  });
}

function openBrowserPopup() {
  // Check if popup already exists and is still open
  if (browserPopup && !browserPopup.closed) {
    browserPopup.focus();
    return;
  }

  // Calculate center position
  const width = 420;
  const height = 720;
  const left = window.screenX + (window.outerWidth - width) / 2;
  const top = window.screenY + (window.outerHeight - height) / 2;

  browserPopup = window.open(
    getOverlayUrl(),
    CHAT_OVERLAY_LABEL,
    `width=${width},height=${height},left=${left},top=${top},resizable=yes,scrollbars=no`
  );

  if (!browserPopup) {
    console.error('Failed to open chat popup - popup may be blocked');
  }
}

export async function openChatOverlay() {
  try {
    if (isTauri()) {
      await openTauriOverlay();
    } else {
      openBrowserPopup();
    }
  } catch (error) {
    console.error('Failed to open chat overlay window:', error);
  }
}

/**
 * Close the chat overlay window if it exists.
 */
export async function closeChatOverlay() {
  if (isTauri()) {
    try {
      const { WebviewWindow } = await import('@tauri-apps/api/webviewWindow');
      const overlay = await WebviewWindow.getByLabel(CHAT_OVERLAY_LABEL);
      if (overlay) {
        await overlay.close();
      }
    } catch (error) {
      console.error('Failed to close chat overlay:', error);
    }
  } else if (browserPopup && !browserPopup.closed) {
    browserPopup.close();
    browserPopup = null;
  }
}

/**
 * Update the always-on-top state of the chat overlay window.
 */
export async function setOverlayAlwaysOnTop(alwaysOnTop: boolean) {
  if (!isTauri()) return;

  try {
    const { WebviewWindow } = await import('@tauri-apps/api/webviewWindow');
    const overlay = await WebviewWindow.getByLabel(CHAT_OVERLAY_LABEL);
    if (overlay) {
      await overlay.setAlwaysOnTop(alwaysOnTop);
    }
  } catch (error) {
    console.error('Failed to set always on top:', error);
  }
}

/**
 * Set up listener to close chat overlay when main window closes.
 * Call this once from the main app on mount.
 */
export function setupMainWindowCloseHandler() {
  // For browser, close popup when main window unloads
  window.addEventListener('beforeunload', () => {
    if (browserPopup && !browserPopup.closed) {
      browserPopup.close();
    }
  });
}

/**
 * Set up listener on the overlay to close when main window is destroyed.
 * Call this from the ChatOverlay component.
 *
 * Uses polling to check if main window still exists, since event-based
 * approaches can block the main window from closing.
 *
 * TODO: Find a more elegant solution - possibly handle this in Rust/main.rs
 * by configuring the app to quit when the main window closes, or use
 * parent-child window relationships in Tauri.
 */
export async function setupOverlayAutoClose() {
  if (!isTauri()) return;

  try {
    const { WebviewWindow } = await import('@tauri-apps/api/webviewWindow');
    const { getCurrentWindow } = await import('@tauri-apps/api/window');

    const overlayWindow = getCurrentWindow();

    // Poll to check if main window still exists
    const checkInterval = setInterval(async () => {
      try {
        const mainWindow = await WebviewWindow.getByLabel('main');
        if (!mainWindow) {
          clearInterval(checkInterval);
          overlayWindow.close().catch(() => {});
        }
      } catch {
        // If we can't check, main window is probably gone
        clearInterval(checkInterval);
        overlayWindow.close().catch(() => {});
      }
    }, 500);
  } catch (error) {
    console.error('Failed to set up overlay auto-close:', error);
  }
}
