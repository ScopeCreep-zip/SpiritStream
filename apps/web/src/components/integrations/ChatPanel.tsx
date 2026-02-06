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
  ExternalLink,
  User,
  LogOut,
  Trash2,
} from 'lucide-react';
import { Card, CardHeader, CardTitle, CardDescription, CardBody } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { Input } from '@/components/ui/Input';
import { Toggle } from '@/components/ui/Toggle';
import { api } from '@/lib/backend';
import { toast } from '@/hooks/useToast';
import { cn } from '@/lib/cn';
import type { ChatPlatform, ChatPlatformStatus, ChatConfig, ChatCredentials, OAuthAccount } from '@/types/chat';
import type { AppSettings } from '@/types/api';

// ============================================================================
// Types
// ============================================================================

interface TwitchCardProps {
  status: ChatPlatformStatus | null;
  isConnecting: boolean;
  account: OAuthAccount | null;
  channel: string;
  autoConnect: boolean;
  onConnect: (config: ChatConfig) => Promise<void>;
  onDisconnect: () => Promise<void>;
  onLogin: () => Promise<void>;
  onLogout: () => Promise<void>;
  onForget: () => Promise<void>;
  onChannelChange: (channel: string) => void;
  onAutoConnectChange: (enabled: boolean) => void;
}

interface YouTubeCardProps {
  status: ChatPlatformStatus | null;
  isConnecting: boolean;
  account: OAuthAccount | null;
  channelId: string;
  apiKey: string;
  useApiKey: boolean;
  autoConnect: boolean;
  onConnect: (config: ChatConfig) => Promise<void>;
  onDisconnect: () => Promise<void>;
  onLogin: () => Promise<void>;
  onLogout: () => Promise<void>;
  onForget: () => Promise<void>;
  onChannelIdChange: (channelId: string) => void;
  onApiKeyChange: (apiKey: string) => void;
  onUseApiKeyChange: (useApiKey: boolean) => void;
  onAutoConnectChange: (enabled: boolean) => void;
}

// ============================================================================
// Twitch Card Component
// ============================================================================

