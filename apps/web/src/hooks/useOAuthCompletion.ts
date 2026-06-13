import { useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { events } from '@spiritstream/api-client';
import { logger } from '@/lib/logger';
import { toast } from '@/hooks/useToast';
import { useProfileStore } from '@/stores/profileStore';

interface OAuthCompleteEvent {
  provider?: string;
  username?: string;
  displayName?: string;
}

/**
 * Reflect a completed OAuth sign-in in the UI. The backend stores the
 * token on the active profile and emits `oauth_complete`, but nothing
 * reloads the profile — so without this the sign-in button never flips
 * to "Signed in as …" and the user gets no confirmation. Here we reload
 * the active profile (picking up the new `oauth.<provider>.username`)
 * and toast a success. The actual chat connection is handled
 * server-side; the status poll surfaces it.
 */
export function useOAuthCompletion(): void {
  const { t } = useTranslation();

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | null = null;

    // Reload the active profile so `signedInAs` reflects the stored
    // account. Read the current name at fire time — the store is the
    // source of truth for which profile to refresh.
    const reloadActiveProfile = (): void => {
      const { current, loadProfile } = useProfileStore.getState();
      if (current?.name) {
        loadProfile(current.name).catch((error) =>
          logger.error('[useOAuthCompletion] profile reload failed:', error)
        );
      }
    };

    const handleComplete = (payload: OAuthCompleteEvent): void => {
      reloadActiveProfile();
      const who = payload?.displayName || payload?.username;
      toast.success(
        who
          ? t('chat.oauth.signInSuccessNamed', { defaultValue: 'Signed in as {{name}}', name: who })
          : t('chat.oauth.signInSuccess', { defaultValue: 'Signed in' })
      );
    };

    const setup = async (): Promise<void> => {
      const handle = await events.on<OAuthCompleteEvent>('oauth_complete', handleComplete);
      if (cancelled) {
        handle();
        return;
      }
      unlisten = handle;
    };

    setup().catch((error) => logger.error('[useOAuthCompletion] listener setup failed:', error));

    return () => {
      cancelled = true;
      if (unlisten) unlisten();
    };
  }, [t]);
}
