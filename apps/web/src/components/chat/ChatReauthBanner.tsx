import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { Settings } from 'lucide-react';
import { Button } from '@/components/ui/Button';
import { useProfileStore } from '@/stores/profileStore';
import { useChatStore } from '@/stores/chatStore';
import { createDefaultChatSettings } from '@/lib/profile-helpers';
import type { ChatPlatformStatus } from '@spiritstream/types';

interface ChatReauthBannerProps {
  statuses: ChatPlatformStatus[];
  /** Opens the Integrations → Chat panel where OAuth setup + sign-in live. */
  onOpenIntegrations?: () => void;
}

/** OAuth-capable chat platforms whose read-only fallback is recoverable by
 *  re-authenticating. TikTok (read-only by design) is excluded. */
const OAUTH_PLATFORMS = ['twitch', 'youtube', 'kick', 'trovo', 'facebook'] as const;
type OAuthPlatform = (typeof OAUTH_PLATFORMS)[number];

/**
 * Inline "sign in to send" prompt shown directly above the composer when a
 * chat platform is connected but cannot send — the Twitch anonymous
 * read-only fallback (token failed connect-time validation) or a token that
 * expired mid-session (`twitchReauthNeeded`).
 *
 * Routes the user to the Integrations chat panel (via `onOpenIntegrations`)
 * rather than inlining OAuth here: that panel owns the COMPLETE setup — the
 * developer-portal link, paste-ready console fields, the credentials form
 * for a not-yet-configured provider, AND the sign-in flow. Stuffing that
 * into the chat column lost information and cramped the flow. Backend owns
 * the `canSend` verdict; this only surfaces it and points at the fix.
 */
export function ChatReauthBanner({
  statuses,
  onOpenIntegrations,
}: ChatReauthBannerProps): React.ReactElement | null {
  const { t } = useTranslation();
  const currentProfile = useProfileStore((state) => state.current);
  const chatSettings = useMemo(
    () => currentProfile?.settings?.chat ?? createDefaultChatSettings(),
    [currentProfile]
  );
  const twitchReauthNeeded = useChatStore((state) => state.twitchReauthNeeded);

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

  if (needsReauth.length === 0 || !onOpenIntegrations) return null;

  return (
    <div className="mt-3 shrink-0 flex items-center gap-2 rounded-lg border border-border-strong bg-bg-elevated p-2">
      <span className="flex-1 text-xs text-text-secondary">
        {t('chat.reauth.prompt', {
          defaultValue: 'Sign in to {{platform}} again to send messages.',
          platform: needsReauth.map((p) => t(`chat.platforms.${p}`, p)).join(', '),
        })}
      </span>
      <Button variant="primary" size="sm" onClick={onOpenIntegrations}>
        <Settings className="w-3.5 h-3.5" />
        {t('chat.reauth.openSettings', { defaultValue: 'Open settings' })}
      </Button>
    </div>
  );
}