function TwitchCard({
  status,
  isConnecting,
  account,
  channel,
  autoConnect,
  onConnect,
  onDisconnect,
  onLogin,
  onLogout,
  onForget,
  onChannelChange,
  onAutoConnectChange,
}: TwitchCardProps) {
  const { t } = useTranslation();
  const [isLoggingIn, setIsLoggingIn] = useState(false);
  const [isLoggingOut, setIsLoggingOut] = useState(false);

  const isConnected = status?.status === 'connected';
  const hasError = status?.status === 'error';
  const isLoggedIn = account?.loggedIn ?? false;

  const handleLogin = async () => {
    setIsLoggingIn(true);
    try {
      await onLogin();
    } catch (error) {
      toast.error(formatError(error));
    } finally {
      setIsLoggingIn(false);
    }
  };

  const handleLogout = async () => {
    setIsLoggingOut(true);
    try {
      await onLogout();
    } catch (error) {
      toast.error(formatError(error));
    } finally {
      setIsLoggingOut(false);
    }
  };

  const handleForget = async () => {
    setIsLoggingOut(true);
    try {
      await onForget();
      toast.success(t('chat.accountForgotten', 'Account forgotten'));
    } catch (error) {
      toast.error(formatError(error));
    } finally {
      setIsLoggingOut(false);
    }
  };

  const handleConnect = async () => {
    if (!channel.trim()) {
      toast.error(t('chat.twitch.enterChannel'));
      return;
    }

    const credentials: ChatCredentials = {
      type: 'twitch',
      channel: channel.trim(),
      // Use OAuth token if logged in, otherwise anonymous read-only
      auth: isLoggedIn
        ? { method: 'appOAuth', accessToken: '' } // Backend will use stored token
        : undefined,
    };

    const config: ChatConfig = {
      platform: 'twitch',
      enabled: true,
      credentials,
    };

    await onConnect(config);
  };

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
          {/* Connection status indicator */}
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
        {/* Account section */}
        <div className="p-3 rounded-lg bg-[var(--bg-elevated)] border border-[var(--border-subtle)]">
          <div className="flex items-center gap-3">
            <div className="p-2 rounded-full bg-[var(--bg-base)]">
              <User className="w-5 h-5 text-[var(--text-secondary)]" />
            </div>
            <div className="flex-1">
              {isLoggedIn ? (
                <>
                  <p className="font-medium text-[var(--text-primary)]">{account?.displayName}</p>
                  <p className="text-sm text-[var(--text-secondary)]">@{account?.username}</p>
                </>
              ) : (
                <p className="text-[var(--text-secondary)]">{t('chat.notLoggedIn', 'Not logged in')}</p>
              )}
            </div>
            {isLoggedIn ? (
              <div className="flex gap-2">
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={handleLogout}
                  disabled={isLoggingOut}
                  title={t('chat.disconnect', 'Disconnect')}
                >
                  <LogOut className="w-4 h-4" />
                </Button>
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={handleForget}
                  disabled={isLoggingOut}
                  title={t('chat.forgetAccount', 'Forget Account')}
                  className="text-[var(--status-error)] hover:text-[var(--status-error)]"
                >
                  <Trash2 className="w-4 h-4" />
                </Button>
              </div>
            ) : (
              <Button variant="outline" size="sm" onClick={handleLogin} disabled={isLoggingIn} className="gap-2">
                {isLoggingIn ? (
                  <Loader2 className="w-4 h-4 animate-spin" />
                ) : (
                  <ExternalLink className="w-4 h-4" />
                )}
                {t('chat.twitch.loginWithTwitch', 'Login with Twitch')}
              </Button>
            )}
          </div>
        </div>

        {/* Channel input */}
        <Input
          label={t('chat.twitch.channel')}
          value={channel}
          onChange={(e) => onChannelChange(e.target.value)}
          placeholder={isLoggedIn ? account?.username : t('chat.twitch.channelPlaceholder')}
          disabled={isConnected || isConnecting}
        />
        {!isLoggedIn && (
          <p className="text-xs text-[var(--text-tertiary)]">
            {t('chat.twitch.readOnlyHint', 'Read-only without login')}
          </p>
        )}

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

        {/* Auto-connect and Connect button */}
        <div className="flex items-center justify-between pt-2">
          <label className="flex items-center gap-2 cursor-pointer">
            <Toggle checked={autoConnect} onChange={onAutoConnectChange} />
            <span className="text-sm text-[var(--text-secondary)]">{t('chat.autoConnect')}</span>
          </label>
          {isConnected ? (
            <Button variant="outline" onClick={onDisconnect} disabled={isConnecting}>
              <WifiOff className="w-4 h-4" />
              {t('chat.disconnect')}
            </Button>
          ) : (
            <Button variant="primary" onClick={handleConnect} disabled={isConnecting || !channel.trim()}>
              {isConnecting ? <Loader2 className="w-4 h-4 animate-spin" /> : <Wifi className="w-4 h-4" />}
              {t('chat.connect')}
            </Button>
          )}
        </div>
      </CardBody>
    </Card>
  );
}

// ============================================================================
// YouTube Card Component
// ============================================================================

