import { useCallback, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Copy, LogIn, LogOut } from 'lucide-react';
import { Button } from '@/components/ui/Button';
import { DeviceCodePanel } from '@/components/chat/settings/DeviceCodePanel';
import { api } from '@/lib/client';
import { toast } from '@/hooks/useToast';
import { logger } from '@/lib/logger';

interface PlatformSignInButtonProps {
  provider: 'twitch' | 'youtube' | 'kick' | 'facebook' | 'trovo';
  /** Username from `profile.settings.oauth.{provider}.username` — non-empty
   *  means signed in; rendered as the button label when present. */
  signedInAs: string;
  signInLabel: string;
  /** Backend-reported flag from `GET /oauth/config` — false means this
   *  build/env carries no real client credentials for the provider, so
   *  the button renders an honest "not set up" hint instead of kicking
   *  off a flow that would land on the provider's 400 page. */
  configured: boolean;
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
  configured,
}: PlatformSignInButtonProps) {
  const { t } = useTranslation();
  const [busy, setBusy] = useState(false);
  // Set when the backend could not open the system browser — the auth
  // URL is rendered with a copy affordance instead of a toast pointing
  // at a tab that never opened.
  const [manualUrl, setManualUrl] = useState<string | null>(null);
  // Set when the backend answered with a device-code grant (it chooses
  // the flow) — render the code panel with the values it provided.
  const [devicePanel, setDevicePanel] = useState<{
    userCode: string;
    verificationUri: string;
    expiresIn: number;
  } | null>(null);
  const isSignedIn = signedInAs.trim().length > 0;

  const handleSignIn = useCallback(async () => {
    setBusy(true);
    setManualUrl(null);
    setDevicePanel(null);
    try {
      const started = await api.oauth.startFlow(provider);
      if (started.flow === 'device' && started.userCode && started.verificationUri) {
        setDevicePanel({
          userCode: started.userCode,
          verificationUri: started.verificationUri,
          expiresIn: started.expiresIn ?? 600,
        });
      } else if (started.browserOpened) {
        toast.info(
          t('chat.oauth.browserOpened', {
            defaultValue: 'Check your browser to complete authentication',
          })
        );
      } else if (started.authUrl) {
        setManualUrl(started.authUrl);
      }
    } catch (error) {
      logger.error(`[PlatformSignInButton] ${provider} sign-in failed:`, error);
      toast.error(
        t('chat.oauth.startFailed', {
          defaultValue: 'Failed to start sign-in: {{error}}',
          error: error instanceof Error ? error.message : String(error),
        })
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
        })
      );
    } catch (error) {
      logger.error(`[PlatformSignInButton] ${provider} sign-out failed:`, error);
      toast.error(
        t('chat.oauth.signOutFailed', {
          defaultValue: 'Failed to sign out: {{error}}',
          error: error instanceof Error ? error.message : String(error),
        })
      );
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

  if (!configured) {
    return (
      <p className="mt-3 text-xs text-text-tertiary">
        {t('chat.oauth.notConfigured', {
          defaultValue:
            'Sign-in is not set up in this build — no client credentials for this platform.',
        })}
      </p>
    );
  }

  const handleCopyUrl = async (): Promise<void> => {
    if (!manualUrl) return;
    try {
      await navigator.clipboard.writeText(manualUrl);
      toast.success(t('common.copied'));
    } catch {
      toast.error(t('common.error'));
    }
  };

  return (
    <div className="mt-3">
      <Button
        variant="primary"
        size="sm"
        onClick={handleSignIn}
        disabled={busy || devicePanel !== null}
      >
        <LogIn className="w-3.5 h-3.5" />
        {signInLabel}
      </Button>
      {devicePanel && (
        <DeviceCodePanel
          userCode={devicePanel.userCode}
          verificationUri={devicePanel.verificationUri}
          expiresIn={devicePanel.expiresIn}
          onFinished={() => setDevicePanel(null)}
        />
      )}
      {manualUrl && (
        <div className="mt-2 flex items-center gap-2">
          <span className="text-xs text-text-secondary">
            {t('chat.oauth.openManually', {
              defaultValue: 'Your browser didn’t open — copy the sign-in link:',
            })}
          </span>
          <Button variant="ghost" size="sm" onClick={handleCopyUrl}>
            <Copy className="w-3.5 h-3.5" />
            {t('chat.oauth.copyLink', { defaultValue: 'Copy link' })}
          </Button>
        </div>
      )}
    </div>
  );
}
