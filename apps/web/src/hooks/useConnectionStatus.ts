import { useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { toast } from '@/hooks/useToast';
import { showSystemNotification } from '@/lib/notification';
import { backendMode } from '@/lib/backend/env';
import { useSettingsStore } from '@/stores/settingsStore';

/**
 * Hook that listens for backend connection events and displays toast notifications.
 * Only active in HTTP mode (not in Tauri mode where IPC is always available).
 */
export function useConnectionStatus() {
  const { t } = useTranslation();

  useEffect(() => {
    // Skip in Tauri mode - IPC is always available
    if (backendMode === 'tauri') {
      return;
    }

    const handleConnected = () => {
      // Initial connection - no toast needed
    };

    const handleReconnected = () => {
      toast.success(t('connection.reconnected', 'Reconnected to backend'));
      // System notification for reconnection
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
      toast.error(
        error
          ? t('connection.lostWithError', 'Connection lost: {{error}}', { error })
          : t('connection.lost', 'Connection to backend lost. Attempting to reconnect...')
      );
      // System notification for disconnection
      const showNotifications = useSettingsStore.getState().showNotifications;
      if (showNotifications) {
        showSystemNotification(
          t('connection.lostTitle', 'Connection Lost'),
          t('connection.lostBody', 'Lost connection to the backend server.')
        );
      }
    };

    window.addEventListener('backend:connected', handleConnected);
    window.addEventListener('backend:reconnected', handleReconnected);
    window.addEventListener('backend:disconnected', handleDisconnected as EventListener);

    return () => {
      window.removeEventListener('backend:connected', handleConnected);
      window.removeEventListener('backend:reconnected', handleReconnected);
      window.removeEventListener('backend:disconnected', handleDisconnected as EventListener);
    };
  }, [t]);
}
