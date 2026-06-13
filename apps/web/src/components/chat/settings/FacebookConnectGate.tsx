import { useCallback, useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { AlertTriangle } from 'lucide-react';
import { Card, CardHeader, CardTitle, CardDescription, CardBody } from '@/components/ui/Card';
import { Input } from '@/components/ui/Input';
import { Button } from '@/components/ui/Button';
import { Toggle } from '@/components/ui/Toggle';
import { PlatformSignInButton } from '@/components/chat/settings/PlatformSignInButton';
import { PlatformConnectionBadge } from '@/components/chat/PlatformStatusDot';
import type { ChatPlatformStatus } from '@spiritstream/types';
import type { OAuthProviderSummary } from '@spiritstream/api-client';
import { useProfileStore } from '@/stores/profileStore';
import { api } from '@/lib/client';
import { toast } from '@/hooks/useToast';
import { logger } from '@/lib/logger';
import {
  createDefaultChatSettings,
  createDefaultOAuthAccount,
  createDefaultOAuthSettings,
} from '@/lib/profile-helpers';

/**
 * Identity-revealing Facebook Live chat connect, gated by a
 * `<details>` warning block + an explicit acknowledgement checkbox.
 * Even after acknowledgement, the actual connect call requires a
 * server-side confirm-token (handled inside `chatApi.connectFacebook`)
 * so the user can't muscle-memory their way through the gate by
 * persisting `facebook_live_video_id` once and forgetting.
 *
 * The token, the access token, and the video id all stay on this
 * component until the user clicks Connect; only then does the
 * combined payload reach the backend.
 */
interface FacebookConnectGateProps {
  /** Backend setup state from `GET /oauth/config` — drives the OAuth
   *  button + in-app credentials form; the manual Page-Token path
   *  below works regardless. */
  oauthSummary: OAuthProviderSummary | null;
  /** Bubbles post-save summaries up so the panel refreshes every card. */
  onCredentialsSaved: (updated: OAuthProviderSummary[]) => void;
  /** Live connection state from the parent's `useChatPlatformStatus`. */
  connectionStatus: ChatPlatformStatus['status'];
}

export function FacebookConnectGate({
  oauthSummary,
  onCredentialsSaved,
  connectionStatus,
}: FacebookConnectGateProps): React.ReactElement {
  const { t } = useTranslation();
  const currentProfile = useProfileStore((state) => state.current);
  const updateProfileSettings = useProfileStore((state) => state.updateProfileSettings);

  const chatSettings = useMemo(
    () => currentProfile?.settings?.chat ?? createDefaultChatSettings(),
    [currentProfile]
  );

  const [videoId, setVideoId] = useState('');
  const [accessToken, setAccessToken] = useState('');
  const [acknowledged, setAcknowledged] = useState(false);
  const [connecting, setConnecting] = useState(false);

  useEffect(() => {
    setVideoId(chatSettings.facebookLiveVideoId);
    // Read Page Access Token from oauth.facebook for editing.
    setAccessToken(currentProfile?.settings?.oauth?.facebook?.accessToken ?? '');
  }, [chatSettings.facebookLiveVideoId, currentProfile]);

  const handleConnect = useCallback(async () => {
    if (!videoId.trim() || !accessToken.trim() || !acknowledged) return;

    setConnecting(true);
    try {
      // Persist video_id + token first so a successful connect ties
      // them to the active profile.
      await updateProfileSettings({
        chat: { ...chatSettings, facebookLiveVideoId: videoId.trim() },
        oauth: {
          // Factory-backed fallback: the old inline literal silently
          // drifted whenever OAuthSettings gained a provider.
          ...(currentProfile?.settings?.oauth ?? createDefaultOAuthSettings()),
          facebook: {
            ...(currentProfile?.settings?.oauth?.facebook ?? createDefaultOAuthAccount()),
            accessToken: accessToken.trim(),
          },
        },
      });

      // Confirm-token + connect. The connectFacebook helper requests an
      // `enable_facebook_chat` token, then POSTs /chat/connections with
      // X-Confirm-Token set — the backend's require_confirm_token gate
      // refuses Facebook connects that arrive without it.
      await api.chat.connectFacebook({
        platform: 'facebook',
        enabled: true,
        credentials: {
          type: 'facebook',
          videoId: videoId.trim(),
          accessToken: accessToken.trim(),
        },
      });

      toast.success(t('chat.facebook.connectSuccess', { defaultValue: 'Facebook chat connected' }));
    } catch (error) {
      logger.error('[FacebookConnectGate] connect failed:', error);
      const message = error instanceof Error ? error.message : String(error);
      toast.error(
        t('chat.facebook.connectFailed', {
          defaultValue: 'Facebook connect failed: {{error}}',
          error: message,
        })
      );
    } finally {
      setConnecting(false);
    }
  }, [acknowledged, accessToken, chatSettings, currentProfile, t, updateProfileSettings, videoId]);

  const handleDisconnect = useCallback(async () => {
    try {
      await api.chat.disconnect('facebook');
      await updateProfileSettings({
        chat: { ...chatSettings, facebookLiveVideoId: '' },
      });
      toast.success(
        t('chat.facebook.disconnectSuccess', { defaultValue: 'Facebook chat disconnected' })
      );
    } catch (error) {
      logger.error('[FacebookConnectGate] disconnect failed:', error);
      toast.error(
        t('chat.facebook.disconnectFailed', {
          defaultValue: 'Facebook disconnect failed: {{error}}',
          error: error instanceof Error ? error.message : String(error),
        })
      );
    }
  }, [chatSettings, t, updateProfileSettings]);

  const canConnect = Boolean(videoId.trim() && accessToken.trim() && acknowledged && !connecting);

  return (
    <Card>
      <CardHeader>
        <div>
          <CardTitle>{t('chat.platforms.facebook')}</CardTitle>
          <CardDescription>
            {t('chat.facebook.description', {
              defaultValue: 'Connect to a Facebook Live video — identity-revealing',
            })}
          </CardDescription>
        </div>
        <PlatformConnectionBadge status={connectionStatus} />
      </CardHeader>
      <CardBody>
        {/* Identity warning. The <details> block defaults closed; the
            user has to physically expand it to read the full warning
            before they can tick the acknowledgement. */}
        <details className="rounded-md border border-error-border bg-error-subtle">
          <summary
            role="alert"
            className="flex items-center gap-2 px-3 py-2 cursor-pointer font-medium text-error-text"
          >
            <AlertTriangle className="w-4 h-4 shrink-0" />
            {t('chat.facebook.warningHeading', {
              defaultValue: 'Connecting Facebook reveals your real-name account',
            })}
          </summary>
          <div className="px-3 pb-3 text-sm text-text-secondary">
            <p>{t('chat.facebook.identityWarning')}</p>
          </div>
        </details>

        <div className="mt-4">
          <Input
            label={t('chat.facebook.videoId', { defaultValue: 'Facebook Live video ID' })}
            value={videoId}
            onChange={(e) => setVideoId(e.target.value)}
            placeholder="123456789012345"
            helper={t('chat.facebook.videoIdHint', {
              defaultValue:
                'From the URL of your Facebook Live broadcast (the numeric id, not the slug).',
            })}
          />
        </div>

        <div className="mt-3">
          <Input
            label={t('chat.facebook.accessToken', { defaultValue: 'Page Access Token' })}
            value={accessToken}
            onChange={(e) => setAccessToken(e.target.value)}
            type="password"
            helper={t('chat.facebook.accessTokenHint', {
              defaultValue:
                'Requires pages_read_engagement + pages_manage_engagement scopes. Long-lived Page tokens never expire.',
            })}
          />
          {/* OAuth alternative to manual token entry. Same auth-server
              flow the Twitch/YouTube/Kick buttons trigger; on
              successful callback the backend persists tokens to
              `oauth.facebook` and this form re-reads the access_token
              via the useEffect below. App-Review-gated for production,
              but works in dev / for maintainer-issued Page tokens. */}
          <PlatformSignInButton
            provider="facebook"
            signedInAs={currentProfile?.settings?.oauth?.facebook?.username ?? ''}
            summary={oauthSummary}
            onCredentialsSaved={onCredentialsSaved}
            signInLabel={t('chat.facebook.loginWithFacebook', {
              defaultValue: 'Login with Facebook',
            })}
          />
        </div>

        <div className="mt-4">
          <Toggle
            checked={acknowledged}
            onChange={setAcknowledged}
            label={t('chat.facebook.confirmEnable')}
          />
        </div>

        <div className="mt-4 flex gap-2">
          <Button variant="primary" onClick={handleConnect} disabled={!canConnect}>
            {connecting
              ? t('chat.facebook.connecting', { defaultValue: 'Connecting…' })
              : t('chat.facebook.connect', { defaultValue: 'Enable Facebook chat' })}
          </Button>
          {chatSettings.facebookLiveVideoId && (
            <Button variant="ghost" onClick={handleDisconnect}>
              {t('chat.facebook.disconnect', { defaultValue: 'Disconnect' })}
            </Button>
          )}
        </div>
      </CardBody>
    </Card>
  );
}
