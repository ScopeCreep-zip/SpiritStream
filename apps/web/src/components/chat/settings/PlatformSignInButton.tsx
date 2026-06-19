import { useCallback, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { AlertTriangle, Copy, LogIn, LogOut } from 'lucide-react';
import { Button } from '@/components/ui/Button';
import { DeviceCodePanel } from '@/components/chat/settings/DeviceCodePanel';
import { ProviderCredentialsForm } from '@/components/chat/settings/ProviderCredentialsForm';
import type { OAuthProviderSummary } from '@spiritstream/api-client';
import type { ChatPlatformStatus } from '@spiritstream/types';
import { api } from '@/lib/client';
import { toast } from '@/hooks/useToast';
import { logger } from '@/lib/logger';
import { useProfileStore } from '@/stores/profileStore';
import { createDefaultOAuthAccount } from '@/lib/profile-helpers';

interface PlatformSignInButtonProps {
  provider: 'twitch' | 'youtube' | 'kick' | 'facebook' | 'trovo';
  /** Username from `profile.settings.oauth.{provider}.username` — non-empty
   *  means an account is stored. */
  signedInAs: string;
  signInLabel: string;
  /** Backend setup state from `GET /oauth/config`. `null` while loading.
   *  Unconfigured providers render the in-app credentials form instead
   *  of kicking off a flow that would land on the provider's 400 page. */
  summary: OAuthProviderSummary | null;
  /** Bubbles the post-save summaries up so the panel refreshes every card. */
  onCredentialsSaved: (updated: OAuthProviderSummary[]) => void;
  /** Live connection truth (from `useChatPlatformStatus`). Lets the control
   *  detect that a STORED sign-in is actually broken — connected but unable
   *  to send (the Twitch anonymous read-only fallback after a dead token) —
   *  and make "Sign back in" the primary action instead of "Sign out". */
  connectionStatus?: ChatPlatformStatus['status'];
  canSend?: boolean;
  /** Optional preflight used by parent panels to flush unsaved channel/id
   *  edits before the OAuth flow steals focus or reloads the profile. */
  beforeSignIn?: () => Promise<void> | void;
}

type AccountState = 'needsSetup' | 'signedOut' | 'needsReauth' | 'signedIn';

/** The single source of truth for what the control renders. */
function deriveAccountState(
  notConfigured: boolean,
  hasAccount: boolean,
  broken: boolean
): AccountState {
  // No app credentials → no OAuth flow can start, so the ONLY action is the
  // in-app credentials setup — even for a signed-in account whose app
  // credentials are missing (they must be re-entered before re-auth). This
  // is what stops "Sign back in" from dead-ending on
  // `oauth_provider_not_configured`.
  if (notConfigured) return 'needsSetup';
  if (hasAccount) return broken ? 'needsReauth' : 'signedIn';
  return 'signedOut';
}

/**
 * State-driven OAuth account control for a chat platform. The rendered
 * primary action follows the REAL account state, not a flag:
 *
 * - `needsSetup`   — no app credentials → in-app credentials form.
 * - `signedOut`    — configured, no account → "Sign in" + guided flow.
 * - `needsReauth`  — account stored but the connection can't send (token
 *                    expired / anonymous read-only): "Sign back in" is the
 *                    PRIMARY action with the device/browser guidance; "Sign
 *                    out" is demoted to a secondary affordance.
 * - `signedIn`     — healthy → "Signed in as X" + "Sign out".
 *
 * On success the backend persists the token, reconnects the chat with it,
 * and emits `oauth_complete` (handled by `useOAuthCompletion`); the status
 * poll then flips the card back to `signedIn`.
 */
export function PlatformSignInButton({
  provider,
  signedInAs,
  signInLabel,
  summary,
  onCredentialsSaved,
  connectionStatus,
  canSend,
  beforeSignIn,
}: PlatformSignInButtonProps) {
  const { t } = useTranslation();
  const [busy, setBusy] = useState(false);
  // Set when the backend could not open the system browser — the auth URL is
  // rendered with a copy affordance instead of a toast pointing at a tab that
  // never opened.
  const [manualUrl, setManualUrl] = useState<string | null>(null);
  // Set when the backend answered with a device-code grant (it chooses the
  // flow) — render the code panel with the values it provided.
  const [devicePanel, setDevicePanel] = useState<{
    userCode: string;
    verificationUri: string;
    expiresIn: number;
    browserOpened: boolean;
  } | null>(null);

  // Set when `startFlow` reports the provider has no credentials — a
  // backstop for the race where the summary hadn't loaded (or disagreed)
  // when the user clicked. Flips the control to the setup form.
  const [flowNotConfigured, setFlowNotConfigured] = useState(false);

  const hasAccount = signedInAs.trim().length > 0;
  // A stored account whose live connection is up but can't send is a dead
  // token, not a healthy sign-in. `canSend` is only meaningful while
  // connected, so we only treat connected-but-!canSend as broken.
  const broken = connectionStatus === 'connected' && canSend === false;
  // Trust a loaded summary; fall back to the flow's verdict while it loads.
  const notConfigured = summary != null ? !summary.configured : flowNotConfigured;
  const accountState = deriveAccountState(notConfigured, hasAccount, broken);

  const platformLabel = t(`chat.platforms.${provider}`, provider);

  const clearStoredAccount = useCallback(() => {
    const current = useProfileStore.getState().current;
    if (!current) return;

    const clearedAccount = createDefaultOAuthAccount();
    const nextOauth = { ...current.settings.oauth };
    switch (provider) {
      case 'twitch':
        nextOauth.twitch = clearedAccount;
        break;
      case 'youtube':
        nextOauth.youtube = clearedAccount;
        break;
      case 'kick':
        nextOauth.kick = clearedAccount;
        break;
      case 'facebook':
        nextOauth.facebook = clearedAccount;
        break;
      case 'trovo':
        nextOauth.trovo = clearedAccount;
        break;
    }

    useProfileStore.setState({
      current: {
        ...current,
        settings: {
          ...current.settings,
          oauth: nextOauth,
        },
      },
    });
  }, [provider]);

  const handleSignIn = useCallback(async () => {
    setBusy(true);
    setManualUrl(null);
    setDevicePanel(null);
    try {
      await beforeSignIn?.();
      const started = await api.oauth.startFlow(provider);
      if (started.flow === 'device' && started.userCode && started.verificationUri) {
        setDevicePanel({
          userCode: started.userCode,
          verificationUri: started.verificationUri,
          expiresIn: started.expiresIn ?? 600,
          browserOpened: started.browserOpened ?? false,
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
      const kind =
        (error as { kind?: string })?.kind ?? (error instanceof Error ? error.message : '');
      if (kind.includes('oauth_provider_not_configured')) {
        // Missing app credentials — route to setup, don't dead-end.
        setFlowNotConfigured(true);
        toast.error(
          t('chat.oauth.needsSetupToStart', {
            defaultValue: 'Set up {{platform}} sign-in credentials first.',
            platform: platformLabel,
          })
        );
      } else {
        toast.error(
          t('chat.oauth.startFailed', {
            defaultValue: 'Failed to start sign-in: {{error}}',
            error: error instanceof Error ? error.message : String(error),
          })
        );
      }
    } finally {
      setBusy(false);
    }
  }, [beforeSignIn, provider, platformLabel, t]);

  const handleSignOut = useCallback(async () => {
    setBusy(true);
    try {
      await api.oauth.forget(provider);
      clearStoredAccount();
      toast.success(
        t('chat.oauth.signedOut', {
          defaultValue: 'Signed out of {{platform}}',
          platform: platformLabel,
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
  }, [clearStoredAccount, provider, platformLabel, t]);

  const handleCopyUrl = async (): Promise<void> => {
    if (!manualUrl) return;
    try {
      await navigator.clipboard.writeText(manualUrl);
      toast.success(t('common.copied'));
    } catch {
      toast.error(t('common.error'));
    }
  };

  // Device-code / manual-URL guidance renders the same wherever a flow can
  // be started, so "Sign in" and "Sign back in" both walk the user through.
  const flowPanels = (
    <>
      {devicePanel && (
        <DeviceCodePanel
          userCode={devicePanel.userCode}
          verificationUri={devicePanel.verificationUri}
          expiresIn={devicePanel.expiresIn}
          browserOpened={devicePanel.browserOpened}
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
    </>
  );

  const signOutButton = (
    <Button variant="ghost" size="sm" onClick={handleSignOut} disabled={busy}>
      <LogOut className="w-3.5 h-3.5" />
      {t('chat.signOut', { defaultValue: 'Sign out' })}
    </Button>
  );

  if (accountState === 'needsSetup') {
    return (
      <div className="mt-3">
        <p className="text-xs text-text-tertiary">
          {t('chat.oauth.notConfigured', {
            defaultValue: 'Sign-in needs a one-time setup for this platform.',
          })}
        </p>
        {summary && <ProviderCredentialsForm summary={summary} onSaved={onCredentialsSaved} />}
      </div>
    );
  }

  if (accountState === 'needsReauth') {
    return (
      <div className="mt-3 space-y-2">
        <div className="flex items-start gap-2 rounded p-2 bg-warning-subtle">
          <AlertTriangle className="w-4 h-4 text-warning-text mt-0.5 shrink-0" />
          <p className="text-xs text-warning-text">
            {t('chat.reauth.expired', {
              defaultValue:
                'Your {{platform}} sign-in expired — you can read chat, but you can’t send until you sign back in.',
              platform: platformLabel,
            })}
          </p>
        </div>
        <div className="flex items-center gap-2">
          <Button
            variant="primary"
            size="sm"
            onClick={handleSignIn}
            disabled={busy || devicePanel !== null}
          >
            <LogIn className="w-3.5 h-3.5" />
            {t('chat.reauth.signBackIn', { defaultValue: 'Sign back in to {{platform}}', platform: platformLabel })}
          </Button>
          {signOutButton}
        </div>
        {flowPanels}
      </div>
    );
  }

  if (accountState === 'signedIn') {
    return (
      <div className="mt-3 flex items-center gap-2">
        <span className="text-sm text-text-secondary flex-1">
          {t('chat.oauth.signedInAs', {
            defaultValue: 'Signed in as {{username}}',
            username: signedInAs,
          })}
        </span>
        {signOutButton}
      </div>
    );
  }

  // signedOut
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
      {flowPanels}
      {/* Configured via user-entered credentials: keep the form reachable
          (collapsed) so a typo'd id or rotated secret can be corrected
          without env vars. Saving empty values clears it. */}
      {summary?.overrideClientId && (
        <ProviderCredentialsForm summary={summary} onSaved={onCredentialsSaved} />
      )}
    </div>
  );
}
