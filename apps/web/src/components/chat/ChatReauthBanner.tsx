import { useCallback, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Copy, LogIn } from 'lucide-react';
import { Button } from '@/components/ui/Button';
import { DeviceCodePanel } from '@/components/chat/settings/DeviceCodePanel';
import { useProfileStore } from '@/stores/profileStore';
import { useChatStore } from '@/stores/chatStore';
import { createDefaultChatSettings } from '@/lib/profile-helpers';
import { api } from '@/lib/client';
import { toast } from '@/hooks/useToast';
import { logger } from '@/lib/logger';
import type { ChatPlatformStatus } from '@spiritstream/types';

interface ChatReauthBannerProps {
  statuses: ChatPlatformStatus[];
}

/** OAuth-capable chat platforms whose read-only fallback is recoverable by
 *  re-authenticating. TikTok (read-only by design) is excluded. */
const OAUTH_PLATFORMS = ['twitch', 'youtube', 'kick', 'trovo', 'facebook'] as const;
type OAuthPlatform = (typeof OAUTH_PLATFORMS)[number];

interface ActiveFlow {
  provider: OAuthPlatform;
  manualUrl: string | null;
  device: {
    userCode: string;
    verificationUri: string;
    expiresIn: number;
    browserOpened: boolean;
  } | null;
}

/**
 * Inline "sign in to send" prompt shown directly above the composer when a
 * chat platform is connected but cannot send — the Twitch anonymous
 * read-only fallback (token failed connect-time validation) or a token that
 * expired mid-session (`twitchReauthNeeded`). Reachable without leaving the
 * chat surface: clicking kicks off the SAME `api.oauth.startFlow` the
 * Integrations sign-in button uses. Self-hides when nothing needs re-auth.
 *
 * Backend owns the decision (`canSend` on the status, emitted by core); this
 * only renders the verdict and starts the recovery flow.
 */
export function ChatReauthBanner({ statuses }: ChatReauthBannerProps): React.ReactElement | null {
  const { t } = useTranslation();
  const currentProfile = useProfileStore((state) => state.current);
  const chatSettings = useMemo(
    () => currentProfile?.settings?.chat ?? createDefaultChatSettings(),
    [currentProfile]
  );
  const twitchReauthNeeded = useChatStore((state) => state.twitchReauthNeeded);
  const [activeFlow, setActiveFlow] = useState<ActiveFlow | null>(null);

  const needsReauth = useMemo<OAuthPlatform[]>(() => {
    return statuses
      .map((status) => status.platform)
      .filter((platform): platform is OAuthPlatform =>
        OAUTH_PLATFORMS.includes(platform as OAuthPlatform)
      )
      .filter((platform) => {
        const status = statuses.find((s) => s.platform === platform);
        if (!status) return false;
        // YouTube API-key mode is read-only on purpose — not a re-auth case.
        if (platform === 'youtube' && chatSettings.youtubeUseApiKey) return false;
        const readOnlyConnected = status.status === 'connected' && !status.canSend;
        const tokenExpired = platform === 'twitch' && twitchReauthNeeded;
        return readOnlyConnected || tokenExpired;
      });
  }, [statuses, chatSettings, twitchReauthNeeded]);

  const handleSignIn = useCallback(
    async (provider: OAuthPlatform): Promise<void> => {
      setActiveFlow(null);
      try {
        const started = await api.oauth.startFlow(provider);
        if (started.flow === 'device' && started.userCode && started.verificationUri) {
          setActiveFlow({
            provider,
            manualUrl: null,
            device: {
              userCode: started.userCode,
              verificationUri: started.verificationUri,
              expiresIn: started.expiresIn ?? 600,
              browserOpened: started.browserOpened ?? false,
            },
          });
        } else if (started.browserOpened) {
          toast.info(
            t('chat.oauth.browserOpened', {
              defaultValue: 'Check your browser to complete authentication',
            })
          );
        } else if (started.authUrl) {
          setActiveFlow({ provider, manualUrl: started.authUrl, device: null });
        }
      } catch (error) {
        logger.error(`[ChatReauthBanner] ${provider} sign-in failed:`, error);
        toast.error(
          t('chat.oauth.startFailed', {
            defaultValue: 'Failed to start sign-in: {{error}}',
            error: error instanceof Error ? error.message : String(error),
          })
        );
      }
    },
    [t]
  );

  const handleCopyUrl = useCallback(async (): Promise<void> => {
    if (!activeFlow?.manualUrl) return;
    try {
      await navigator.clipboard.writeText(activeFlow.manualUrl);
      toast.success(t('common.copied'));
    } catch {
      toast.error(t('common.error'));
    }
  }, [activeFlow, t]);

  if (needsReauth.length === 0) return null;

  return (
    <div className="mt-3 shrink-0 rounded-lg border border-border-strong bg-bg-elevated p-2">
      {needsReauth.map((platform) => (
        <div key={platform} className="flex items-center gap-2 py-0.5">
          <span className="flex-1 text-xs text-text-secondary">
            {t('chat.reauth.prompt', {
              defaultValue: 'Sign in to {{platform}} again to send messages.',
              platform: t(`chat.platforms.${platform}`, platform),
            })}
          </span>
          <Button
            variant="primary"
            size="sm"
            onClick={() => handleSignIn(platform)}
            disabled={activeFlow?.device !== null && activeFlow?.provider === platform}
          >
            <LogIn className="w-3.5 h-3.5" />
            {t('chat.reauth.signIn', { defaultValue: 'Sign in' })}
          </Button>
        </div>
      ))}
      {activeFlow?.device && (
        <DeviceCodePanel
          userCode={activeFlow.device.userCode}
          verificationUri={activeFlow.device.verificationUri}
          expiresIn={activeFlow.device.expiresIn}
          browserOpened={activeFlow.device.browserOpened}
          onFinished={() => setActiveFlow(null)}
        />
      )}
      {activeFlow?.manualUrl && (
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
