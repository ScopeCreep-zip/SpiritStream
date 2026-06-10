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

    // The api-client dispatches stable codes; this seam owns the wording.
    const describeCode = (code: string): string => {
      switch (code) {
        case 'auth_required':
          return t('connection.authRequired', 'Authentication required — sign in again.');
        case 'max_reconnects':
          return t('connection.maxReconnects', 'Connection lost. Please refresh the page.');
        case 'connection_error':
          return t('connection.error', 'Connection error');
        default:
          return code;
      }
    };

    const handleDisconnected = (event: CustomEvent<{ error?: string }>) => {
      const code = event.detail?.error;
      const error = code ? describeCode(code) : undefined;
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
