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
  RefreshCw,
} from 'lucide-react';
import { Card, CardHeader, CardTitle, CardDescription, CardBody } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { Input } from '@/components/ui/Input';
import { Toggle } from '@/components/ui/Toggle';
import { api, events } from '@/lib/backend';
import { toast } from '@/hooks/useToast';
import { cn } from '@/lib/cn';
import type { ChatPlatform, ChatPlatformStatus, OAuthAccount } from '@/types/chat';
import type { AppSettings } from '@/types/api';

// ============================================================================
// Types
// ============================================================================

interface TwitchCardProps {
  status: ChatPlatformStatus | null;
  account: OAuthAccount | null;
  channel: string;
  sendEnabled: boolean;
  onSendEnabledChange: (enabled: boolean) => void;
  onRetry: () => Promise<void>;
  onClear: () => void;
  onLogin: () => Promise<void>;
  onLogout: () => Promise<void>;
  onForget: () => Promise<void>;
  onChannelChange: (channel: string) => void;
  onChannelSave: () => void;
}

interface YouTubeCardProps {
  status: ChatPlatformStatus | null;
  account: OAuthAccount | null;
  channelId: string;
  apiKey: string;
  useApiKey: boolean;
  sendEnabled: boolean;
  onSendEnabledChange: (enabled: boolean) => void;
  onRetry: () => Promise<void>;
  onClear: () => void;
  onLogin: () => Promise<void>;
  onLogout: () => Promise<void>;
  onForget: () => Promise<void>;
  onChannelIdChange: (channelId: string) => void;
  onChannelIdSave: () => void;
  onUseMyChannel: () => void;
  onApiKeyChange: (apiKey: string) => void;
  onApiKeySave: () => void;
  onUseApiKeyChange: (useApiKey: boolean) => void;
}

// ============================================================================
// Twitch Card Component
// ============================================================================

