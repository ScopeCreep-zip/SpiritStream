import { useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { SquareArrowOutUpRight, Trash2, Send, Download, Search } from 'lucide-react';
import { emit } from '@tauri-apps/api/event';
import { Card, CardHeader, CardTitle, CardDescription, CardBody } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { Toggle } from '@/components/ui/Toggle';
import { Input } from '@/components/ui/Input';
import { Modal } from '@/components/ui/Modal';
import { ChatList } from '@/components/chat/ChatList';
import { CHAT_OVERLAY_SETTINGS_EVENT, CHAT_OVERLAY_ALWAYS_ON_TOP_EVENT } from '@/lib/chatEvents';
import { openChatOverlay, setOverlayAlwaysOnTop } from '@/lib/chatWindow';
import { useChatStore } from '@/stores/chatStore';
import { useProfileStore } from '@/stores/profileStore';
import { api, dialogs } from '@/lib/backend';
import type { ChatMessage, ChatPlatformStatus } from '@/types/chat';
import { createDefaultChatSettings } from '@/types/profile';
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
  const currentProfile = useProfileStore((state) => state.current);
  const chatSettings = useMemo(
    () => currentProfile?.settings?.chat ?? createDefaultChatSettings(),
    [currentProfile]
  );
  const [activeStreamCount, setActiveStreamCount] = useState(0);
  const [draftMessage, setDraftMessage] = useState('');
  const [isSending, setIsSending] = useState(false);
  const [searchQuery, setSearchQuery] = useState('');
  const [searchScope, setSearchScope] = useState<'memory' | 'session'>('memory');
  const [searchResults, setSearchResults] = useState<ChatMessage[]>([]);
  const [isSearching, setIsSearching] = useState(false);
  const [searchOpen, setSearchOpen] = useState(false);

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
        const [loadedStatuses, streamCount] = await Promise.all([
          api.chat.getStatus(),
          api.stream.getActiveCount(),
        ]);
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
        const [loadedStatuses, streamCount] = await Promise.all([
          api.chat.getStatus(),
          api.stream.getActiveCount(),
        ]);
        setStatuses(loadedStatuses);
        setActiveStreamCount(streamCount);
      } catch (error) {
        console.error('Failed to refresh chat status:', error);
      }
    }, 5000);

    return () => clearInterval(interval);
  }, []);

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
  }, [sendTargets]);

  const filteredMessages = useMemo(() => {
    if (!searchQuery.trim() || searchScope === 'session') {
      return messages;
    }

    const query = searchQuery.trim().toLowerCase();
    return messages.filter((message) => {
      return (
        message.username.toLowerCase().includes(query) ||
        message.message.toLowerCase().includes(query)
      );
    });
  }, [messages, searchQuery, searchScope]);

  const modalMessages = useMemo(() => {
    if (searchScope === 'session') {
      return searchQuery.trim().length > 0 ? searchResults : [];
    }
    return filteredMessages;
  }, [filteredMessages, searchResults, searchQuery, searchScope]);

  const searchEmptyLabel = useMemo(() => {
    if (!searchQuery.trim()) {
      return searchScope === 'session'
        ? t('chat.searchSessionHint', {
            defaultValue: 'Enter a search term to scan the current stream session.',
          })
        : t('chat.searchMemoryHint', {
            defaultValue: 'Type to filter the messages currently on screen.',
          });
    }

    return t('chat.searchNoResults', { defaultValue: 'No matching messages.' });
  }, [searchQuery, searchScope, t]);

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

  const handleSearchSession = async () => {
    const query = searchQuery.trim();
    if (!query) {
      setSearchResults([]);
      return;
    }

    try {
      setIsSearching(true);
      const results = await api.chat.searchSession(query, 500);
      setSearchResults(results);
    } catch (error) {
      console.error('Failed to search chat session:', error);
      toast.error(
        t('chat.searchFailed', { defaultValue: 'Failed to search chat logs.' })
      );
    } finally {
      setIsSearching(false);
    }
  };

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
  }, [chatSettings, statuses]);

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
          defaultValue: 'YouTube API key mode is read-only. Sign in or enable another platform to send.',
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
  }, [activeStreamCount, chatSettings, statuses, t]);

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
          <CardTitle>{t('chat.viewTitle', { defaultValue: 'Unified Chat' })}</CardTitle>
          <CardDescription>
            {t('chat.viewDescription', {
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
            <Button variant="ghost" size="sm" onClick={() => setSearchOpen(true)}>
              <Search className="w-4 h-4" />
              {t('chat.search', { defaultValue: 'Search' })}
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
      <Modal
        open={searchOpen}
        onClose={() => setSearchOpen(false)}
        title={t('chat.searchTitle', { defaultValue: 'Search chat' })}
        maxWidth="720px"
      >
        <div className="space-y-4">
          <div className="flex flex-wrap items-center gap-3">
            <div className="flex-1 min-w-[240px]">
              <Input
                value={searchQuery}
                onChange={(event) => setSearchQuery(event.target.value)}
                placeholder={t('chat.searchPlaceholder', { defaultValue: 'Search chat...' })}
                onKeyDown={(event) => {
                  if (event.key === 'Enter' && searchScope === 'session') {
                    event.preventDefault();
                    handleSearchSession();
                  }
                }}
              />
            </div>
            <div className="flex items-center gap-2">
              <Button
                variant={searchScope === 'memory' ? 'secondary' : 'ghost'}
                size="sm"
                onClick={() => setSearchScope('memory')}
              >
                {t('chat.searchInMemory', { defaultValue: 'On screen' })}
              </Button>
              <Button
                variant={searchScope === 'session' ? 'secondary' : 'ghost'}
                size="sm"
                onClick={() => setSearchScope('session')}
                disabled={activeStreamCount === 0}
              >
                {t('chat.searchSession', { defaultValue: 'Full session' })}
              </Button>
              {searchScope === 'session' && (
                <Button
                  size="sm"
                  onClick={handleSearchSession}
                  disabled={!searchQuery.trim() || isSearching}
                  className="gap-2"
                >
                  <Search className="w-4 h-4" />
                  {isSearching
                    ? t('chat.searching', { defaultValue: 'Searching...' })
                    : t('chat.search', { defaultValue: 'Search' })}
                </Button>
              )}
            </div>
          </div>
          <ChatList
            messages={modalMessages}
            className="max-h-[480px]"
            emptyLabel={searchEmptyLabel}
            showTimestamps
          />
          <div className="flex items-center justify-between text-xs text-[var(--text-tertiary)]">
            <span>
              {searchScope === 'session'
                ? t('chat.searchSessionHint', {
                    defaultValue: 'Enter a search term to scan the current stream session.',
                  })
                : t('chat.searchMemoryHint', {
                    defaultValue: 'Type to filter the messages currently on screen.',
                  })}
            </span>
            <span>{t('chat.searchLimit', { defaultValue: 'Up to 500 matches.' })}</span>
          </div>
        </div>
      </Modal>
    </Card>
  );
}
