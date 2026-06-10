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
import type { ChatPlatformStatus } from '@spiritstream/types';

interface ChatComposerProps {
  statuses: ChatPlatformStatus[];
  activeStreamCount: number;
}

/**
 * Send draft + platform pills + the "why can't I send?" hint. Owns its
 * own draftMessage + isSending state; reads statuses + activeStreamCount
 * from the parent's `useChatPlatformStatus` hook so we don't double-poll.
 *
 * Backend decides what's valid to send: this component only formats the
 * draft and calls `api.chat.sendMessage`.
 */
export function ChatComposer({
  statuses,
  activeStreamCount,
}: ChatComposerProps): React.ReactElement {
  const { t } = useTranslation();
  const currentProfile = useProfileStore((state) => state.current);
  const chatSettings = useMemo(
    () => currentProfile?.settings?.chat ?? createDefaultChatSettings(),
    [currentProfile]
  );
  const [draftMessage, setDraftMessage] = useState('');
  const [isSending, setIsSending] = useState(false);
  // Per-message target when the user has disabled broadcast-to-all on
  // the chat-settings panel. Picks the first connected send-target by
  // default so the dropdown is never empty when shown.
  const [singleTarget, setSingleTarget] = useState<ChatPlatformStatus['platform'] | null>(null);

  const sendTargets = useMemo(() => {
    return statuses.filter((status) => {
      if (status.status !== 'connected') return false;
      if (status.platform === 'twitch') return chatSettings.twitchSendEnabled;
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
  }, [chatSettings, statuses]);

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

  const sendTargetLabel = useMemo(() => {
    if (sendTargets.length === 0) return '';
    return sendTargets
      .map((target) => {
        if (target.platform === 'twitch') return t('chat.platforms.twitch');
        if (target.platform === 'youtube') return t('chat.platforms.youtube');
        if (target.platform === 'trovo') return t('chat.platforms.trovo');
        if (target.platform === 'kick') return t('chat.platforms.kick');
        if (target.platform === 'facebook') return t('chat.platforms.facebook');
        if (target.platform === 'tiktok') return t('chat.platforms.tiktok');
        return target.platform;
      })
      .join(', ');
  }, [sendTargets, t]);

  // Single source of truth for "what state is each chat platform in?" —
  // drives both the bottom badge row and the disabled-send hint below.
  // Mirrors the configured/send-enabled logic used by ChatPanel's auto-
  // visibility list and by `sendTargets` above; tiktok is read-only,
  // facebook send is auth-gated server-side (config = opted-in via the
  // confirm-token gate at connect).
  const allPlatformStates = useMemo(() => {
    const getStatus = (platform: ChatPlatformStatus['platform']) =>
      statuses.find((status) => status.platform === platform)?.status ?? 'disconnected';
    const facebookConfigured = (chatSettings.facebookLiveVideoId ?? '').trim().length > 0;

    return [
      {
        id: 'twitch' as const,
        label: t('chat.platforms.twitch'),
        configured: chatSettings.twitchChannel.trim().length > 0,
        sendEnabled: chatSettings.twitchSendEnabled,
        readOnly: false,
        status: getStatus('twitch'),
      },
      {
        id: 'youtube' as const,
        label: t('chat.platforms.youtube'),
        configured: chatSettings.youtubeChannelId.trim().length > 0,
        sendEnabled: chatSettings.youtubeSendEnabled && !chatSettings.youtubeUseApiKey,
        readOnly: chatSettings.youtubeUseApiKey,
        status: getStatus('youtube'),
      },
      {
        id: 'trovo' as const,
        label: t('chat.platforms.trovo'),
        configured: chatSettings.trovoChannelId.trim().length > 0,
        sendEnabled: chatSettings.trovoSendEnabled,
        readOnly: false,
        status: getStatus('trovo'),
      },
      {
        id: 'kick' as const,
        label: t('chat.platforms.kick'),
        configured: chatSettings.kickChannel.trim().length > 0,
        sendEnabled: chatSettings.kickSendEnabled,
        readOnly: false,
        status: getStatus('kick'),
      },
      {
        id: 'tiktok' as const,
        label: t('chat.platforms.tiktok'),
        configured: chatSettings.tiktokUsername.trim().length > 0,
        sendEnabled: false,
        readOnly: true,
        status: getStatus('tiktok'),
      },
      {
        id: 'facebook' as const,
        label: t('chat.platforms.facebook'),
        configured: facebookConfigured,
        sendEnabled: facebookConfigured,
        readOnly: false,
        status: getStatus('facebook'),
      },
    ];
  }, [chatSettings, statuses, t]);

  // Only badge platforms the user has actually configured — an unconfigured
  // row has no useful state to surface.
  const platformStates = useMemo(
    () => allPlatformStates.filter((row) => row.configured),
    [allPlatformStates]
  );

  const sendDisabledReason = useMemo(() => {
    const isStreaming = activeStreamCount > 0;
    const configuredPlatforms = allPlatformStates.filter((row) => row.configured);
    // TikTok cannot send by design; exclude from "can we send?" enumeration.
    const sendEnabledPlatforms = allPlatformStates.filter(
      (row) => row.configured && row.sendEnabled && !row.readOnly
    );

    if (!isStreaming) {
      return t('chat.sendRequiresStream', {
        defaultValue: 'Chat connects when you start streaming. Start a stream to enable sending.',
      });
    }

    if (configuredPlatforms.length === 0) {
      return t('chat.sendRequiresConfig', {
        defaultValue: 'Configure a chat platform in Integrations to enable sending.',
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

    const connectedCount = statuses.filter((status) => status.status === 'connected').length;
    if (connectedCount === 0) {
      return t('chat.sendNoConnected', {
        defaultValue: 'No chat platforms are connected yet. Check your channel IDs and sign-in.',
      });
    }

    return t('chat.sendDisabledHint', {
      defaultValue: 'Enable sending in Integrations and connect your chat to send messages.',
    });
  }, [activeStreamCount, allPlatformStates, chatSettings, statuses, t]);

  const handleSend = useCallback(async (): Promise<void> => {
    const trimmed = draftMessage.trim();
    if (!trimmed || isSending) return;
    setIsSending(true);
    try {
      // When the user has disabled broadcast-to-all, pass an explicit
      // single-platform list to the backend so it bypasses the
      // per-platform send-enable flags and dispatches only there.
      const explicitTargets =
        chatSettings.sendAllEnabled || singleTarget === null ? undefined : [singleTarget];
      const results = await api.chat.sendMessage(trimmed, explicitTargets);
      const failures = results.filter((result) => !result.success);
      if (failures.length) {
        toast.error(t('chat.sendPartialFail', 'Some platforms failed to receive your message.'));
      }
      setDraftMessage('');
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

      <p className="text-xs text-text-tertiary">
        {sendTargets.length === 0
          ? sendDisabledReason
          : t('chat.sendTargets', {
              defaultValue: 'Sending to: {{targets}}',
              targets: sendTargetLabel,
            })}
      </p>

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

function statusDotClass(status: ChatPlatformStatus['status']): string {
  switch (status) {
    case 'connected':
      return 'bg-status-live';
    case 'connecting':
      return 'bg-status-connecting';
    case 'error':
      return 'bg-status-error';
    default:
      return 'bg-text-tertiary';
  }
}
