import { useState, useCallback, useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import {
  MessageSquare,
  Wifi,
  WifiOff,
  Loader2,
  AlertCircle,
  Eye,
  EyeOff,
} from 'lucide-react';
import { Card, CardHeader, CardTitle, CardDescription, CardBody } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { Input } from '@/components/ui/Input';
import { Toggle } from '@/components/ui/Toggle';
import { api } from '@/lib/backend';
import { toast } from '@/hooks/useToast';
import { cn } from '@/lib/cn';
import type { ChatPlatform, ChatPlatformStatus, ChatConfig, ChatCredentials } from '@/types/chat';
import type { AppSettings } from '@/types/api';

interface PlatformCardProps {
  platform: ChatPlatform;
  platformName: string;
  icon: React.ReactNode;
  iconColor: string;
  status: ChatPlatformStatus | null;
  isConnecting: boolean;
  onConnect: (config: ChatConfig) => Promise<void>;
  onDisconnect: () => Promise<void>;
}

interface TwitchCardProps extends Omit<PlatformCardProps, 'platform' | 'platformName' | 'icon' | 'iconColor'> {
  initialChannel: string;
  initialOauthToken: string;
  onCredentialsChange: (channel: string, oauthToken: string) => void;
}

function TwitchCard({
  status,
  isConnecting,
  onConnect,
  onDisconnect,
  initialChannel,
  initialOauthToken,
  onCredentialsChange,
}: TwitchCardProps) {
  const { t } = useTranslation();
  const [channel, setChannel] = useState(initialChannel);
  const [oauthToken, setOauthToken] = useState(initialOauthToken);
  const [showToken, setShowToken] = useState(false);

  // Update state when initial values change (settings loaded)
  useEffect(() => {
    setChannel(initialChannel);
    setOauthToken(initialOauthToken);
  }, [initialChannel, initialOauthToken]);

  const isConnected = status?.status === 'connected';
  const hasError = status?.status === 'error';

  const handleConnect = useCallback(async () => {
    if (!channel.trim()) {
      toast.error(t('chat.twitch.enterChannel'));
      return;
    }

    // Notify parent of credentials for saving
    onCredentialsChange(channel.trim(), oauthToken.trim());

    const credentials: ChatCredentials = {
      type: 'twitch',
      channel: channel.trim(),
      oauthToken: oauthToken.trim() || undefined,
    };

    const config: ChatConfig = {
      platform: 'twitch',
      enabled: true,
      credentials,
    };

    await onConnect(config);
  }, [channel, oauthToken, onConnect, onCredentialsChange, t]);

  return (
    <Card>
      <CardHeader>
        <div className="flex items-center gap-3">
          <div className="p-2 rounded-lg bg-[#9146FF]/10">
            <svg className="w-5 h-5" viewBox="0 0 24 24" fill="#9146FF">
              <path d="M11.571 4.714h1.715v5.143H11.57zm4.715 0H18v5.143h-1.714zM6 0L1.714 4.286v15.428h5.143V24l4.286-4.286h3.428L22.286 12V0zm14.571 11.143l-3.428 3.428h-3.429l-3 3v-3H6.857V1.714h13.714Z" />
            </svg>
          </div>
          <div className="flex-1">
            <CardTitle>{t('chat.twitch.title')}</CardTitle>
            <CardDescription>{t('chat.twitch.description')}</CardDescription>
          </div>
          {/* Status indicator */}
          <div className="flex items-center gap-2">
            {isConnecting ? (
              <Loader2 className="w-4 h-4 text-[var(--status-connecting)] animate-spin" />
            ) : isConnected ? (
              <Wifi className="w-4 h-4 text-[var(--status-live)]" />
            ) : hasError ? (
              <AlertCircle className="w-4 h-4 text-[var(--status-error)]" />
            ) : (
              <WifiOff className="w-4 h-4 text-[var(--text-tertiary)]" />
            )}
            <span
              className={cn(
                'text-sm',
                isConnected
                  ? 'text-[var(--status-live)]'
                  : hasError
                    ? 'text-[var(--status-error)]'
                    : 'text-[var(--text-tertiary)]'
              )}
            >
              {isConnecting
                ? t('chat.connecting')
                : isConnected
                  ? t('chat.connected')
                  : hasError
                    ? t('chat.error')
                    : t('chat.disconnected')}
            </span>
          </div>
        </div>
      </CardHeader>
      <CardBody className="space-y-4">
        {/* Channel input */}
        <Input
          label={t('chat.twitch.channel')}
          value={channel}
          onChange={(e) => setChannel(e.target.value)}
          placeholder={t('chat.twitch.channelPlaceholder')}
          disabled={isConnected || isConnecting}
        />

        {/* OAuth Token (optional) */}
        <div className="relative">
          <Input
            label={t('chat.twitch.oauthToken')}
            type={showToken ? 'text' : 'password'}
            value={oauthToken}
            onChange={(e) => setOauthToken(e.target.value)}
            placeholder={t('chat.twitch.oauthTokenPlaceholder')}
            disabled={isConnected || isConnecting}
          />
          <button
            type="button"
            onClick={() => setShowToken(!showToken)}
            className={cn(
              'absolute right-3 top-[34px]',
              'p-1 rounded-md',
              'text-[var(--text-tertiary)] hover:text-[var(--text-primary)]',
              'transition-colors'
            )}
          >
            {showToken ? <EyeOff className="w-4 h-4" /> : <Eye className="w-4 h-4" />}
          </button>
          <p className="text-xs text-[var(--text-tertiary)] mt-1">
            {t('chat.twitch.oauthTokenHint')}
          </p>
        </div>

        {/* Error message */}
        {hasError && status?.error && (
          <div className="flex items-start gap-2 p-3 rounded-lg bg-[var(--status-error)]/10 border border-[var(--status-error)]/20">
            <AlertCircle className="w-4 h-4 text-[var(--status-error)] flex-shrink-0 mt-0.5" />
            <p className="text-sm text-[var(--status-error)]">{status.error}</p>
          </div>
        )}

        {/* Message count */}
        {isConnected && status && (
          <div className="text-sm text-[var(--text-secondary)]">
            {t('chat.messageCount', { count: status.messageCount })}
          </div>
        )}

        {/* Connect/Disconnect button */}
        <div className="flex justify-end">
          {isConnected ? (
            <Button variant="outline" onClick={onDisconnect} disabled={isConnecting}>
              <WifiOff className="w-4 h-4" />
              {t('chat.disconnect')}
            </Button>
          ) : (
            <Button variant="primary" onClick={handleConnect} disabled={isConnecting || !channel.trim()}>
              {isConnecting ? (
                <Loader2 className="w-4 h-4 animate-spin" />
              ) : (
                <Wifi className="w-4 h-4" />
              )}
              {t('chat.connect')}
            </Button>
          )}
        </div>
      </CardBody>
    </Card>
  );
}

interface YouTubeCardProps extends Omit<PlatformCardProps, 'platform' | 'platformName' | 'icon' | 'iconColor'> {
  initialChannelId: string;
  initialApiKey: string;
  onCredentialsChange: (channelId: string, apiKey: string) => void;
}

function YouTubeCard({
  status,
  isConnecting,
  onConnect,
  onDisconnect,
  initialChannelId,
  initialApiKey,
  onCredentialsChange,
}: YouTubeCardProps) {
  const { t } = useTranslation();
  const [channelId, setChannelId] = useState(initialChannelId);
  const [apiKey, setApiKey] = useState(initialApiKey);
  const [showApiKey, setShowApiKey] = useState(false);

  // Update state when initial values change (settings loaded)
  useEffect(() => {
    setChannelId(initialChannelId);
    setApiKey(initialApiKey);
  }, [initialChannelId, initialApiKey]);

  const isConnected = status?.status === 'connected';
  const hasError = status?.status === 'error';

  const handleConnect = useCallback(async () => {
    if (!channelId.trim()) {
      toast.error(t('chat.youtube.enterChannelId'));
      return;
    }
    if (!apiKey.trim()) {
      toast.error(t('chat.youtube.enterApiKey'));
      return;
    }

    // Notify parent of credentials for saving
    onCredentialsChange(channelId.trim(), apiKey.trim());

    const credentials: ChatCredentials = {
      type: 'youtube',
      channelId: channelId.trim(),
      apiKey: apiKey.trim(),
    };

    const config: ChatConfig = {
      platform: 'youtube',
      enabled: true,
      credentials,
    };

    await onConnect(config);
  }, [channelId, apiKey, onConnect, onCredentialsChange, t]);

  return (
    <Card>
      <CardHeader>
        <div className="flex items-center gap-3">
          <div className="p-2 rounded-lg bg-[#FF0000]/10">
            <svg className="w-5 h-5" viewBox="0 0 24 24" fill="#FF0000">
              <path d="M23.498 6.186a3.016 3.016 0 0 0-2.122-2.136C19.505 3.545 12 3.545 12 3.545s-7.505 0-9.377.505A3.017 3.017 0 0 0 .502 6.186C0 8.07 0 12 0 12s0 3.93.502 5.814a3.016 3.016 0 0 0 2.122 2.136c1.871.505 9.376.505 9.376.505s7.505 0 9.377-.505a3.015 3.015 0 0 0 2.122-2.136C24 15.93 24 12 24 12s0-3.93-.502-5.814zM9.545 15.568V8.432L15.818 12l-6.273 3.568z" />
            </svg>
          </div>
          <div className="flex-1">
            <CardTitle>{t('chat.youtube.title')}</CardTitle>
            <CardDescription>{t('chat.youtube.description')}</CardDescription>
          </div>
          {/* Status indicator */}
          <div className="flex items-center gap-2">
            {isConnecting ? (
              <Loader2 className="w-4 h-4 text-[var(--status-connecting)] animate-spin" />
            ) : isConnected ? (
              <Wifi className="w-4 h-4 text-[var(--status-live)]" />
            ) : hasError ? (
              <AlertCircle className="w-4 h-4 text-[var(--status-error)]" />
            ) : (
              <WifiOff className="w-4 h-4 text-[var(--text-tertiary)]" />
            )}
            <span
              className={cn(
                'text-sm',
                isConnected
                  ? 'text-[var(--status-live)]'
                  : hasError
                    ? 'text-[var(--status-error)]'
                    : 'text-[var(--text-tertiary)]'
              )}
            >
              {isConnecting
                ? t('chat.connecting')
                : isConnected
                  ? t('chat.connected')
                  : hasError
                    ? t('chat.error')
                    : t('chat.disconnected')}
            </span>
          </div>
        </div>
      </CardHeader>
      <CardBody className="space-y-4">
        {/* Channel ID input */}
        <div>
          <Input
            label={t('chat.youtube.channelId')}
            value={channelId}
            onChange={(e) => setChannelId(e.target.value)}
            placeholder={t('chat.youtube.channelIdPlaceholder')}
            disabled={isConnected || isConnecting}
          />
          <p className="text-xs text-[var(--text-tertiary)] mt-1">
            {t('chat.youtube.channelIdHint')}
          </p>
        </div>

        {/* API Key */}
        <div className="relative">
          <Input
            label={t('chat.youtube.apiKey')}
            type={showApiKey ? 'text' : 'password'}
            value={apiKey}
            onChange={(e) => setApiKey(e.target.value)}
            placeholder={t('chat.youtube.apiKeyPlaceholder')}
            disabled={isConnected || isConnecting}
          />
          <button
            type="button"
            onClick={() => setShowApiKey(!showApiKey)}
            className={cn(
              'absolute right-3 top-[34px]',
              'p-1 rounded-md',
              'text-[var(--text-tertiary)] hover:text-[var(--text-primary)]',
              'transition-colors'
            )}
          >
            {showApiKey ? <EyeOff className="w-4 h-4" /> : <Eye className="w-4 h-4" />}
          </button>
          <p className="text-xs text-[var(--text-tertiary)] mt-1">
            {t('chat.youtube.apiKeyHint')}
          </p>
        </div>

        {/* Error message */}
        {hasError && status?.error && (
          <div className="flex items-start gap-2 p-3 rounded-lg bg-[var(--status-error)]/10 border border-[var(--status-error)]/20">
            <AlertCircle className="w-4 h-4 text-[var(--status-error)] flex-shrink-0 mt-0.5" />
            <p className="text-sm text-[var(--status-error)]">{status.error}</p>
          </div>
        )}

        {/* Message count */}
        {isConnected && status && (
          <div className="text-sm text-[var(--text-secondary)]">
            {t('chat.messageCount', { count: status.messageCount })}
          </div>
        )}

        {/* Connect/Disconnect button */}
        <div className="flex justify-end">
          {isConnected ? (
            <Button variant="outline" onClick={onDisconnect} disabled={isConnecting}>
              <WifiOff className="w-4 h-4" />
              {t('chat.disconnect')}
            </Button>
          ) : (
            <Button
              variant="primary"
              onClick={handleConnect}
              disabled={isConnecting || !channelId.trim() || !apiKey.trim()}
            >
              {isConnecting ? (
                <Loader2 className="w-4 h-4 animate-spin" />
              ) : (
                <Wifi className="w-4 h-4" />
              )}
              {t('chat.connect')}
            </Button>
          )}
        </div>
      </CardBody>
    </Card>
  );
}

/** Extract a short, user-friendly error message */
function formatError(error: unknown): string {
  const message = error instanceof Error ? error.message : String(error);

  // Channel not found on Twitch
  const twitchMatch = message.match(/Channel '([^']+)' does not exist on Twitch/i);
  if (twitchMatch) {
    return `Channel '${twitchMatch[1]}' not found`;
  }

  // Generic "does not exist" patterns
  if (message.toLowerCase().includes('does not exist')) {
    return 'Channel not found';
  }

  // Connection errors
  if (message.toLowerCase().includes('connection')) {
    return 'Connection failed';
  }

  // Return as-is if short enough, otherwise truncate
  return message.length > 50 ? message.slice(0, 47) + '...' : message;
}

