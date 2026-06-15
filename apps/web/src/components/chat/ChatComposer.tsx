import { useCallback, useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Send } from 'lucide-react';
import { Button } from '@/components/ui/Button';
import { Input } from '@/components/ui/Input';
import { useProfileStore } from '@/stores/profileStore';
import { api } from '@/lib/client';
import { toast } from '@/hooks/useToast';
import { logger } from '@/lib/logger';
import { createDefaultChatSettings } from '@/lib/profile-helpers';
import { cn } from '@/lib/cn';
import { statusDotClass } from '@/components/chat/PlatformStatusDot';
import { useChatStore } from '@/stores/chatStore';
import type { ChatPlatformStatus } from '@spiritstream/types';

interface ChatComposerProps {
  statuses: ChatPlatformStatus[];
}

/**
 * Send draft + platform pills + the "why can't I send?" hint. Owns its
 * own draftMessage + isSending state; reads statuses from the parent's
 * `useChatPlatformStatus` hook so we don't double-poll. Chat is
 * decoupled from streaming — sending is gated on a connected, send-
 * capable platform, not on an active stream.
 *
 * Backend decides what's valid to send: this component only formats the
 * draft and calls `api.chat.sendMessage`.
 */
export function ChatComposer({ statuses }: ChatComposerProps): React.ReactElement {
  const { t } = useTranslation();
  const currentProfile = useProfileStore((state) => state.current);
  const chatSettings = useMemo(
    () => currentProfile?.settings?.chat ?? createDefaultChatSettings(),
    [currentProfile]
  );
  // Immediate re-auth signal: a Twitch token can expire while the IRC
  // socket stays open (status `canSend` is then stale-true until the next
  // reconnect). When this is set, Twitch is not a valid send target even
  // if `canSend` hasn't flipped yet.
  const twitchReauthNeeded = useChatStore((state) => state.twitchReauthNeeded);
  const [draftMessage, setDraftMessage] = useState('');
  const [isSending, setIsSending] = useState(false);
  // Per-message target when the user has disabled broadcast-to-all on
  // the chat-settings panel. Picks the first connected send-target by
  // default so the dropdown is never empty when shown.
  const [singleTarget, setSingleTarget] = useState<ChatPlatformStatus['platform'] | null>(null);

  const sendTargets = useMemo(() => {
    return statuses.filter((status) => {
      if (status.status !== 'connected') return false;
      // Connected-but-read-only (anonymous Twitch fallback, expired token):
      // the connection receives but the server rejects sends. Not a target.
      if (!status.canSend) return false;
      if (status.platform === 'twitch') {
        if (twitchReauthNeeded) return false;
        return chatSettings.twitchSendEnabled;
      }
      if (status.platform === 'youtube') {
        return chatSettings.youtubeSendEnabled && !chatSettings.youtubeUseApiKey;
      }
      if (status.platform === 'trovo') return chatSettings.trovoSendEnabled;
      if (status.platform === 'kick') return chatSettings.kickSendEnabled;
      // Facebook: send is auth-gated server-side — connector exposes
      // can_send() based on whether a Page Access Token was captured at
      // connect time. There's no per-profile "facebook_send_enabled"
      // toggle because connecting Facebook at all already implies
      // identity exposure; the gate is at the connect step.
      if (status.platform === 'facebook') return true;
      // TikTok intentionally excluded: third-party send is rejected by
      // TikTok; the connector returns PlatformError on send.
      return false;
    });
  }, [chatSettings, statuses, twitchReauthNeeded]);

  // Keep `singleTarget` valid as the connected-set changes (e.g. a
  // platform drops). Picks the first available send-target whenever the
  // current pick disappears.
  useEffect(() => {
    if (chatSettings.sendAllEnabled) return;
    const currentValid =
      singleTarget !== null && sendTargets.some((t) => t.platform === singleTarget);
    if (!currentValid && sendTargets[0]) {
      setSingleTarget(sendTargets[0].platform);
    }
  }, [chatSettings.sendAllEnabled, sendTargets, singleTarget]);

  const canSend = draftMessage.trim().length > 0 && sendTargets.length > 0;

  // Single source of truth for "what state is each chat platform in?" —
  // drives both the bottom badge row and the disabled-send hint below.
  // Mirrors the configured/send-enabled logic used by ChatPanel's auto-
  // visibility list and by `sendTargets` above; tiktok is read-only,
  // facebook send is auth-gated server-side (config = opted-in via the
  // confirm-token gate at connect).
  const allPlatformStates = useMemo(() => {
    const findStatus = (platform: ChatPlatformStatus['platform']) =>
      statuses.find((status) => status.platform === platform);
    const getStatus = (platform: ChatPlatformStatus['platform']) =>
      findStatus(platform)?.status ?? 'disconnected';
    // Connect-time send authorization. A connected platform with
    // `canSend === false` is read-only (anonymous Twitch fallback / expired
    // token); for twitch the immediate `twitchReauthNeeded` flag overrides
    // a stale `canSend` that hasn't flipped yet.
    const getCanSend = (platform: ChatPlatformStatus['platform']) => {
      const authed = findStatus(platform)?.canSend ?? false;
      return platform === 'twitch' ? authed && !twitchReauthNeeded : authed;
    };
    const facebookConfigured = (chatSettings.facebookLiveVideoId ?? '').trim().length > 0;

    return [
      {
        id: 'twitch' as const,
        label: t('chat.platforms.twitch'),
        configured: chatSettings.twitchChannel.trim().length > 0,
        sendEnabled: chatSettings.twitchSendEnabled,
        readOnly: false,
        status: getStatus('twitch'),
        canSend: getCanSend('twitch'),
      },
      {
        id: 'youtube' as const,
        label: t('chat.platforms.youtube'),
        configured: chatSettings.youtubeChannelId.trim().length > 0,
        sendEnabled: chatSettings.youtubeSendEnabled && !chatSettings.youtubeUseApiKey,
        readOnly: chatSettings.youtubeUseApiKey,
        status: getStatus('youtube'),
        canSend: getCanSend('youtube'),
      },
      {
        id: 'trovo' as const,
        label: t('chat.platforms.trovo'),
        configured: chatSettings.trovoChannelId.trim().length > 0,
        sendEnabled: chatSettings.trovoSendEnabled,
        readOnly: false,
        status: getStatus('trovo'),
        canSend: getCanSend('trovo'),
      },
      {
        id: 'kick' as const,
        label: t('chat.platforms.kick'),
        configured: chatSettings.kickChannel.trim().length > 0,
        sendEnabled: chatSettings.kickSendEnabled,
        readOnly: false,
        status: getStatus('kick'),
        canSend: getCanSend('kick'),
      },
      {
        id: 'tiktok' as const,
        label: t('chat.platforms.tiktok'),
        configured: chatSettings.tiktokUsername.trim().length > 0,
        sendEnabled: false,
        readOnly: true,
        status: getStatus('tiktok'),
        canSend: false,
      },
      {
        id: 'facebook' as const,
        label: t('chat.platforms.facebook'),
        configured: facebookConfigured,
        sendEnabled: facebookConfigured,
        readOnly: false,
        status: getStatus('facebook'),
        canSend: getCanSend('facebook'),
      },
    ];
  }, [chatSettings, statuses, t, twitchReauthNeeded]);

  // Only badge platforms the user has actually configured — an unconfigured
  // row has no useful state to surface.
  const platformStates = useMemo(
    () => allPlatformStates.filter((row) => row.configured),
    [allPlatformStates]
  );

  const sendDisabledReason = useMemo(() => {
    const configuredPlatforms = allPlatformStates.filter((row) => row.configured);
    // TikTok cannot send by design; exclude from "can we send?" enumeration.
    const sendEnabledPlatforms = allPlatformStates.filter(
      (row) => row.configured && row.sendEnabled && !row.readOnly
    );

    // Chat is decoupled from streaming — no "start a stream first" gate.
    if (configuredPlatforms.length === 0) {
      return t('chat.sendRequiresConfig', {
        defaultValue: 'Sign in or set a channel in Integrations to connect chat.',
      });
    }

    if (sendEnabledPlatforms.length === 0) {
      if (chatSettings.youtubeUseApiKey && !chatSettings.twitchSendEnabled) {
        return t('chat.sendApiKeyReadOnly', {
          defaultValue:
            'YouTube API key mode is read-only. Sign in or enable another platform to send.',
        });
      }
      return t('chat.sendDisabledHint', {
        defaultValue: 'Enable sending in Integrations and connect your chat to send messages.',
      });
    }

    const hasConnecting = statuses.some((status) => status.status === 'connecting');
    if (hasConnecting) {
      return t('chat.sendConnecting', { defaultValue: 'Connecting to chat...' });
    }

    const errorStatus = statuses.find((status) => status.status === 'error' && status.error);
    if (errorStatus?.error) {
      return t('chat.sendError', {
        defaultValue: 'Chat connection error: {{error}}',
        error: errorStatus.error,
      });
    }

    // Configured + send-enabled + connected, but the connection is
    // read-only (anonymous fallback / expired token). Point the user at
    // re-auth instead of the generic "enable sending in Integrations" hint.
    const readOnlyConnected = sendEnabledPlatforms.some(
      (row) => row.status === 'connected' && !row.canSend
    );
    if (readOnlyConnected) {
      return t('chat.sendReauthNeeded', {
        defaultValue: 'Your chat sign-in is read-only — sign in again to send.',
      });
    }

    const connectedCount = statuses.filter((status) => status.status === 'connected').length;
    if (connectedCount === 0) {
      return t('chat.sendNoConnected', {
        defaultValue: 'No chat platforms are connected yet. Check your channel IDs and sign-in.',
      });
    }

    return t('chat.sendDisabledHint', {
      defaultValue: 'Enable sending in Integrations and connect your chat to send messages.',
    });
  }, [allPlatformStates, chatSettings, statuses, t]);

  const handleSend = useCallback(async (): Promise<void> => {
    const trimmed = draftMessage.trim();
    if (!trimmed || isSending) return;
    setIsSending(true);
    try {
      // When the user has disabled broadcast-to-all, pass an explicit
      // single-platform list. This only narrows the target set — core's
      // send path re-checks the per-platform send-enable flags either way.
      const explicitTargets =
        chatSettings.sendAllEnabled || singleTarget === null ? undefined : [singleTarget];
      const results = await api.chat.sendMessage(trimmed, explicitTargets);
      const failures = results.filter((result) => !result.success);
      if (failures.length) {
        toast.error(t('chat.sendPartialFail', 'Some platforms failed to receive your message.'));
      }
      // Keep the draft when NOTHING went out so the user can retry
      // without retyping; clear it once at least one platform took it.
      if (failures.length < results.length) {
        setDraftMessage('');
      }
    } catch (error) {
      toast.error(t('chat.sendFailed', { defaultValue: 'Failed to send message' }));
      logger.error('Failed to send chat message:', error);
    } finally {
      setIsSending(false);
    }
  }, [chatSettings.sendAllEnabled, singleTarget, draftMessage, isSending, t]);

  return (
    <div className="space-y-2">
      {/* Per-message platform picker — only when broadcast-to-all is
          off. When on, the backend dispatches to every connected
          platform whose per-platform send flag is true. */}
      {!chatSettings.sendAllEnabled && sendTargets.length > 0 && (
        <label className="flex items-center gap-2 text-xs">
          <span className="text-text-tertiary">
            {t('chat.sendTo', { defaultValue: 'Send to:' })}
          </span>
          <select
            value={singleTarget ?? ''}
            onChange={(event) =>
              setSingleTarget(event.target.value as ChatPlatformStatus['platform'])
            }
            className="bg-bg-elevated border border-border-subtle rounded px-2 py-1 text-text-primary"
          >
            {sendTargets.map((target) => (
              <option key={target.platform} value={target.platform}>
                {t(`chat.platforms.${target.platform}`)}
              </option>
            ))}
          </select>
        </label>
      )}
      <div className="flex items-center gap-2">
        <div className="flex-1 min-w-0">
          <Input
            value={draftMessage}
            onChange={(event) => setDraftMessage(event.target.value)}
            placeholder={t('chat.sendPlaceholder', { defaultValue: 'Send a message…' })}
            disabled={sendTargets.length === 0}
            onKeyDown={(event) => {
              if (event.key === 'Enter' && !event.shiftKey) {
                event.preventDefault();
                handleSend();
              }
            }}
          />
        </div>
        <Button
          size="icon"
          disabled={!canSend || isSending}
          onClick={handleSend}
          aria-label={t('chat.send', { defaultValue: 'Send' })}
          title={t('chat.send', { defaultValue: 'Send' })}
        >
          <Send className="w-4 h-4" />
        </Button>
      </div>

      {/* When nothing is sendable, surface the actionable reason here. When
          there ARE targets, the platform chips below already show which
          platforms will receive the message — no redundant "Sending to:". */}
      {sendTargets.length === 0 && (
        <p className="text-xs text-text-tertiary">{sendDisabledReason}</p>
      )}

      {platformStates.length > 0 && (
        <div className="flex flex-wrap items-center gap-1.5 text-xs">
          {platformStates.map((platform) => {
            let stateLabel: string;
            if (platform.readOnly) {
              stateLabel = t('chat.platformReadOnly', { defaultValue: 'read-only' });
            } else if (platform.sendEnabled) {
              stateLabel = t('chat.sendOn', { defaultValue: 'on' });
            } else {
              stateLabel = t('chat.sendOff', { defaultValue: 'off' });
            }
            return (
              <span
                key={platform.id}
                className="inline-flex items-center gap-1.5 rounded-full border border-border-subtle bg-bg-elevated px-2 py-0.5"
                title={`${platform.label} — ${stateLabel}`}
              >
                <span
                  className={cn(
                    'inline-block h-2 w-2 rounded-full',
                    statusDotClass(platform.status)
                  )}
                  aria-hidden="true"
                />
                <span className="text-text-primary">{platform.label}</span>
                <span className="text-text-tertiary">· {stateLabel}</span>
              </span>
            );
          })}
        </div>
      )}
    </div>
  );
}
