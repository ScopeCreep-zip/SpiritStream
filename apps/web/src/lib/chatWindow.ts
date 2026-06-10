import { isTauri } from '@spiritstream/api-client';
import { WebviewWindow } from '@tauri-apps/api/webviewWindow';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { logger } from '@/lib/logger';
import { clientConfig } from '@/lib/constants';
import { useChatStore } from '@/stores/chatStore';

// Tauri API modules are static imports: App.tsx and ChatOverlay.tsx
// already pull them statically, so a `await import()` here would NOT
// help with code-splitting (Vite warns about it) and only adds runtime
// indirection. Static imports are also a no-op in browser mode because
// the modules' bodies just declare classes that throw when called
// outside Tauri — they're cheap until something invokes them.

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
    width: clientConfig.CHAT_POPUP_WIDTH,
    height: clientConfig.CHAT_POPUP_HEIGHT,
    resizable: true,
    decorations: false,
    transparent: true,
    center: true,
    alwaysOnTop,
    url: getOverlayUrl(),
  });

  overlay.once('tauri://created', () => {
    overlay.show().catch((error) => {
      logger.error('Failed to show chat overlay window:', error);
    });
    overlay.setFocus().catch((error) => {
      logger.error('Failed to focus chat overlay window:', error);
    });
  });

  overlay.once('tauri://error', (error) => {
    logger.error('Failed to create chat overlay window:', error);
  });
}

function openBrowserPopup() {
  // Check if popup already exists and is still open
  if (browserPopup && !browserPopup.closed) {
    browserPopup.focus();
    return;
  }

  // Calculate center position
  const width = clientConfig.CHAT_POPUP_WIDTH;
  const height = clientConfig.CHAT_POPUP_HEIGHT;
  const left = window.screenX + (window.outerWidth - width) / 2;
  const top = window.screenY + (window.outerHeight - height) / 2;

  browserPopup = window.open(
    getOverlayUrl(),
    CHAT_OVERLAY_LABEL,
    `width=${width},height=${height},left=${left},top=${top},resizable=yes,scrollbars=no`
  );

  if (!browserPopup) {
    logger.error('Failed to open chat popup - popup may be blocked');
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
    logger.error('Failed to open chat overlay window:', error);
  }
}

/**
 * Close the chat overlay window if it exists.
 */
export async function closeChatOverlay() {
  if (isTauri()) {
    try {
      const overlay = await WebviewWindow.getByLabel(CHAT_OVERLAY_LABEL);
      if (overlay) {
        await overlay.close();
      }
    } catch (error) {
      logger.error('Failed to close chat overlay:', error);
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
    const overlay = await WebviewWindow.getByLabel(CHAT_OVERLAY_LABEL);
    if (overlay) {
      await overlay.setAlwaysOnTop(alwaysOnTop);
    }
  } catch (error) {
    logger.error('Failed to set always on top:', error);
  }
}

/**
 * Set up listener to close chat overlay when main window closes.
 * Call this once from the main app on mount. Returns a cleanup
 * function so the owning effect can unregister on unmount (StrictMode
 * double-mounts used to stack a second permanent listener).
 */
export function setupMainWindowCloseHandler(): () => void {
  // For browser, close popup when main window unloads
  const closePopup = (): void => {
    if (browserPopup && !browserPopup.closed) {
      browserPopup.close();
    }
  };
  window.addEventListener('beforeunload', closePopup);
  return () => window.removeEventListener('beforeunload', closePopup);
}

/**
 * Set up listener on the overlay to close when main window is destroyed.
 * Call this from the ChatOverlay component.
 *
 * Uses polling rather than Tauri events: event-based teardown can
 * keep the main window's Drop chain blocked, preventing a clean
 * close. A parent-child window relationship would be cleaner but
 * Tauri 2 doesn't yet expose that wiring for runtime-created
 * webviews; tracked in `crates/transport-veilid/BLOCKERS.md`-style
 * follow-up.
 */
export async function setupOverlayAutoClose() {
  if (!isTauri()) return;

  try {
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
    }, clientConfig.CHAT_OVERLAY_POLL_MS);
  } catch (error) {
    logger.error('Failed to set up overlay auto-close:', error);
  }
}
