import { useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { events } from '@spiritstream/api-client';
import { logger } from '@/lib/logger';
import { toast } from '@/hooks/useToast';
import { useChatStore } from '@/stores/chatStore';

interface FollowerOnlyUnsupportedEvent {
  platform: string;
  reason: string;
}

interface OAuthCompleteEvent {
  provider: string;
}

/**
 * Surface the backend's follower-only outcome events as user-visible
 * notices. Core decides whether the protection applied
 * (`crates/core/src/services/chat_manager/connection.rs` emits
 * `follower_only_applied` / `follower_only_unsupported`); this hook only
 * renders the verdict — toast per event, plus a sticky store flag that
 * drives the "sign in with Twitch again" hint in the chat settings panel
 * when the failure is recoverable by re-authenticating.
 */
export function useFollowerOnlyNotices(): void {
  const { t } = useTranslation();
  const setReauthNeeded = useChatStore((s) => s.setFollowerOnlyReauthNeeded);
  const setTwitchReauthNeeded = useChatStore((s) => s.setTwitchReauthNeeded);

  useEffect(() => {
    let cancelled = false;
    let unlistenApplied: (() => void) | null = null;
    let unlistenUnsupported: (() => void) | null = null;
    let unlistenOAuthComplete: (() => void) | null = null;

    const setupListeners = async (): Promise<void> => {
      const applied = await events.on('follower_only_applied', () => {
        setReauthNeeded(false);
        toast.success(
          t('chat.followerOnly.applied', {
            defaultValue: 'Follower-only chat is on for Twitch.',
          })
        );
      });
      if (cancelled) {
        applied();
        return;
      }
      unlistenApplied = applied;

      const unsupported = await events.on<FollowerOnlyUnsupportedEvent>(
        'follower_only_unsupported',
        (payload) => {
          logger.warn('[useFollowerOnlyNotices] follower_only_unsupported', payload);
          if (
            payload.reason === 'follower_only_missing_scope' ||
            payload.reason === 'no_oauth_token'
          ) {
            // Recoverable: re-running the Twitch sign-in grants the
            // moderator scope (it's already in the requested scope list).
            setReauthNeeded(true);
            toast.error(
              t('chat.followerOnly.reauth', {
                defaultValue:
                  'Sign in with Twitch again to grant the follower-only permission.',
              })
            );
          } else if (payload.reason === 'platform_not_supported') {
            toast.info(
              t('chat.followerOnly.unsupported', {
                defaultValue:
                  '{{platform}} does not support follower-only chat — that protection is not active there.',
                platform: t(`chat.platforms.${payload.platform}`, payload.platform),
              })
            );
          } else {
            toast.error(
              t('chat.followerOnly.failed', {
                defaultValue: 'Could not turn on follower-only chat: {{reason}}',
                reason: payload.reason,
              })
            );
          }
        }
      );
      if (cancelled) {
        unsupported();
        return;
      }
      unlistenUnsupported = unsupported;

      // A fresh Twitch sign-in carries the scope AND a usable send token,
      // so both sticky re-auth hints are stale the moment it completes.
      const oauthComplete = await events.on<OAuthCompleteEvent>('oauth_complete', (payload) => {
        if (payload.provider === 'twitch') {
          setReauthNeeded(false);
          setTwitchReauthNeeded(false);
        }
      });
      if (cancelled) {
        oauthComplete();
        return;
      }
      unlistenOAuthComplete = oauthComplete;
    };

    setupListeners().catch((error) => {
      logger.error('[useFollowerOnlyNotices] failed to register listeners:', error);
    });

    return () => {
      cancelled = true;
      if (unlistenApplied) unlistenApplied();
      if (unlistenUnsupported) unlistenUnsupported();
      if (unlistenOAuthComplete) unlistenOAuthComplete();
    };
  }, [setReauthNeeded, setTwitchReauthNeeded, t]);
}
