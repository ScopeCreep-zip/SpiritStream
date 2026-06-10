import { useCallback, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { ConfirmDialog } from '@spiritstream/ui';
import { Button } from '@/components/ui/Button';
import { api } from '@/lib/client';
import { disconnectSocket } from '@spiritstream/api-client';
import { toast } from '@/hooks/useToast';
import { logger } from '@/lib/logger';

/**
 * Session controls: log out of THIS device, or revoke every session on
 * every device (confirm-token gated server-side). Backed by the
 * cross-process session store — a revoke here also invalidates CLI
 * sessions and vice versa.
 *
 * After either action succeeds the local socket is torn down and the
 * `backend:auth-required` flow routes the user to the login modal.
 */
export function SessionSection(): React.ReactElement {
  const { t } = useTranslation();
  const [loggingOut, setLoggingOut] = useState(false);
  const [revokeConfirmOpen, setRevokeConfirmOpen] = useState(false);
  const [revoking, setRevoking] = useState(false);

  const returnToLogin = useCallback((): void => {
    disconnectSocket();
    window.dispatchEvent(new CustomEvent('backend:auth-required'));
  }, []);

  const handleLogout = useCallback(async (): Promise<void> => {
    setLoggingOut(true);
    try {
      await api.security.logout();
      toast.success(t('security.loggedOut', { defaultValue: 'Logged out.' }));
      returnToLogin();
    } catch (err) {
      logger.error('[session] logout failed', err);
      toast.error(
        t('security.logoutFailed', {
          defaultValue: 'Logout failed: {{error}}',
          error: err instanceof Error ? err.message : String(err),
        })
      );
    } finally {
      setLoggingOut(false);
    }
  }, [returnToLogin, t]);

  const handleRevokeAll = useCallback(async (): Promise<void> => {
    setRevoking(true);
    try {
      const { revoked } = await api.security.revokeAllSessions();
      toast.success(
        t('security.sessionsRevoked', {
          defaultValue: 'Signed out everywhere — {{count}} sessions revoked.',
          count: revoked,
        })
      );
      setRevokeConfirmOpen(false);
      returnToLogin();
    } catch (err) {
      logger.error('[session] revoke-all failed', err);
      toast.error(
        t('security.revokeAllFailed', {
          defaultValue: 'Failed to revoke sessions: {{error}}',
          error: err instanceof Error ? err.message : String(err),
        })
      );
    } finally {
      setRevoking(false);
    }
  }, [returnToLogin, t]);

  return (
    <div className="flex flex-col gap-3 pt-2 border-t border-border-subtle">
      <div>
        <div className="text-sm font-medium text-text-primary">
          {t('security.sessions', { defaultValue: 'Sessions' })}
        </div>
        <div className="text-xs text-text-tertiary">
          {t('security.sessionsDescription', {
            defaultValue:
              'Log out of this device, or sign out everywhere if you think a session was exposed.',
          })}
        </div>
      </div>
      <div className="flex flex-wrap gap-2">
        <Button variant="outline" onClick={handleLogout} disabled={loggingOut || revoking}>
          {loggingOut
            ? t('common.loading')
            : t('security.logout', { defaultValue: 'Log out' })}
        </Button>
        <Button
          variant="outline"
          onClick={() => setRevokeConfirmOpen(true)}
          disabled={loggingOut || revoking}
        >
          {t('security.revokeAll', { defaultValue: 'Sign out everywhere' })}
        </Button>
      </div>

      <ConfirmDialog
        open={revokeConfirmOpen}
        title={t('security.revokeAll', { defaultValue: 'Sign out everywhere' })}
        confirmLabel={revoking ? t('common.loading') : t('common.confirm')}
        cancelLabel={t('common.cancel')}
        confirmDisabled={revoking}
        confirmVariant="danger"
        onConfirm={handleRevokeAll}
        onCancel={() => setRevokeConfirmOpen(false)}
        message={
          <p>
            {t('security.revokeAllConfirm', {
              defaultValue:
                'Every signed-in device — including this one — will be logged out immediately. Continue?',
            })}
          </p>
        }
      />
    </div>
  );
}
