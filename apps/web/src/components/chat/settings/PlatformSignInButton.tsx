import { useCallback, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { LogIn, LogOut } from 'lucide-react';
import { Button } from '@/components/ui/Button';
import { api } from '@/lib/client';
import { toast } from '@/hooks/useToast';
import { logger } from '@/lib/logger';

interface PlatformSignInButtonProps {
  provider: 'twitch' | 'youtube' | 'kick' | 'facebook';
  /** Username from `profile.settings.oauth.{provider}.username` — non-empty
   *  means signed in; rendered as the button label when present. */
  signedInAs: string;
  signInLabel: string;
}

/**
 * Generic OAuth sign-in / sign-out toggle for chat platforms.
 *
 * Click → `api.oauth.startFlow(provider)` which the backend handles by
 * spinning up a localhost callback server, opening the platform's
 * authorize URL in the user's browser, and persisting the resulting
 * tokens to the active profile's `oauth.{provider}` slot once the
 * callback fires. The frontend listens for `oauth_complete` events
 * elsewhere; this button only kicks off the flow + surfaces a toast.
 *
 * When already signed in (parent passes `signedInAs`), the button
 * flips to a "Sign out" affordance that calls `api.oauth.forget` to
 * revoke + clear the stored token.
 */
export function PlatformSignInButton({
  provider,
  signedInAs,
  signInLabel,
}: PlatformSignInButtonProps) {
  const { t } = useTranslation();
  const [busy, setBusy] = useState(false);
  const isSignedIn = signedInAs.trim().length > 0;

  const handleSignIn = useCallback(async () => {
    setBusy(true);
    try {
      await api.oauth.startFlow(provider);
      toast.info(
        t('chat.oauth.browserOpened', {
          defaultValue: 'Check your browser to complete authentication',
        }),
      );
    } catch (error) {
      logger.error(`[PlatformSignInButton] ${provider} sign-in failed:`, error);
      toast.error(
        t('chat.oauth.startFailed', {
          defaultValue: 'Failed to start sign-in: {{error}}',
          error: error instanceof Error ? error.message : String(error),
        }),
      );
    } finally {
      setBusy(false);
    }
  }, [provider, t]);

  const handleSignOut = useCallback(async () => {
    setBusy(true);
    try {
      await api.oauth.forget(provider);
      toast.success(
        t('chat.oauth.signedOut', {
          defaultValue: 'Signed out of {{platform}}',
          platform: provider,
        }),
      );
    } catch (error) {
      logger.error(`[PlatformSignInButton] ${provider} sign-out failed:`, error);
    } finally {
      setBusy(false);
    }
  }, [provider, t]);

  if (isSignedIn) {
    return (
      <div className="flex items-center gap-2 mt-3">
        <span className="text-sm text-text-secondary flex-1">
          {t('chat.oauth.signedInAs', {
            defaultValue: 'Signed in as {{username}}',
            username: signedInAs,
          })}
        </span>
        <Button variant="ghost" size="sm" onClick={handleSignOut} disabled={busy}>
          <LogOut className="w-3.5 h-3.5" />
          {t('chat.signOut', { defaultValue: 'Sign out' })}
        </Button>
      </div>
    );
  }

  return (
    <div className="mt-3">
      <Button variant="primary" size="sm" onClick={handleSignIn} disabled={busy}>
        <LogIn className="w-3.5 h-3.5" />
        {signInLabel}
      </Button>
    </div>
  );
}