function YouTubeCard({
  status,
  isConnecting,
  account,
  channelId,
  apiKey,
  useApiKey,
  autoConnect,
  onConnect,
  onDisconnect,
  onLogin,
  onLogout,
  onForget,
  onChannelIdChange,
  onApiKeyChange,
  onUseApiKeyChange,
  onAutoConnectChange,
}: YouTubeCardProps) {
  const { t } = useTranslation();
  const [isLoggingIn, setIsLoggingIn] = useState(false);
  const [isLoggingOut, setIsLoggingOut] = useState(false);
  const [showApiKey, setShowApiKey] = useState(false);

  const isConnected = status?.status === 'connected';
  const hasError = status?.status === 'error';
  const isLoggedIn = account?.loggedIn ?? false;

  const handleLogin = async () => {
    setIsLoggingIn(true);
    try {
      await onLogin();
    } catch (error) {
      toast.error(formatError(error));
    } finally {
      setIsLoggingIn(false);
    }
  };

  const handleLogout = async () => {
    setIsLoggingOut(true);
    try {
      await onLogout();
    } catch (error) {
      toast.error(formatError(error));
    } finally {
      setIsLoggingOut(false);
    }
  };

  const handleForget = async () => {
    setIsLoggingOut(true);
    try {
      await onForget();
      toast.success(t('chat.accountForgotten', 'Account forgotten'));
    } catch (error) {
      toast.error(formatError(error));
    } finally {
      setIsLoggingOut(false);
    }
  };

  const handleConnect = async () => {
    if (!channelId.trim()) {
      toast.error(t('chat.youtube.enterChannelId'));
      return;
    }

    if (useApiKey && !apiKey.trim()) {
      toast.error(t('chat.youtube.enterApiKey'));
      return;
    }

    const credentials: ChatCredentials = {
      type: 'youtube',
      channelId: channelId.trim(),
      auth: useApiKey
        ? { method: 'apiKey', key: apiKey.trim() }
        : { method: 'appOAuth', accessToken: '' }, // Backend will use stored token
    };

    const config: ChatConfig = {
      platform: 'youtube',
      enabled: true,
      credentials,
    };

    await onConnect(config);
  };

  const canConnect = useApiKey ? channelId.trim() && apiKey.trim() : channelId.trim() && isLoggedIn;

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
          {/* Connection status indicator */}
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
        {/* Auth mode selector */}
        <div className="space-y-2">
          <label className="text-sm font-medium text-[var(--text-primary)]">
            {t('chat.youtube.authMethod', 'Authentication Method')}
          </label>
          <div className="space-y-2">
            <label className="flex items-center gap-3 p-3 rounded-lg border border-[var(--border-subtle)] cursor-pointer hover:bg-[var(--bg-elevated)] transition-colors">
              <input
                type="radio"
                name="youtube-auth"
                checked={!useApiKey}
                onChange={() => onUseApiKeyChange(false)}
                className="w-4 h-4 text-[var(--primary)]"
                disabled={isConnected}
              />
              <div className="flex-1">
                <p className="font-medium text-[var(--text-primary)]">
                  {t('chat.youtube.useOAuth', 'Sign in with Google')}
                </p>
                <p className="text-xs text-[var(--text-tertiary)]">
                  {t('chat.youtube.oauthHint', 'Uses shared app quota')}
                </p>
              </div>
            </label>
            <label className="flex items-center gap-3 p-3 rounded-lg border border-[var(--border-subtle)] cursor-pointer hover:bg-[var(--bg-elevated)] transition-colors">
              <input
                type="radio"
                name="youtube-auth"
                checked={useApiKey}
                onChange={() => onUseApiKeyChange(true)}
                className="w-4 h-4 text-[var(--primary)]"
                disabled={isConnected}
              />
              <div className="flex-1">
                <p className="font-medium text-[var(--text-primary)]">
                  {t('chat.youtube.useApiKey', 'Use my own API Key')}
                </p>
                <p className="text-xs text-[var(--text-tertiary)]">
                  {t('chat.youtube.apiKeyHint', 'Uses your quota - recommended for high usage')}
                </p>
              </div>
            </label>
          </div>
        </div>

        {/* OAuth account section (when OAuth mode is selected) */}
        {!useApiKey && (
          <div className="p-3 rounded-lg bg-[var(--bg-elevated)] border border-[var(--border-subtle)]">
            <div className="flex items-center gap-3">
              <div className="p-2 rounded-full bg-[var(--bg-base)]">
                <User className="w-5 h-5 text-[var(--text-secondary)]" />
              </div>
              <div className="flex-1">
                {isLoggedIn ? (
                  <>
                    <p className="font-medium text-[var(--text-primary)]">{account?.displayName}</p>
                    <p className="text-sm text-[var(--text-secondary)]">{account?.userId}</p>
                  </>
                ) : (
                  <p className="text-[var(--text-secondary)]">{t('chat.notLoggedIn', 'Not logged in')}</p>
                )}
              </div>
              {isLoggedIn ? (
                <div className="flex gap-2">
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={handleLogout}
                    disabled={isLoggingOut}
                    title={t('chat.disconnect', 'Disconnect')}
                  >
                    <LogOut className="w-4 h-4" />
                  </Button>
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={handleForget}
                    disabled={isLoggingOut}
                    title={t('chat.forgetAccount', 'Forget Account')}
                    className="text-[var(--status-error)] hover:text-[var(--status-error)]"
                  >
                    <Trash2 className="w-4 h-4" />
                  </Button>
                </div>
              ) : (
                <Button variant="outline" size="sm" onClick={handleLogin} disabled={isLoggingIn} className="gap-2">
                  {isLoggingIn ? (
                    <Loader2 className="w-4 h-4 animate-spin" />
                  ) : (
                    <ExternalLink className="w-4 h-4" />
                  )}
                  {t('chat.youtube.loginWithGoogle', 'Sign in with Google')}
                </Button>
              )}
            </div>
          </div>
        )}

        {/* Channel ID input */}
        <div>
          <Input
            label={t('chat.youtube.channelId')}
            value={channelId}
            onChange={(e) => onChannelIdChange(e.target.value)}
            placeholder={t('chat.youtube.channelIdPlaceholder')}
            disabled={isConnected || isConnecting}
          />
          <p className="text-xs text-[var(--text-tertiary)] mt-1">{t('chat.youtube.channelIdHint')}</p>
        </div>

        {/* API Key input (when API key mode is selected) */}
        {useApiKey && (
          <div className="relative">
            <Input
              label={t('chat.youtube.apiKey')}
              type={showApiKey ? 'text' : 'password'}
              value={apiKey}
              onChange={(e) => onApiKeyChange(e.target.value)}
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
          </div>
        )}

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

        {/* Auto-connect and Connect button */}
        <div className="flex items-center justify-between pt-2">
          <label className="flex items-center gap-2 cursor-pointer">
            <Toggle checked={autoConnect} onChange={onAutoConnectChange} />
            <span className="text-sm text-[var(--text-secondary)]">{t('chat.autoConnect')}</span>
          </label>
          {isConnected ? (
            <Button variant="outline" onClick={onDisconnect} disabled={isConnecting}>
              <WifiOff className="w-4 h-4" />
              {t('chat.disconnect')}
            </Button>
          ) : (
            <Button variant="primary" onClick={handleConnect} disabled={isConnecting || !canConnect}>
              {isConnecting ? <Loader2 className="w-4 h-4 animate-spin" /> : <Wifi className="w-4 h-4" />}
              {t('chat.connect')}
            </Button>
          )}
        </div>
      </CardBody>
    </Card>
  );
}