export function ChatPanel() {
  const { t } = useTranslation();
  const [platformStatuses, setPlatformStatuses] = useState<ChatPlatformStatus[]>([]);
  const [connectingPlatform, setConnectingPlatform] = useState<ChatPlatform | null>(null);
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [autoConnect, setAutoConnect] = useState(false);
  const autoConnectAttempted = useRef(false);

  // Load settings and status on mount
  useEffect(() => {
    const loadInitialData = async () => {
      try {
        const [loadedSettings, statuses] = await Promise.all([
          api.settings.get(),
          api.chat.getStatus(),
        ]);
        setSettings(loadedSettings);
        setAutoConnect(loadedSettings.chatAutoConnect);
        setPlatformStatuses(statuses);
      } catch (error) {
        console.error('Failed to load chat settings:', error);
      }
    };

    loadInitialData();
  }, []);

  // Auto-connect on startup if enabled
  useEffect(() => {
    if (!settings || autoConnectAttempted.current) return;
    if (!settings.chatAutoConnect) return;

    autoConnectAttempted.current = true;

    const autoConnectPlatforms = async () => {
      // Auto-connect to Twitch if channel is configured
      if (settings.chatTwitchChannel) {
        try {
          const config: ChatConfig = {
            platform: 'twitch',
            enabled: true,
            credentials: {
              type: 'twitch',
              channel: settings.chatTwitchChannel,
              oauthToken: settings.chatTwitchOauthToken || undefined,
            },
          };
          await api.chat.connect(config);
        } catch (error) {
          console.error('Auto-connect to Twitch failed:', error);
        }
      }

      // Auto-connect to YouTube if channel and API key are configured
      if (settings.chatYoutubeChannelId && settings.chatYoutubeApiKey) {
        try {
          const config: ChatConfig = {
            platform: 'youtube',
            enabled: true,
            credentials: {
              type: 'youtube',
              channelId: settings.chatYoutubeChannelId,
              apiKey: settings.chatYoutubeApiKey,
            },
          };
          await api.chat.connect(config);
        } catch (error) {
          console.error('Auto-connect to YouTube failed:', error);
        }
      }

      // Refresh status after auto-connect attempts
      const statuses = await api.chat.getStatus();
      setPlatformStatuses(statuses);
    };

    autoConnectPlatforms();
  }, [settings]);

  // Poll for status updates
  useEffect(() => {
    const interval = setInterval(async () => {
      try {
        const statuses = await api.chat.getStatus();
        setPlatformStatuses(statuses);
      } catch (error) {
        console.error('Failed to load chat status:', error);
      }
    }, 5000);

    return () => clearInterval(interval);
  }, []);

  const getStatusForPlatform = (platform: ChatPlatform): ChatPlatformStatus | null => {
    return platformStatuses.find((s) => s.platform === platform) || null;
  };

  const saveSettings = useCallback(async (updates: Partial<AppSettings>) => {
    if (!settings) return;

    const newSettings = { ...settings, ...updates };
    setSettings(newSettings);

    try {
      await api.settings.save(newSettings);
    } catch (error) {
      console.error('Failed to save settings:', error);
    }
  }, [settings]);

  const handleTwitchCredentialsChange = useCallback((channel: string, oauthToken: string) => {
    saveSettings({
      chatTwitchChannel: channel,
      chatTwitchOauthToken: oauthToken,
    });
  }, [saveSettings]);

  const handleYouTubeCredentialsChange = useCallback((channelId: string, apiKey: string) => {
    saveSettings({
      chatYoutubeChannelId: channelId,
      chatYoutubeApiKey: apiKey,
    });
  }, [saveSettings]);

  const handleAutoConnectChange = useCallback((enabled: boolean) => {
    setAutoConnect(enabled);
    saveSettings({ chatAutoConnect: enabled });
  }, [saveSettings]);

  const handleConnect = useCallback(async (config: ChatConfig) => {
    setConnectingPlatform(config.platform);
    try {
      await api.chat.connect(config);
      const statuses = await api.chat.getStatus();
      setPlatformStatuses(statuses);
      toast.success(t('chat.connectSuccess', { platform: config.platform }));
    } catch (error) {
      toast.error(formatError(error));
    } finally {
      setConnectingPlatform(null);
    }
  }, [t]);

  const handleDisconnect = useCallback(async (platform: ChatPlatform) => {
    setConnectingPlatform(platform);
    try {
      await api.chat.disconnect(platform);
      const statuses = await api.chat.getStatus();
      setPlatformStatuses(statuses);
      toast.success(t('chat.disconnectSuccess', { platform }));
    } catch (error) {
      toast.error(formatError(error));
    } finally {
      setConnectingPlatform(null);
    }
  }, [t]);

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-3">
          <div className="p-2 rounded-lg bg-[var(--bg-elevated)]">
            <MessageSquare className="w-5 h-5 text-[var(--primary)]" />
          </div>
          <div>
            <h2 className="text-lg font-semibold text-[var(--text-primary)]">{t('chat.title')}</h2>
            <p className="text-sm text-[var(--text-secondary)]">{t('chat.description')}</p>
          </div>
        </div>
        {/* Auto-connect toggle */}
        <div className="flex items-center gap-2">
          <span className="text-sm text-[var(--text-secondary)]">{t('chat.autoConnect')}</span>
          <Toggle checked={autoConnect} onChange={handleAutoConnectChange} />
        </div>
      </div>

      {/* Platform Cards */}
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        <TwitchCard
          status={getStatusForPlatform('twitch')}
          isConnecting={connectingPlatform === 'twitch'}
          onConnect={handleConnect}
          onDisconnect={() => handleDisconnect('twitch')}
          initialChannel={settings?.chatTwitchChannel ?? ''}
          initialOauthToken={settings?.chatTwitchOauthToken ?? ''}
          onCredentialsChange={handleTwitchCredentialsChange}
        />
        <YouTubeCard
          status={getStatusForPlatform('youtube')}
          isConnecting={connectingPlatform === 'youtube'}
          onConnect={handleConnect}
          onDisconnect={() => handleDisconnect('youtube')}
          initialChannelId={settings?.chatYoutubeChannelId ?? ''}
          initialApiKey={settings?.chatYoutubeApiKey ?? ''}
          onCredentialsChange={handleYouTubeCredentialsChange}
        />
      </div>
    </div>
  );
}
