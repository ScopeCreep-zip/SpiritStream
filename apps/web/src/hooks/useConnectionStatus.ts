import { useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { toast } from '@/hooks/useToast';
import { showSystemNotification } from '@/lib/notification';
import { useSettingsStore } from '@/stores/settingsStore';
import { useConnectionStore } from '@/stores/connectionStore';

/**
 * Bridges the api-client's `window.dispatchEvent` connection signals
 * into both the `connectionStore` (drives the sidebar badge) and the
 * toast/system-notification surfaces (user-facing).
 *
 * The api-client at `packages/api-client/src/events.ts` is intentionally
 * decoupled from any state library; it just dispatches `backend:*`
 * CustomEvents on the window. This hook is the integration seam that
 * turns those events into store updates and notifications.
 */
export function useConnectionStatus() {
  const { t } = useTranslation();
  const { setConnected, setConnecting, setDisconnected } = useConnectionStore();

  useEffect(() => {
    const handleConnecting = () => {
      setConnecting();
    };

    const handleConnected = () => {
      setConnected();
    };

    const handleReconnected = () => {
      setConnected();
      toast.success(t('connection.reconnected', 'Reconnected to backend'));
      const showNotifications = useSettingsStore.getState().showNotifications;
      if (showNotifications) {
        showSystemNotification(
          t('connection.reconnectedTitle', 'Connection Restored'),
          t('connection.reconnectedBody', 'Reconnected to the backend server.')
        );
      }
    };

    const handleDisconnected = (event: CustomEvent<{ error?: string }>) => {
      const error = event.detail?.error;
      setDisconnected(error);
      toast.error(
        error
          ? t('connection.lostWithError', 'Connection lost: {{error}}', { error })
          : t('connection.lost', 'Connection to backend lost. Attempting to reconnect...')
      );
      const showNotifications = useSettingsStore.getState().showNotifications;
      if (showNotifications) {
        showSystemNotification(
          t('connection.lostTitle', 'Connection Lost'),
          t('connection.lostBody', 'Lost connection to the backend server.')
        );
      }
    };

    window.addEventListener('backend:connecting', handleConnecting);
    window.addEventListener('backend:connected', handleConnected);
    window.addEventListener('backend:reconnected', handleReconnected);
    window.addEventListener('backend:disconnected', handleDisconnected as EventListener);

    return () => {
      window.removeEventListener('backend:connecting', handleConnecting);
      window.removeEventListener('backend:connected', handleConnected);
      window.removeEventListener('backend:reconnected', handleReconnected);
      window.removeEventListener('backend:disconnected', handleDisconnected as EventListener);
    };
  }, [t, setConnected, setConnecting, setDisconnected]);
}