// ============================================================================
// Helper Functions
// ============================================================================

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

// ============================================================================
// Main Chat Panel Component
// ============================================================================

export function ChatPanel() {
  const { t } = useTranslation();
  const [platformStatuses, setPlatformStatuses] = useState<ChatPlatformStatus[]>([]);
  const [connectingPlatform, setConnectingPlatform] = useState<ChatPlatform | null>(null);
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [twitchAccount, setTwitchAccount] = useState<OAuthAccount | null>(null);
  const [youtubeAccount, setYoutubeAccount] = useState<OAuthAccount | null>(null);

  // Local state for form fields
  const [twitchChannel, setTwitchChannel] = useState('');
  const [youtubeChannelId, setYoutubeChannelId] = useState('');
  const [youtubeApiKey, setYoutubeApiKey] = useState('');
  const [youtubeUseApiKey, setYoutubeUseApiKey] = useState(false);
  const [autoConnect, setAutoConnect] = useState(false);

  // Pending OAuth flow state
  const pendingOAuthRef = useRef<{ provider: string; state: string } | null>(null);
  const autoConnectAttempted = useRef(false);

  // Load settings and OAuth accounts on mount
  useEffect(() => {
    const loadInitialData = async () => {
      try {
        const [loadedSettings, statuses, twitchAcc, youtubeAcc] = await Promise.all([
          api.settings.get(),
          api.chat.getStatus(),
          api.oauth.getAccount('twitch'),
          api.oauth.getAccount('youtube'),
        ]);

        setSettings(loadedSettings);
        setPlatformStatuses(statuses);
        setTwitchAccount(twitchAcc);
        setYoutubeAccount(youtubeAcc);

        // Initialize form state from settings
        setTwitchChannel(loadedSettings.chatTwitchChannel || '');
        setYoutubeChannelId(loadedSettings.chatYoutubeChannelId || '');
        setYoutubeApiKey(loadedSettings.chatYoutubeApiKey || '');
        setYoutubeUseApiKey(loadedSettings.youtubeUseApiKey || false);
        setAutoConnect(loadedSettings.chatAutoConnect || false);

        // Default channel to logged-in user if not set
        if (!loadedSettings.chatTwitchChannel && twitchAcc.loggedIn && twitchAcc.username) {
          setTwitchChannel(twitchAcc.username);
        }
        if (!loadedSettings.chatYoutubeChannelId && youtubeAcc.loggedIn && youtubeAcc.userId) {
          setYoutubeChannelId(youtubeAcc.userId);
        }
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
      if (twitchChannel) {
        try {
          const config: ChatConfig = {
            platform: 'twitch',
            enabled: true,
            credentials: {
              type: 'twitch',
              channel: twitchChannel,
              auth: twitchAccount?.loggedIn ? { method: 'appOAuth', accessToken: '' } : undefined,
            },
          };
          await api.chat.connect(config);
        } catch (error) {
          console.error('Auto-connect to Twitch failed:', error);
        }
      }

      // Auto-connect to YouTube if configured
      if (youtubeChannelId && (youtubeUseApiKey ? youtubeApiKey : youtubeAccount?.loggedIn)) {
        try {
          const config: ChatConfig = {
            platform: 'youtube',
            enabled: true,
            credentials: {
              type: 'youtube',
              channelId: youtubeChannelId,
              auth: youtubeUseApiKey
                ? { method: 'apiKey', key: youtubeApiKey }
                : { method: 'appOAuth', accessToken: '' },
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
  }, [settings, twitchChannel, twitchAccount, youtubeChannelId, youtubeApiKey, youtubeUseApiKey, youtubeAccount]);

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

  // Save settings helper
  const saveSettings = useCallback(
    async (updates: Partial<AppSettings>) => {
      if (!settings) return;

      const newSettings = { ...settings, ...updates };
      setSettings(newSettings);

      try {
        await api.settings.save(newSettings);
      } catch (error) {
        console.error('Failed to save settings:', error);
      }
    },
    [settings]
  );

  // Get status for a platform
  const getStatusForPlatform = (platform: ChatPlatform): ChatPlatformStatus | null => {
    return platformStatuses.find((s) => s.platform === platform) || null;
  };

  // ============================================================================
  // Twitch Handlers
  // ============================================================================

  const handleTwitchConnect = useCallback(
    async (config: ChatConfig) => {
      setConnectingPlatform('twitch');
      try {
        // Save channel to settings
        await saveSettings({ chatTwitchChannel: twitchChannel });

        await api.chat.connect(config);
        const statuses = await api.chat.getStatus();
        setPlatformStatuses(statuses);
        toast.success(t('chat.connectSuccess', { platform: 'Twitch' }));
      } catch (error) {
        toast.error(formatError(error));
      } finally {
        setConnectingPlatform(null);
      }
    },
    [twitchChannel, saveSettings, t]
  );

  const handleTwitchDisconnect = useCallback(async () => {
    setConnectingPlatform('twitch');
    try {
      await api.chat.disconnect('twitch');
      const statuses = await api.chat.getStatus();
      setPlatformStatuses(statuses);
      toast.success(t('chat.disconnectSuccess', { platform: 'Twitch' }));
    } catch (error) {
      toast.error(formatError(error));
    } finally {
      setConnectingPlatform(null);
    }
  }, [t]);

  const handleTwitchLogin = useCallback(async () => {
    const result = await api.oauth.startFlow('twitch');
    pendingOAuthRef.current = { provider: 'twitch', state: result.state };
    toast.info(t('chat.oauth.browserOpened', 'Check your browser to complete authentication'));

    // Poll for OAuth completion (backend stores tokens after callback)
    const pollInterval = setInterval(async () => {
      try {
        const account = await api.oauth.getAccount('twitch');
        if (account.loggedIn) {
          clearInterval(pollInterval);
          setTwitchAccount(account);
          // Default channel to logged-in user
          if (!twitchChannel) {
            setTwitchChannel(account.username || '');
          }
          toast.success(t('chat.oauth.loginSuccess', 'Logged in successfully'));
          pendingOAuthRef.current = null;
        }
      } catch {
        // Ignore polling errors
      }
    }, 2000);

    // Stop polling after 2 minutes
    setTimeout(() => {
      clearInterval(pollInterval);
      pendingOAuthRef.current = null;
    }, 120000);
  }, [twitchChannel, t]);

  const handleTwitchLogout = useCallback(async () => {
    await api.oauth.disconnect('twitch');
    setTwitchAccount({ loggedIn: false });
  }, []);

  const handleTwitchForget = useCallback(async () => {
    await api.oauth.forget('twitch');
    setTwitchAccount({ loggedIn: false });
  }, []);

  // ============================================================================
  // YouTube Handlers
  // ============================================================================

  const handleYoutubeConnect = useCallback(
    async (config: ChatConfig) => {
      setConnectingPlatform('youtube');
      try {
        // Save settings
        await saveSettings({
          chatYoutubeChannelId: youtubeChannelId,
          chatYoutubeApiKey: youtubeApiKey,
          youtubeUseApiKey: youtubeUseApiKey,
        });

        await api.chat.connect(config);
        const statuses = await api.chat.getStatus();
        setPlatformStatuses(statuses);
        toast.success(t('chat.connectSuccess', { platform: 'YouTube' }));
      } catch (error) {
        toast.error(formatError(error));
      } finally {
        setConnectingPlatform(null);
      }
    },
    [youtubeChannelId, youtubeApiKey, youtubeUseApiKey, saveSettings, t]
  );

  const handleYoutubeDisconnect = useCallback(async () => {
    setConnectingPlatform('youtube');
    try {
      await api.chat.disconnect('youtube');
      const statuses = await api.chat.getStatus();
      setPlatformStatuses(statuses);
      toast.success(t('chat.disconnectSuccess', { platform: 'YouTube' }));
    } catch (error) {
      toast.error(formatError(error));
    } finally {
      setConnectingPlatform(null);
    }
  }, [t]);

  const handleYoutubeLogin = useCallback(async () => {
    const result = await api.oauth.startFlow('youtube');
    pendingOAuthRef.current = { provider: 'youtube', state: result.state };
    toast.info(t('chat.oauth.browserOpened', 'Check your browser to complete authentication'));

    // Poll for OAuth completion
    const pollInterval = setInterval(async () => {
      try {
        const account = await api.oauth.getAccount('youtube');
        if (account.loggedIn) {
          clearInterval(pollInterval);
          setYoutubeAccount(account);
          // Default channel ID to logged-in user
          if (!youtubeChannelId) {
            setYoutubeChannelId(account.userId || '');
          }
          toast.success(t('chat.oauth.loginSuccess', 'Logged in successfully'));
          pendingOAuthRef.current = null;
        }
      } catch {
        // Ignore polling errors
      }
    }, 2000);

    // Stop polling after 2 minutes
    setTimeout(() => {
      clearInterval(pollInterval);
      pendingOAuthRef.current = null;
    }, 120000);
  }, [youtubeChannelId, t]);

  const handleYoutubeLogout = useCallback(async () => {
    await api.oauth.disconnect('youtube');
    setYoutubeAccount({ loggedIn: false });
  }, []);

  const handleYoutubeForget = useCallback(async () => {
    await api.oauth.forget('youtube');
    setYoutubeAccount({ loggedIn: false });
  }, []);

  const handleAutoConnectChange = useCallback(
    (enabled: boolean) => {
      setAutoConnect(enabled);
      saveSettings({ chatAutoConnect: enabled });
    },
    [saveSettings]
  );

  const handleYoutubeUseApiKeyChange = useCallback(
    (useKey: boolean) => {
      setYoutubeUseApiKey(useKey);
      saveSettings({ youtubeUseApiKey: useKey });
    },
    [saveSettings]
  );

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex items-center gap-3">
        <div className="p-2 rounded-lg bg-[var(--bg-elevated)]">
          <MessageSquare className="w-5 h-5 text-[var(--primary)]" />
        </div>
        <div>
          <h2 className="text-lg font-semibold text-[var(--text-primary)]">{t('chat.title')}</h2>
          <p className="text-sm text-[var(--text-secondary)]">{t('chat.description')}</p>
        </div>
      </div>

      {/* Platform Cards */}
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        <TwitchCard
          status={getStatusForPlatform('twitch')}
          isConnecting={connectingPlatform === 'twitch'}
          account={twitchAccount}
          channel={twitchChannel}
          autoConnect={autoConnect}
          onConnect={handleTwitchConnect}
          onDisconnect={handleTwitchDisconnect}
          onLogin={handleTwitchLogin}
          onLogout={handleTwitchLogout}
          onForget={handleTwitchForget}
          onChannelChange={setTwitchChannel}
          onAutoConnectChange={handleAutoConnectChange}
        />
        <YouTubeCard
          status={getStatusForPlatform('youtube')}
          isConnecting={connectingPlatform === 'youtube'}
          account={youtubeAccount}
          channelId={youtubeChannelId}
          apiKey={youtubeApiKey}
          useApiKey={youtubeUseApiKey}
          autoConnect={autoConnect}
          onConnect={handleYoutubeConnect}
          onDisconnect={handleYoutubeDisconnect}
          onLogin={handleYoutubeLogin}
          onLogout={handleYoutubeLogout}
          onForget={handleYoutubeForget}
          onChannelIdChange={setYoutubeChannelId}
          onApiKeyChange={setYoutubeApiKey}
          onUseApiKeyChange={handleYoutubeUseApiKeyChange}
          onAutoConnectChange={handleAutoConnectChange}
        />
      </div>
    </div>
  );
}
