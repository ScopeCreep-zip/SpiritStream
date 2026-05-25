import { useCallback, useMemo, useState } from 'react';
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
export function ChatComposer({ statuses, activeStreamCount }: ChatComposerProps): React.ReactElement {
  const { t } = useTranslation();
  const currentProfile = useProfileStore((state) => state.current);
  const chatSettings = useMemo(
    () => currentProfile?.settings?.chat ?? createDefaultChatSettings(),
    [currentProfile],
  );
  const [draftMessage, setDraftMessage] = useState('');
  const [isSending, setIsSending] = useState(false);

  const sendTargets = useMemo(() => {
    return statuses.filter((status) => {
      if (status.status !== 'connected') return false;
      if (status.platform === 'twitch') return chatSettings.twitchSendEnabled;
      if (status.platform === 'youtube') {
        return chatSettings.youtubeSendEnabled && !chatSettings.youtubeUseApiKey;
      }
      if (status.platform === 'trovo') return chatSettings.trovoSendEnabled;
      return false;
    });
  }, [chatSettings, statuses]);

  const canSend = draftMessage.trim().length > 0 && sendTargets.length > 0;

  const sendTargetLabel = useMemo(() => {
    if (sendTargets.length === 0) return '';
    return sendTargets
      .map((target) => {
        if (target.platform === 'twitch') return t('chat.platforms.twitch');
        if (target.platform === 'youtube') return t('chat.platforms.youtube');
        if (target.platform === 'trovo') return t('chat.platforms.trovo');
        return target.platform;
      })
      .join(', ');
  }, [sendTargets, t]);

  const platformStates = useMemo(() => {
    const getStatus = (platform: ChatPlatformStatus['platform']) =>
      statuses.find((status) => status.platform === platform)?.status ?? 'disconnected';

    return [
      {
        id: 'twitch',
        label: t('chat.platforms.twitch'),
        configured: chatSettings.twitchChannel.trim().length > 0,
        sendEnabled: chatSettings.twitchSendEnabled,
        status: getStatus('twitch'),
      },
      {
        id: 'youtube',
        label: t('chat.platforms.youtube'),
        configured: chatSettings.youtubeChannelId.trim().length > 0,
        sendEnabled: chatSettings.youtubeSendEnabled && !chatSettings.youtubeUseApiKey,
        status: getStatus('youtube'),
      },
      {
        id: 'trovo',
        label: t('chat.platforms.trovo'),
        configured: chatSettings.trovoChannelId.trim().length > 0,
        sendEnabled: chatSettings.trovoSendEnabled,
        status: getStatus('trovo'),
      },
    ];
  }, [chatSettings, statuses, t]);

  const sendDisabledReason = useMemo(() => {
    const isStreaming = activeStreamCount > 0;
    const configuredPlatforms = [
      chatSettings.twitchChannel.trim() ? 'twitch' : null,
      chatSettings.youtubeChannelId.trim() ? 'youtube' : null,
      chatSettings.trovoChannelId.trim() ? 'trovo' : null,
    ].filter(Boolean);
    const sendEnabledPlatforms = [
      chatSettings.twitchSendEnabled ? 'twitch' : null,
      chatSettings.youtubeSendEnabled && !chatSettings.youtubeUseApiKey ? 'youtube' : null,
      chatSettings.trovoSendEnabled ? 'trovo' : null,
    ].filter(Boolean);

    if (!isStreaming) {
      return t('chat.sendRequiresStream', {
        defaultValue:
          'Chat connects when you start streaming. Start a stream to enable sending.',
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
        defaultValue:
          'No chat platforms are connected yet. Check your channel IDs and sign-in.',
      });
    }

    return t('chat.sendDisabledHint', {
      defaultValue: 'Enable sending in Integrations and connect your chat to send messages.',
    });
  }, [activeStreamCount, chatSettings, statuses, t]);

  const handleSend = useCallback(async (): Promise<void> => {
    const trimmed = draftMessage.trim();
    if (!trimmed || isSending) return;
    setIsSending(true);
    try {
      const results = await api.chat.sendMessage(trimmed);
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
  }, [draftMessage, isSending, t]);

  return (
    <div className="space-y-2">
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
            const stateLabel = !platform.configured
              ? t('chat.platformNotConfigured', { defaultValue: 'not configured' })
              : platform.sendEnabled
                ? t('chat.sendOn', { defaultValue: 'on' })
                : t('chat.sendOff', { defaultValue: 'off' });
            return (
              <span
                key={platform.id}
                className="inline-flex items-center gap-1.5 rounded-full border border-border-subtle bg-bg-elevated px-2 py-0.5"
                title={`${platform.label} — ${stateLabel}`}
              >
                <span
                  className={cn('inline-block h-2 w-2 rounded-full', statusDotClass(platform.status))}
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
