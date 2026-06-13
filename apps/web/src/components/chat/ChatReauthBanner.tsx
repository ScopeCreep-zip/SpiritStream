import { useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { PlatformSignInButton } from '@/components/chat/settings/PlatformSignInButton';
import { useProfileStore } from '@/stores/profileStore';
import { useChatStore } from '@/stores/chatStore';
import { createDefaultChatSettings } from '@/lib/profile-helpers';
import { api } from '@/lib/client';
import { logger } from '@/lib/logger';
import type { OAuthProviderSummary } from '@spiritstream/api-client';
import type { ChatPlatformStatus } from '@spiritstream/types';

interface ChatReauthBannerProps {
  statuses: ChatPlatformStatus[];
}

/** OAuth-capable chat platforms whose read-only fallback is recoverable by
 *  re-authenticating. TikTok (read-only by design) is excluded. */
const OAUTH_PLATFORMS = ['twitch', 'youtube', 'kick', 'trovo', 'facebook'] as const;
type OAuthPlatform = (typeof OAUTH_PLATFORMS)[number];

/**
 * Inline "sign in to send" prompt shown directly above the composer when a
 * chat platform is connected but cannot send — the Twitch anonymous
 * read-only fallback (token failed connect-time validation) or a token that
 * expired mid-session (`twitchReauthNeeded`). Reachable without leaving chat.
 *
 * Delegates the actual sign-in to the canonical `PlatformSignInButton`, which
 * already handles BOTH states: it runs the device/browser flow when the
 * provider has credentials, and renders the in-app credentials form when it
 * does NOT (so a not-configured provider guides the user through setup
 * instead of dead-ending on `oauth_provider_not_configured`). Backend owns
 * the `canSend` verdict; this only renders it and routes recovery.
 */
export function ChatReauthBanner({ statuses }: ChatReauthBannerProps): React.ReactElement | null {
  const { t } = useTranslation();
  const currentProfile = useProfileStore((state) => state.current);
  const chatSettings = useMemo(
    () => currentProfile?.settings?.chat ?? createDefaultChatSettings(),
    [currentProfile]
  );
  const twitchReauthNeeded = useChatStore((state) => state.twitchReauthNeeded);
  const [summaries, setSummaries] = useState<OAuthProviderSummary[] | null>(null);

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

  // Load backend OAuth setup summaries (only when something needs re-auth) so
  // PlatformSignInButton can branch configured vs not-configured. The
  // credentials-form save returns fresh summaries, so a successful in-app
  // setup flips the button to sign-in without a refetch. Mirrors ChatPanel.
  useEffect(() => {
    if (needsReauth.length === 0) return;
    let cancelled = false;
    api.oauth
      .getConfig()
      .then((loaded) => {
        if (!cancelled) setSummaries(loaded);
      })
      .catch((error) => logger.error('[ChatReauthBanner] failed to load oauth config:', error));
    return () => {
      cancelled = true;
    };
  }, [needsReauth.length]);

  if (needsReauth.length === 0) return null;

  const summaryFor = (provider: OAuthPlatform): OAuthProviderSummary | null =>
    summaries?.find((s) => s.provider === provider) ?? null;

  return (
    <div className="mt-3 shrink-0 rounded-lg border border-border-strong bg-bg-elevated p-2">
      {needsReauth.map((platform) => (
        <div key={platform} className="py-0.5">
          <span className="text-xs text-text-secondary">
            {t('chat.reauth.prompt', {
              defaultValue: 'Sign in to {{platform}} again to send messages.',
              platform: t(`chat.platforms.${platform}`, platform),
            })}
          </span>
          {/* signedInAs="" forces the sign-in affordance (never sign-out). */}
          <PlatformSignInButton
            provider={platform}
            signedInAs=""
            signInLabel={t('chat.reauth.signIn', { defaultValue: 'Sign in' })}
            summary={summaryFor(platform)}
            onCredentialsSaved={setSummaries}
          />
        </div>
      ))}
    </div>
  );
}
