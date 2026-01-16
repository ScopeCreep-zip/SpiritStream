import { WebviewWindow } from '@tauri-apps/api/webviewWindow';

const CHAT_OVERLAY_LABEL = 'chat-overlay';

export async function openChatOverlay() {
  try {
    const existing = await WebviewWindow.getByLabel(CHAT_OVERLAY_LABEL);
    if (existing) {
      await existing.show();
      await existing.setFocus();
      return;
    }

    const overlay = new WebviewWindow(CHAT_OVERLAY_LABEL, {
      title: 'SpiritStream Chat',
      width: 420,
      height: 720,
      resizable: true,
      decorations: false,
      transparent: true,
      center: true,
      url: '/?overlay=chat',
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
  } catch (error) {
    console.error('Failed to open chat overlay window:', error);
  }
}
