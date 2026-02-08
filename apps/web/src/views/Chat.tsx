import { useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { SquareArrowOutUpRight, Trash2, Send, Download } from 'lucide-react';
import { emit } from '@tauri-apps/api/event';
import { Card, CardHeader, CardTitle, CardDescription, CardBody } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { Toggle } from '@/components/ui/Toggle';
import { Input } from '@/components/ui/Input';
import { ChatList } from '@/components/chat/ChatList';
import { CHAT_OVERLAY_SETTINGS_EVENT, CHAT_OVERLAY_ALWAYS_ON_TOP_EVENT } from '@/lib/chatEvents';
import { openChatOverlay, setOverlayAlwaysOnTop } from '@/lib/chatWindow';
import { useChatStore } from '@/stores/chatStore';
import { api, dialogs } from '@/lib/backend';
import type { ChatPlatformStatus } from '@/types/chat';
import type { AppSettings } from '@/types/api';
import { toast } from '@/hooks/useToast';
import { cn } from '@/lib/cn';

export function Chat() {
  const { t } = useTranslation();
  const messages = useChatStore((state) => state.messages);
  const overlayTransparent = useChatStore((state) => state.overlayTransparent);
  const setOverlayTransparent = useChatStore((state) => state.setOverlayTransparent);
  const overlayAlwaysOnTop = useChatStore((state) => state.overlayAlwaysOnTop);
  const setOverlayAlwaysOnTopState = useChatStore((state) => state.setOverlayAlwaysOnTop);
  const clearMessages = useChatStore((state) => state.clearMessages);
  const [statuses, setStatuses] = useState<ChatPlatformStatus[]>([]);
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [activeStreamCount, setActiveStreamCount] = useState(0);
  const [draftMessage, setDraftMessage] = useState('');
  const [isSending, setIsSending] = useState(false);

  const handleTransparentToggle = (transparent: boolean) => {
    setOverlayTransparent(transparent);
    emit(CHAT_OVERLAY_SETTINGS_EVENT, { transparent }).catch((error) => {
      console.error('Failed to sync chat overlay settings:', error);
    });
  };

  const handleAlwaysOnTopToggle = (alwaysOnTop: boolean) => {
    setOverlayAlwaysOnTopState(alwaysOnTop);
    setOverlayAlwaysOnTop(alwaysOnTop);
    emit(CHAT_OVERLAY_ALWAYS_ON_TOP_EVENT, { alwaysOnTop }).catch((error) => {
      console.error('Failed to sync chat overlay always on top:', error);
    });
  };

  useEffect(() => {
    const load = async () => {
      try {
        const [loadedSettings, loadedStatuses, streamCount] = await Promise.all([
          api.settings.get(),
          api.chat.getStatus(),
          api.stream.getActiveCount(),
        ]);
        setSettings(loadedSettings);
        setStatuses(loadedStatuses);
        setActiveStreamCount(streamCount);
      } catch (error) {
        console.error('Failed to load chat settings:', error);
      }
    };
    load();
  }, []);

  useEffect(() => {
    const interval = setInterval(async () => {
      try {
        const [loadedStatuses, loadedSettings, streamCount] = await Promise.all([
          api.chat.getStatus(),
          api.settings.get(),
          api.stream.getActiveCount(),
        ]);
        setStatuses(loadedStatuses);
        setSettings(loadedSettings);
        setActiveStreamCount(streamCount);
      } catch (error) {
        console.error('Failed to refresh chat status:', error);
      }
    }, 5000);

    return () => clearInterval(interval);
  }, []);

  const sendTargets = useMemo(() => {
    if (!settings) return [];
    return statuses.filter((status) => {
      if (status.status !== 'connected') return false;
      if (status.platform === 'twitch') return settings.chatTwitchSendEnabled;
      if (status.platform === 'youtube') return settings.chatYoutubeSendEnabled;
      return false;
    });
  }, [settings, statuses]);

  const canSend = draftMessage.trim().length > 0 && sendTargets.length > 0;
  const sendTargetLabel = useMemo(() => {
    if (sendTargets.length === 0) return '';
    return sendTargets
      .map((target) => (target.platform === 'twitch' ? 'Twitch' : target.platform === 'youtube' ? 'YouTube' : target.platform))
      .join(', ');
  }, [sendTargets]);

  const formatTimestampForFile = (date: Date) => {
    const pad = (value: number) => String(value).padStart(2, '0');
    return `${date.getFullYear()}${pad(date.getMonth() + 1)}${pad(date.getDate())}-${pad(
      date.getHours()
    )}${pad(date.getMinutes())}${pad(date.getSeconds())}`;
  };

  const handleExportLog = async () => {
    try {
      if (activeStreamCount === 0) {
        toast.error(
          t('chat.exportRequiresStream', {
            defaultValue: 'Start a stream to export the current chat session.',
          })
        );
        return;
      }

      const status = await api.chat.getLogStatus();
      if (!status.active || !status.startedAt) {
        toast.error(
          t('chat.exportNoSession', {
            defaultValue: 'No active chat session to export.',
          })
        );
        return;
      }

      const start = new Date(status.startedAt);
      const end = new Date();
      const defaultName = `chatlog_${formatTimestampForFile(start)}_to_${formatTimestampForFile(
        end
      )}.jsonl`;

      const path = await dialogs.saveFilePath({
        defaultPath: defaultName,
        filters: [{ name: 'JSONL', extensions: ['jsonl'] }],
      });

      if (!path) {
        return;
      }

      await api.chat.exportLog(path);
      toast.success(
        t('chat.exportSuccess', { defaultValue: 'Chat log exported.' })
      );
    } catch (error) {
      console.error('Failed to export chat log:', error);
      toast.error(
        t('chat.exportFailed', { defaultValue: 'Failed to export chat log.' })
      );
    }
  };

  const platformStates = useMemo(() => {
    if (!settings) return [];

    const getStatus = (platform: ChatPlatformStatus['platform']) =>
      statuses.find((status) => status.platform === platform)?.status ?? 'disconnected';

    return [
      {
        id: 'twitch',
        label: 'Twitch',
        configured: settings.chatTwitchChannel.trim().length > 0,
        sendEnabled: settings.chatTwitchSendEnabled,
        status: getStatus('twitch'),
      },
      {
        id: 'youtube',
        label: 'YouTube',
        configured: settings.chatYoutubeChannelId.trim().length > 0,
        sendEnabled: settings.chatYoutubeSendEnabled,
        status: getStatus('youtube'),
      },
    ];
  }, [settings, statuses]);

  const statusDotClass = (status: ChatPlatformStatus['status']) => {
    switch (status) {
      case 'connected':
        return 'bg-[var(--status-live)]';
      case 'connecting':
        return 'bg-[var(--status-connecting)]';
      case 'error':
        return 'bg-[var(--status-error)]';
      default:
        return 'bg-[var(--text-tertiary)]';
    }
  };

  const sendDisabledReason = useMemo(() => {
    if (!settings) {
      return t('common.loading', { defaultValue: 'Loading...' });
    }

    const isStreaming = activeStreamCount > 0;
    const configuredPlatforms = [
      settings.chatTwitchChannel.trim() ? 'twitch' : null,
      settings.chatYoutubeChannelId.trim() ? 'youtube' : null,
    ].filter(Boolean);

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
  }, [activeStreamCount, settings, statuses, t]);

  const handleSend = async () => {
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
      console.error('Failed to send chat message:', error);
    } finally {
      setIsSending(false);
    }
  };

  return (
    <Card>
      <CardHeader>
        <div>
          <CardTitle>{t('chat.title', { defaultValue: 'Unified Chat' })}</CardTitle>
          <CardDescription>
            {t('chat.description', {
              defaultValue: 'Unified chat from your connected platforms. Sending uses your enabled accounts.',
            })}
          </CardDescription>
        </div>
      </CardHeader>
      <CardBody>
        <div className="flex flex-wrap items-center justify-between" style={{ gap: '16px' }}>
          <div className="flex flex-wrap items-center" style={{ gap: '24px' }}>
            <Toggle
              checked={overlayTransparent}
              onChange={handleTransparentToggle}
              label={t('chat.transparentOverlay', { defaultValue: 'Transparent overlay' })}
              description={t('chat.transparentOverlayDescription', {
                defaultValue: 'Makes the pop-out window background transparent.',
              })}
            />
            <Toggle
              checked={overlayAlwaysOnTop}
              onChange={handleAlwaysOnTopToggle}
              label={t('chat.alwaysOnTop', { defaultValue: 'Always on top' })}
              description={t('chat.alwaysOnTopDescription', {
                defaultValue: 'Keeps the pop-out window above other windows.',
              })}
            />
          </div>
          <div className="flex items-center" style={{ gap: '12px' }}>
            <Button variant="ghost" size="sm" onClick={clearMessages}>
              <Trash2 className="w-4 h-4" />
              {t('common.clear')}
            </Button>
            <Button
              variant="ghost"
              size="sm"
              onClick={handleExportLog}
              disabled={activeStreamCount === 0}
            >
              <Download className="w-4 h-4" />
              {t('chat.exportLog', { defaultValue: 'Export chat' })}
            </Button>
            <Button size="sm" onClick={openChatOverlay}>
              <SquareArrowOutUpRight className="w-4 h-4" />
              {t('chat.popOut', { defaultValue: 'Pop out' })}
            </Button>
          </div>
        </div>
        <div className="mt-6 rounded-xl border border-[var(--border-subtle)] bg-[var(--bg-elevated)] p-4">
          <ChatList
            messages={messages}
            className="max-h-[520px]"
            emptyLabel={t('chat.empty', { defaultValue: 'No chat messages yet.' })}
            showTimestamps
          />
        </div>
        <div className="mt-4 space-y-2">
          <div className="flex items-center gap-3">
            <div className="flex-1 min-w-0">
              <Input
                value={draftMessage}
                onChange={(event) => setDraftMessage(event.target.value)}
                placeholder={t('chat.sendPlaceholder', { defaultValue: 'Send a message to all enabled chats...' })}
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
              size="sm"
              disabled={!canSend || isSending}
              onClick={handleSend}
              className="gap-2"
            >
              <Send className="w-4 h-4" />
              {t('chat.send', { defaultValue: 'Send' })}
            </Button>
          </div>
          {sendTargets.length === 0 ? (
            <p className="text-xs text-[var(--text-tertiary)]">
              {sendDisabledReason}
            </p>
          ) : (
            <p className="text-xs text-[var(--text-tertiary)]">
              {t('chat.sendTargets', {
                defaultValue: 'Sending to: {{targets}}',
                targets: sendTargetLabel,
              })}
            </p>
          )}
          {platformStates.length > 0 && (
            <div className="flex flex-wrap items-center gap-2 text-xs text-[var(--text-tertiary)]">
              <span className="font-medium text-[var(--text-secondary)]">
                {t('chat.sendStatus', { defaultValue: 'Send status:' })}
              </span>
              {platformStates.map((platform) => (
                <div
                  key={platform.id}
                  className="flex items-center gap-2 rounded-full border border-[var(--border-subtle)] bg-[var(--bg-elevated)] px-2 py-1"
                >
                  <span
                    className={cn('inline-block h-2 w-2 rounded-full', statusDotClass(platform.status))}
                  />
                  <span className="text-[var(--text-primary)]">{platform.label}</span>
                  <span>
                    {platform.configured
                      ? t('chat.platformConfigured', { defaultValue: 'Configured' })
                      : t('chat.platformNotConfigured', { defaultValue: 'Not configured' })}
                  </span>
                  <span>
                    {platform.sendEnabled
                      ? t('chat.sendOn', { defaultValue: 'Send on' })
                      : t('chat.sendOff', { defaultValue: 'Send off' })}
                  </span>
                </div>
              ))}
            </div>
          )}
        </div>
      </CardBody>
    </Card>
  );
}