function TwitchCard({
  status,
  account,
  channel,
  sendEnabled,
  onSendEnabledChange,
  onRetry,
  onClear,
  onLogin,
  onLogout,
  onForget,
  onChannelChange,
  onChannelSave,
}: TwitchCardProps) {
  const { t } = useTranslation();
  const [isLoggingIn, setIsLoggingIn] = useState(false);
  const [isLoggingOut, setIsLoggingOut] = useState(false);

  const isConnected = status?.status === 'connected';
  const isConnecting = status?.status === 'connecting';
  const hasError = status?.status === 'error';
  const isLoggedIn = account?.loggedIn ?? false;
  const isConfigured = !!channel.trim();

  const statusLabel = isConnecting
    ? t('chat.connecting')
    : isConnected
      ? t('chat.connected', 'Chat connected')
      : hasError
        ? t('chat.error')
        : isConfigured
          ? t('chat.waitingForStream', 'Waiting for stream')
          : t('chat.notConfigured', 'Not configured');

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
              {statusLabel}
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
            <div className="flex gap-2">
              {isLoggedIn ? (
                <>
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={handleLogout}
                    disabled={isLoggingOut}
                    title={t('chat.signOut', 'Sign out')}
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
                </>
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
        </div>

        {/* Channel input */}
        <Input
          label={t('chat.twitch.channel')}
          value={channel}
          onChange={(e) => onChannelChange(e.target.value)}
          onBlur={onChannelSave}
          placeholder={isLoggedIn ? account?.username : t('chat.twitch.channelPlaceholder')}
          disabled={isConnected || isConnecting}
        />
        {!isLoggedIn && (
          <p className="text-xs text-[var(--text-tertiary)]">
            {t('chat.twitch.readOnlyHint', 'Read-only without login')}
          </p>
        )}
        <Toggle
          checked={sendEnabled}
          onChange={onSendEnabledChange}
          disabled={!isLoggedIn}
          label={t('chat.sendEnabled', 'Allow sending messages')}
          description={t('chat.sendEnabledHint', 'Send messages from SpiritStream')}
          className="pt-1"
        />
        {!isLoggedIn && (
          <p className="text-xs text-[var(--text-tertiary)]">
            {t('chat.sendRequiresLogin', 'Sign in to send messages')}
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

        {/* Stream-tied lifecycle hint + retry/clear */}
        <div className="flex items-center justify-between pt-2">
          <p className="text-xs text-[var(--text-tertiary)]">
            {t(
              'chat.streamTiedHint',
              'Chat connects when you start streaming. YouTube may take ~10s to activate.'
            )}
          </p>
          <div className="flex items-center gap-2">
            <Button
              variant="ghost"
              size="sm"
              onClick={onRetry}
              disabled={!isConfigured || isConnecting}
            >
              <RefreshCw className="w-4 h-4" />
              {t('chat.retry', 'Retry')}
            </Button>
            <Button
              variant="ghost"
              size="sm"
              onClick={onClear}
              disabled={!isConfigured || isConnecting}
            >
              <Trash2 className="w-4 h-4" />
              {t('chat.clear', 'Clear')}
            </Button>
          </div>
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
  account,
  channelId,
  apiKey,
  useApiKey,
  sendEnabled,
  onSendEnabledChange,
  onRetry,
  onClear,
  onLogin,
  onLogout,
  onForget,
  onChannelIdChange,
  onChannelIdSave,
  onUseMyChannel,
  onApiKeyChange,
  onApiKeySave,
  onUseApiKeyChange,
}: YouTubeCardProps) {
  const { t } = useTranslation();
  const [isLoggingIn, setIsLoggingIn] = useState(false);
  const [isLoggingOut, setIsLoggingOut] = useState(false);
  const [showApiKey, setShowApiKey] = useState(false);
  const [showAdvanced, setShowAdvanced] = useState(useApiKey);
  useEffect(() => {
    if (useApiKey) {
      setShowAdvanced(true);
    }
  }, [useApiKey]);

  const isConnected = status?.status === 'connected';
  const isConnecting = status?.status === 'connecting';
  const hasError = status?.status === 'error';
  const isLoggedIn = account?.loggedIn ?? false;
  const isConfigured = !!channelId.trim();

  const statusLabel = isConnecting
    ? t('chat.connecting')
    : isConnected
      ? t('chat.connected', 'Chat connected')
      : hasError
        ? t('chat.error')
        : isConfigured
          ? t('chat.waitingForStream', 'Waiting for stream')
          : t('chat.notConfigured', 'Not configured');

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

  const canSend = !useApiKey && isLoggedIn;

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
              {statusLabel}
            </span>
          </div>
        </div>
      </CardHeader>
      <CardBody className="space-y-4">
        {/* Auth mode selector */}
        <div className="space-y-2">
          <div className="flex items-center justify-between">
            <label className="text-sm font-medium text-[var(--text-primary)]">
              {t('chat.youtube.authMethod', 'Authentication Method')}
            </label>
            <Button
              variant="ghost"
              size="sm"
              onClick={() => setShowAdvanced(!showAdvanced)}
              disabled={isConnected}
            >
              {showAdvanced ? t('chat.advancedHide', 'Hide advanced') : t('chat.advancedShow', 'Advanced')}
            </Button>
          </div>
          <div className="p-3 rounded-lg border border-[var(--border-subtle)] bg-[var(--bg-elevated)]">
            <p className="font-medium text-[var(--text-primary)]">
              {t('chat.youtube.useOAuth', 'Sign in with Google')}
            </p>
            <p className="text-xs text-[var(--text-tertiary)]">
              {t('chat.youtube.oauthHint', 'Uses shared app quota')}
            </p>
          </div>
          {showAdvanced && (
            <Toggle
              checked={useApiKey}
              onChange={onUseApiKeyChange}
              disabled={isConnected}
              label={t('chat.youtube.useApiKey', 'Use my own API Key')}
              description={t('chat.youtube.apiKeyHint', 'Uses your quota - recommended for high usage')}
            />
          )}
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
                    title={t('chat.signOut', 'Sign out')}
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
            {!isLoggedIn && (
              <p className="mt-2 text-xs text-[var(--text-tertiary)]">
                {t('chat.youtube.loginRequired', 'Sign in is required to read YouTube chat')}
              </p>
            )}
          </div>
        )}

        {/* Channel ID input */}
        <div>
          <Input
            label={t('chat.youtube.channelId')}
            value={channelId}
            onChange={(e) => onChannelIdChange(e.target.value)}
            onBlur={onChannelIdSave}
            placeholder={t('chat.youtube.channelIdPlaceholder')}
            disabled={isConnected || isConnecting}
          />
          <div className="flex items-center justify-between mt-1">
            <p className="text-xs text-[var(--text-tertiary)]">
              {t('chat.youtube.channelIdHint', 'Use your YouTube channel ID')}
            </p>
            {!useApiKey && isLoggedIn && account?.userId && (
              <Button variant="ghost" size="sm" onClick={onUseMyChannel} disabled={isConnected || isConnecting}>
                {t('chat.youtube.useMyChannel', 'Use my channel')}
              </Button>
            )}
          </div>
        </div>

        {/* API Key input (when API key mode is selected) */}
        {useApiKey && (
          <div className="relative">
            <Input
              label={t('chat.youtube.apiKey')}
              type={showApiKey ? 'text' : 'password'}
              value={apiKey}
              onChange={(e) => onApiKeyChange(e.target.value)}
              onBlur={onApiKeySave}
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
        <Toggle
          checked={sendEnabled}
          onChange={onSendEnabledChange}
          disabled={!canSend}
          label={t('chat.sendEnabled', 'Allow sending messages')}
          description={t('chat.sendEnabledHint', 'Send messages from SpiritStream')}
        />
        {!canSend && (
          <p className="text-xs text-[var(--text-tertiary)]">
            {useApiKey
              ? t('chat.youtube.apiKeyReadOnly', 'API keys are read-only')
              : t('chat.sendRequiresLogin', 'Sign in to send messages')}
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

        {/* Stream-tied lifecycle hint + retry/clear */}
        <div className="flex items-center justify-between pt-2">
          <p className="text-xs text-[var(--text-tertiary)]">
            {t(
              'chat.streamTiedHint',
              'Chat connects when you start streaming. YouTube may take ~10s to activate.'
            )}
          </p>
          <div className="flex items-center gap-2">
            <Button
              variant="ghost"
              size="sm"
              onClick={onRetry}
              disabled={!isConfigured || isConnecting}
            >
              <RefreshCw className="w-4 h-4" />
              {t('chat.retry', 'Retry')}
            </Button>
            <Button
              variant="ghost"
              size="sm"
              onClick={onClear}
              disabled={!isConfigured || isConnecting}
            >
              <Trash2 className="w-4 h-4" />
              {t('chat.clear', 'Clear')}
            </Button>
          </div>
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
  const [, setSettings] = useState<AppSettings | null>(null);
  const [twitchAccount, setTwitchAccount] = useState<OAuthAccount | null>(null);
  const [youtubeAccount, setYoutubeAccount] = useState<OAuthAccount | null>(null);

  // Local state for form fields
  const [twitchChannel, setTwitchChannel] = useState('');
  const [youtubeChannelId, setYoutubeChannelId] = useState('');
  const [youtubeApiKey, setYoutubeApiKey] = useState('');
  const [youtubeUseApiKey, setYoutubeUseApiKey] = useState(false);
  const [twitchSendEnabled, setTwitchSendEnabled] = useState(false);
  const [youtubeSendEnabled, setYoutubeSendEnabled] = useState(false);
  const [crosspostEnabled, setCrosspostEnabled] = useState(false);

  // Pending OAuth flow state
  const pendingOAuthRef = useRef<{ provider: string; state: string } | null>(null);

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
        setTwitchSendEnabled(loadedSettings.chatTwitchSendEnabled || false);
        setYoutubeSendEnabled(loadedSettings.chatYoutubeSendEnabled || false);
        setCrosspostEnabled(loadedSettings.chatCrosspostEnabled || false);

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

  // Save settings helper -- always loads fresh settings from backend first
  // to avoid overwriting OAuth tokens or other values saved by other flows
  const saveSettings = useCallback(
    async (updates: Partial<AppSettings>) => {
      try {
        const freshSettings = await api.settings.get();
        const newSettings = { ...freshSettings, ...updates };
        setSettings(newSettings);
        await api.settings.save(newSettings);
      } catch (error) {
        console.error('Failed to save settings:', error);
      }
    },
    []
  );

  // Chat auto-connect/disconnect is handled by the backend on stream start/stop.
  // Listen for backend events to update UI status.

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

  // Listen for oauth_complete events -- update account state (chat connects on stream start)
  useEffect(() => {
    let unlistenOAuth: (() => void) | null = null;
    let unlistenAutoConnect: (() => void) | null = null;
    let unlistenAutoDisconnect: (() => void) | null = null;
    let unlistenConnectionLost: (() => void) | null = null;
    let unlistenConnectionRestored: (() => void) | null = null;

    const setup = async () => {
      // OAuth login completed -- update account state
      unlistenOAuth = await events.on<{ provider: string; userId: string; username: string; displayName: string }>(
        'oauth_complete',
        async (payload) => {
          const { provider, userId, username, displayName } = payload;

          if (provider === 'twitch') {
            setTwitchAccount({ loggedIn: true, userId, username, displayName });
            if (!twitchChannel && username) {
              setTwitchChannel(username);
              saveSettings({ chatTwitchChannel: username });
            }
            toast.success(t('chat.oauth.loginSuccess', 'Signed in to Twitch'));
          } else if (provider === 'youtube') {
            setYoutubeAccount({ loggedIn: true, userId, username, displayName });
            if (!youtubeChannelId && userId) {
              setYoutubeChannelId(userId);
              saveSettings({ chatYoutubeChannelId: userId });
            }
            toast.success(t('chat.oauth.loginSuccess', 'Signed in to YouTube'));
          }
        }
      );

      // Chat auto-connected by backend on stream start
      unlistenAutoConnect = await events.on<{ platform: string }>(
        'chat_auto_connected',
        async (payload) => {
          const statuses = await api.chat.getStatus();
          setPlatformStatuses(statuses);
          const name = payload.platform === 'twitch' ? 'Twitch' : 'YouTube';
          toast.success(t('chat.autoConnected', { platform: name, defaultValue: '{{platform}} chat connected' }));
        }
      );

      // Chat auto-disconnected by backend on stream stop
      unlistenAutoDisconnect = await events.on(
        'chat_auto_disconnected',
        async () => {
          const statuses = await api.chat.getStatus();
          setPlatformStatuses(statuses);
        }
      );

      unlistenConnectionLost = await events.on<{ platform: string; error: string }>(
        'chat_connection_lost',
        async (payload) => {
          const statuses = await api.chat.getStatus();
          setPlatformStatuses(statuses);
          toast.error(
            t('chat.connectionLost', {
              platform: payload.platform,
              defaultValue: '{{platform}} chat connection lost',
            }) + (payload.error ? `: ${payload.error}` : '')
          );
        }
      );

      unlistenConnectionRestored = await events.on<{ platform: string }>(
        'chat_connection_restored',
        async (payload) => {
          const statuses = await api.chat.getStatus();
          setPlatformStatuses(statuses);
          toast.success(
            t('chat.connectionRestored', {
              platform: payload.platform,
              defaultValue: '{{platform}} chat reconnected',
            })
          );
        }
      );
    };

    setup();

    return () => {
      if (unlistenOAuth) unlistenOAuth();
      if (unlistenAutoConnect) unlistenAutoConnect();
      if (unlistenAutoDisconnect) unlistenAutoDisconnect();
      if (unlistenConnectionLost) unlistenConnectionLost();
      if (unlistenConnectionRestored) unlistenConnectionRestored();
    };
  }, [twitchChannel, youtubeChannelId, t, saveSettings]);

  // Get status for a platform
  const getStatusForPlatform = (platform: ChatPlatform): ChatPlatformStatus | null => {
    return platformStatuses.find((s) => s.platform === platform) || null;
  };

  // ============================================================================
  // Twitch Handlers
  // ============================================================================

  const handleTwitchRetry = useCallback(async () => {
    try {
      await api.chat.retryConnection('twitch');
      const statuses = await api.chat.getStatus();
      setPlatformStatuses(statuses);
      toast.success(t('chat.retrySuccess', { platform: 'Twitch', defaultValue: 'Retrying Twitch chat' }));
    } catch (error) {
      toast.error(formatError(error));
    }
  }, [t]);

  const handleTwitchLogin = useCallback(async () => {
    const result = await api.oauth.startFlow('twitch');
    pendingOAuthRef.current = { provider: 'twitch', state: result.state };
    toast.info(t('chat.oauth.browserOpened', 'Check your browser to complete authentication'));
  }, [twitchChannel, t]);

  const handleTwitchLogout = useCallback(async () => {
    await api.oauth.disconnect('twitch');
    setTwitchAccount({ loggedIn: false });
    setTwitchSendEnabled(false);
    saveSettings({ chatTwitchSendEnabled: false });
  }, [saveSettings]);

  const handleTwitchForget = useCallback(async () => {
    await api.oauth.forget('twitch');
    setTwitchAccount({ loggedIn: false });
    setTwitchChannel('');
    setTwitchSendEnabled(false);
    // Reload settings from backend (forget clears channel + tokens server-side)
    const freshSettings = await api.settings.get();
    setSettings(freshSettings);
  }, []);

  // Save channel name to settings on blur so clearing it actually persists
  const handleTwitchChannelSave = useCallback(() => {
    saveSettings({ chatTwitchChannel: twitchChannel });
  }, [twitchChannel, saveSettings]);

  const handleTwitchClear = useCallback(() => {
    setTwitchChannel('');
    saveSettings({ chatTwitchChannel: '' });
  }, [saveSettings]);

  const handleTwitchSendEnabledChange = useCallback(
    (enabled: boolean) => {
      setTwitchSendEnabled(enabled);
      saveSettings({ chatTwitchSendEnabled: enabled });
    },
    [saveSettings]
  );

  // ============================================================================
  // YouTube Handlers
  // ============================================================================

  const handleYoutubeRetry = useCallback(async () => {
    try {
      await api.chat.retryConnection('youtube');
      const statuses = await api.chat.getStatus();
      setPlatformStatuses(statuses);
      toast.success(t('chat.retrySuccess', { platform: 'YouTube', defaultValue: 'Retrying YouTube chat' }));
    } catch (error) {
      toast.error(formatError(error));
    }
  }, [t]);

  const handleYoutubeLogin = useCallback(async () => {
    const result = await api.oauth.startFlow('youtube');
    pendingOAuthRef.current = { provider: 'youtube', state: result.state };
    toast.info(t('chat.oauth.browserOpened', 'Check your browser to complete authentication'));
  }, [youtubeChannelId, t]);

  const handleYoutubeLogout = useCallback(async () => {
    await api.oauth.disconnect('youtube');
    setYoutubeAccount({ loggedIn: false });
    setYoutubeSendEnabled(false);
    saveSettings({ chatYoutubeSendEnabled: false });
  }, [saveSettings]);

  const handleYoutubeForget = useCallback(async () => {
    await api.oauth.forget('youtube');
    setYoutubeAccount({ loggedIn: false });
    setYoutubeChannelId('');
    setYoutubeSendEnabled(false);
    // Reload settings from backend (forget clears channel + tokens server-side)
    const freshSettings = await api.settings.get();
    setSettings(freshSettings);
  }, []);

  // Save channel ID to settings on blur so clearing it actually persists
  const handleYoutubeChannelIdSave = useCallback(() => {
    saveSettings({ chatYoutubeChannelId: youtubeChannelId });
  }, [youtubeChannelId, saveSettings]);

  const handleYoutubeApiKeySave = useCallback(() => {
    saveSettings({ chatYoutubeApiKey: youtubeApiKey });
  }, [youtubeApiKey, saveSettings]);

  const handleYoutubeUseApiKeyChange = useCallback(
    (useKey: boolean) => {
      setYoutubeUseApiKey(useKey);
      if (useKey) {
        setYoutubeSendEnabled(false);
        saveSettings({ youtubeUseApiKey: useKey, chatYoutubeSendEnabled: false });
      } else {
        saveSettings({ youtubeUseApiKey: useKey });
      }
    },
    [saveSettings]
  );

  const handleYoutubeClear = useCallback(() => {
    setYoutubeChannelId('');
    saveSettings({ chatYoutubeChannelId: '' });
  }, [saveSettings]);

  const handleYoutubeSendEnabledChange = useCallback(
    (enabled: boolean) => {
      setYoutubeSendEnabled(enabled);
      saveSettings({ chatYoutubeSendEnabled: enabled });
    },
    [saveSettings]
  );

  const handleYoutubeUseMyChannel = useCallback(() => {
    if (youtubeAccount?.userId) {
      setYoutubeChannelId(youtubeAccount.userId);
      saveSettings({ chatYoutubeChannelId: youtubeAccount.userId });
    }
  }, [youtubeAccount, saveSettings]);

  const handleCrosspostChange = useCallback(
    (enabled: boolean) => {
      setCrosspostEnabled(enabled);
      saveSettings({ chatCrosspostEnabled: enabled });
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

      <div className="rounded-xl border border-[var(--border-subtle)] bg-[var(--bg-elevated)] p-4">
        <Toggle
          checked={crosspostEnabled}
          onChange={handleCrosspostChange}
          label={t('chat.crosspostEnabled', 'Crosspost chat messages')}
          description={t(
            'chat.crosspostHint',
            'Relay incoming messages to all enabled platforms that allow sending.'
          )}
        />
      </div>

      {/* Platform Cards */}
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        <TwitchCard
          status={getStatusForPlatform('twitch')}
          account={twitchAccount}
          channel={twitchChannel}
          sendEnabled={twitchSendEnabled}
          onSendEnabledChange={handleTwitchSendEnabledChange}
          onRetry={handleTwitchRetry}
          onClear={handleTwitchClear}
          onLogin={handleTwitchLogin}
          onLogout={handleTwitchLogout}
          onForget={handleTwitchForget}
          onChannelChange={setTwitchChannel}
          onChannelSave={handleTwitchChannelSave}
        />
        <YouTubeCard
          status={getStatusForPlatform('youtube')}
          account={youtubeAccount}
          channelId={youtubeChannelId}
          apiKey={youtubeApiKey}
          useApiKey={youtubeUseApiKey}
          sendEnabled={youtubeSendEnabled}
          onSendEnabledChange={handleYoutubeSendEnabledChange}
          onRetry={handleYoutubeRetry}
          onClear={handleYoutubeClear}
          onLogin={handleYoutubeLogin}
          onLogout={handleYoutubeLogout}
          onForget={handleYoutubeForget}
          onChannelIdChange={setYoutubeChannelId}
          onChannelIdSave={handleYoutubeChannelIdSave}
          onUseMyChannel={handleYoutubeUseMyChannel}
          onApiKeyChange={setYoutubeApiKey}
          onApiKeySave={handleYoutubeApiKeySave}
          onUseApiKeyChange={handleYoutubeUseApiKeyChange}
        />
      </div>
    </div>
  );
}
