import { useEffect, useState, useCallback, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { ChevronDown, MessageSquare } from 'lucide-react';
import { useProfileStore } from '@/stores/profileStore';
import { api } from '@/lib/client';
import type { OAuthConfiguredFlags } from '@spiritstream/api-client';
import { toast } from '@/hooks/useToast';
import { logger } from '@/lib/logger';
import { createDefaultChatSettings } from '@/lib/profile-helpers';
import { Button } from '@/components/ui/Button';
import { Card, CardHeader, CardTitle, CardDescription, CardBody } from '@/components/ui/Card';
import { Input } from '@/components/ui/Input';
import { Toggle } from '@/components/ui/Toggle';
import { FacebookConnectGate } from '@/components/chat/settings/FacebookConnectGate';
import { PlatformSignInButton } from '@/components/chat/settings/PlatformSignInButton';
import { cn } from '@/lib/cn';

type ChatField =
  | 'twitch_channel'
  | 'youtube_channel'
  | 'trovo_channel'
  | 'kick_channel'
  | 'tiktok_username'
  | 'facebook_video_id';

/// Stable IDs for the visibility-panel toggles. Match the platform
/// discriminators on the backend `ChatPlatform` enum so the values
/// round-trip through `chatSettings.visiblePlatforms` unchanged.
type VisiblePlatform = 'twitch' | 'youtube' | 'trovo' | 'kick' | 'tiktok' | 'facebook';

const ALL_VISIBLE_PLATFORMS: readonly VisiblePlatform[] = [
  'twitch',
  'youtube',
  'trovo',
  'kick',
  'tiktok',
  'facebook',
];

/**
 * Chat-platform settings panel. Exposes per-platform channel /
 * username inputs plus send-enable toggles. Reads + writes through
 * the active profile via `useProfileStore.updateProfileSettings` so
 * every field round-trips through the backend's save+activate path
 * (which also triggers auto-connect for the changed platform).
 *
 * Facebook is the odd one out: connecting reveals the streamer's
 * real-name identity per Meta's Name Policy, so the enable flow is
 * delegated to {@link FacebookConnectGate} which handles the
 * confirm-token + identity-warning UX before persisting the video id.
 */
export function ChatPanel(): React.ReactElement {
  const { t } = useTranslation();
  const currentProfile = useProfileStore((state) => state.current);
  const updateProfileSettings = useProfileStore((state) => state.updateProfileSettings);

  // Backend-truth per-provider configured flags (placeholder client
  // credentials report false). Until they load — or if the fetch fails —
  // buttons stay in the honest "not set up" state rather than launching
  // a flow the backend would refuse.
  const [oauthFlags, setOauthFlags] = useState<OAuthConfiguredFlags | null>(null);
  useEffect(() => {
    let cancelled = false;
    api.oauth
      .getConfig()
      .then((flags) => {
        if (!cancelled) setOauthFlags(flags);
      })
      .catch((error) => {
        logger.error('[ChatPanel] failed to load oauth config flags:', error);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const chatSettings = useMemo(
    () => currentProfile?.settings?.chat ?? createDefaultChatSettings(),
    [currentProfile]
  );

  const [twitchChannel, setTwitchChannel] = useState('');
  const [twitchSend, setTwitchSend] = useState(false);
  const [youtubeChannelId, setYoutubeChannelId] = useState('');
  const [youtubeApiKey, setYoutubeApiKey] = useState('');
  const [youtubeUseApiKey, setYoutubeUseApiKey] = useState(false);
  const [youtubeSend, setYoutubeSend] = useState(false);
  const [trovoChannelId, setTrovoChannelId] = useState('');
  const [trovoSend, setTrovoSend] = useState(false);
  const [kickChannel, setKickChannel] = useState('');
  const [kickSend, setKickSend] = useState(false);
  const [tiktokUsername, setTiktokUsername] = useState('');
  // Visibility-panel state (custom selection + collapse). Empty
  // `visiblePlatforms` array means "auto" — the resolved set is then
  // derived from which channels are actually configured.
  const [visiblePlatforms, setVisiblePlatforms] = useState<VisiblePlatform[]>([]);
  const [visibilityPanelCollapsed, setVisibilityPanelCollapsed] = useState(true);
  // Master "broadcast to all enabled platforms" toggle. When ON, the
  // ChatComposer sends to every connected platform whose per-platform
  // `*_send_enabled` flag is true (current default behaviour). When
  // OFF, the composer prompts the user to pick a single target per
  // message — that affordance lives in ChatComposer, gated on this
  // setting.
  const [sendAllEnabled, setSendAllEnabled] = useState(true);

  useEffect(() => {
    setTwitchChannel(chatSettings.twitchChannel);
    setTwitchSend(chatSettings.twitchSendEnabled);
    setYoutubeChannelId(chatSettings.youtubeChannelId);
    setYoutubeApiKey(chatSettings.youtubeApiKey);
    setYoutubeUseApiKey(chatSettings.youtubeUseApiKey);
    setYoutubeSend(chatSettings.youtubeSendEnabled);
    setTrovoChannelId(chatSettings.trovoChannelId);
    setTrovoSend(chatSettings.trovoSendEnabled);
    setKickChannel(chatSettings.kickChannel);
    setKickSend(chatSettings.kickSendEnabled);
    setTiktokUsername(chatSettings.tiktokUsername);
    setVisiblePlatforms(
      (chatSettings.visiblePlatforms ?? []).filter((p): p is VisiblePlatform =>
        (ALL_VISIBLE_PLATFORMS as readonly string[]).includes(p)
      )
    );
    setVisibilityPanelCollapsed(chatSettings.visibilityPanelCollapsed ?? true);
    setSendAllEnabled(chatSettings.sendAllEnabled);
  }, [chatSettings]);

  const persist = useCallback(
    async (patch: Partial<typeof chatSettings>): Promise<void> => {
      try {
        await updateProfileSettings({ chat: { ...chatSettings, ...patch } });
      } catch (error) {
        logger.error('[ChatPanel] save failed:', error);
        toast.error(t('chat.saveFailed', { defaultValue: 'Failed to save chat settings' }));
      }
    },
    [chatSettings, updateProfileSettings, t]
  );

  /// In "auto" mode, surface every platform that the user has actually
  /// configured (channel/username/video-id non-empty). Falls back to
  /// Twitch + YouTube on a freshly-activated profile so the cards
  /// aren't blank.
  const autoVisiblePlatforms = useMemo<VisiblePlatform[]>(() => {
    const auto: VisiblePlatform[] = [];
    if (chatSettings.twitchChannel.trim()) auto.push('twitch');
    if (chatSettings.youtubeChannelId.trim()) auto.push('youtube');
    if (chatSettings.trovoChannelId.trim()) auto.push('trovo');
    if (chatSettings.kickChannel.trim()) auto.push('kick');
    if (chatSettings.tiktokUsername.trim()) auto.push('tiktok');
    if (chatSettings.facebookLiveVideoId?.trim()) auto.push('facebook');
    return auto.length > 0 ? auto : ['twitch', 'youtube'];
  }, [chatSettings]);

  const resolvedVisiblePlatforms = useMemo<readonly VisiblePlatform[]>(
    () => (visiblePlatforms.length > 0 ? visiblePlatforms : autoVisiblePlatforms),
    [visiblePlatforms, autoVisiblePlatforms]
  );

  const handleVisibilityToggle = useCallback(
    (platform: VisiblePlatform, enabled: boolean) => {
      // First custom-edit promotes auto → explicit. Subsequent edits
      // operate on the explicit set.
      const base = visiblePlatforms.length > 0 ? visiblePlatforms : [...autoVisiblePlatforms];
      const next = enabled
        ? Array.from(new Set([...base, platform]))
        : base.filter((p) => p !== platform);
      setVisiblePlatforms(next);
      persist({ visiblePlatforms: next });
    },
    [visiblePlatforms, autoVisiblePlatforms, persist]
  );

  const handleVisibilityReset = useCallback(() => {
    setVisiblePlatforms([]);
    persist({ visiblePlatforms: [] });
  }, [persist]);

  const handleVisibilityCollapseToggle = useCallback(() => {
    setVisibilityPanelCollapsed((prev) => {
      const next = !prev;
      persist({ visibilityPanelCollapsed: next });
      return next;
    });
  }, [persist]);

  const handleSendAllToggle = useCallback(
    (enabled: boolean) => {
      setSendAllEnabled(enabled);
      persist({ sendAllEnabled: enabled });
    },
    [persist]
  );

  const handleBlur = useCallback(
    (field: ChatField, value: string) => {
      switch (field) {
        case 'twitch_channel':
          if (value !== chatSettings.twitchChannel) persist({ twitchChannel: value });
          break;
        case 'youtube_channel':
          if (value !== chatSettings.youtubeChannelId) persist({ youtubeChannelId: value });
          break;
        case 'trovo_channel':
          if (value !== chatSettings.trovoChannelId) persist({ trovoChannelId: value });
          break;
        case 'kick_channel':
          if (value !== chatSettings.kickChannel) persist({ kickChannel: value });
          break;
        case 'tiktok_username':
          if (value !== chatSettings.tiktokUsername) persist({ tiktokUsername: value });
          break;
        case 'facebook_video_id':
          // Facebook persistence goes through FacebookConnectGate so the
          // confirm-token gate fires on enable; bypassing it here would
          // defeat the warning UX.
          break;
      }
    },
    [chatSettings, persist]
  );

  if (!currentProfile) {
    return (
      <div className="p-6 text-center text-text-tertiary">
        <MessageSquare className="w-10 h-10 mx-auto mb-3 opacity-60" />
        <p>{t('chat.noProfile', { defaultValue: 'Activate a profile to configure chat.' })}</p>
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-4">
      {/* Visibility panel — collapsible header that lets the user
          customize which platform cards render below. Auto mode
          (visiblePlatforms empty) surfaces every configured platform. */}
      <div className="rounded-xl border border-border-subtle bg-bg-elevated p-4 space-y-3">
        <div className="flex items-center justify-between gap-3">
          <button
            type="button"
            onClick={handleVisibilityCollapseToggle}
            className="flex-1 flex items-center justify-between gap-3 text-left"
            aria-expanded={!visibilityPanelCollapsed}
          >
            <div>
              <p className="text-sm font-medium text-text-primary">
                {t('chat.visibility.title', { defaultValue: 'Visible platforms' })}
              </p>
              <p className="text-xs text-text-tertiary">
                {visiblePlatforms.length > 0
                  ? t('chat.visibility.mode.custom', { defaultValue: 'Custom' })
                  : t('chat.visibility.mode.auto', {
                      defaultValue: 'Auto (configured platforms)',
                    })}
              </p>
            </div>
            <ChevronDown
              className={cn(
                'w-4 h-4 text-text-tertiary transition-transform duration-200',
                !visibilityPanelCollapsed && 'rotate-180'
              )}
            />
          </button>
          <Button
            variant="ghost"
            size="sm"
            onClick={handleVisibilityReset}
            disabled={visiblePlatforms.length === 0}
          >
            {t('chat.visibility.reset', { defaultValue: 'Reset to auto' })}
          </Button>
        </div>
        {!visibilityPanelCollapsed && (
          <>
            <p className="text-xs text-text-tertiary">
              {t('chat.visibility.hint', {
                defaultValue:
                  'By default, every configured platform card is shown. Toggle to customize.',
              })}
            </p>
            <div className="grid grid-cols-1 md:grid-cols-2 gap-2">
              {ALL_VISIBLE_PLATFORMS.map((platform) => (
                <Toggle
                  key={platform}
                  checked={resolvedVisiblePlatforms.includes(platform)}
                  onChange={(enabled) => handleVisibilityToggle(platform, enabled)}
                  label={t(`chat.platforms.${platform}`)}
                />
              ))}
            </div>
          </>
        )}
      </div>

      {/* Broadcast-to-all master toggle. When OFF the ChatComposer
          shows a per-message platform picker; when ON it dispatches to
          every connected platform whose per-platform send flag is on. */}
      <div className="rounded-xl border border-border-subtle bg-bg-elevated p-4">
        <Toggle
          checked={sendAllEnabled}
          onChange={handleSendAllToggle}
          label={t('chat.sendAllEnabled', {
            defaultValue: 'Broadcast outbound messages to all enabled platforms',
          })}
          description={t('chat.sendAllEnabledHint', {
            defaultValue:
              'When off, pick a single target platform per message in the chat composer.',
          })}
        />
      </div>

      {/* Crosspost — `ChatManager::start_message_handler` checks the
          backend `crosspost_enabled` flag on every inbound message and
          forwards to every other connected can_send platform. The UI
          surface for this was dropped in the single-panel rewrite;
          restored here so the existing backend feature has a control. */}
      <div className="rounded-xl border border-border-subtle bg-bg-elevated p-4">
        <Toggle
          checked={chatSettings.crosspostEnabled}
          onChange={(checked) => persist({ crosspostEnabled: checked })}
          label={t('chat.crosspostEnabled', {
            defaultValue: 'Crosspost chat messages',
          })}
          description={t('chat.crosspostHint', {
            defaultValue: 'Relay incoming messages to all enabled platforms that allow sending.',
          })}
        />
      </div>

      {/* Twitch */}
      {resolvedVisiblePlatforms.includes('twitch') && (
        <Card>
          <CardHeader>
            <div>
              <CardTitle>{t('chat.platforms.twitch')}</CardTitle>
              <CardDescription>
                {t('chat.twitch.description', { defaultValue: 'Connect to a Twitch channel chat' })}
              </CardDescription>
            </div>
          </CardHeader>
          <CardBody>
            <Input
              label={t('chat.twitch.channel', { defaultValue: 'Channel name' })}
              value={twitchChannel}
              onChange={(e) => setTwitchChannel(e.target.value)}
              onBlur={(e) => handleBlur('twitch_channel', e.target.value)}
              placeholder={t('chat.twitch.channelPlaceholder', {
                defaultValue: 'e.g. spiritartlife',
              })}
            />
            <PlatformSignInButton
              provider="twitch"
              signedInAs={currentProfile.settings.oauth.twitch.username}
              signInLabel={t('chat.twitch.loginWithTwitch', { defaultValue: 'Login with Twitch' })}
              configured={oauthFlags?.twitchConfigured ?? false}
            />
            <div className="mt-3">
              <Toggle
                checked={twitchSend}
                onChange={(checked) => {
                  setTwitchSend(checked);
                  persist({ twitchSendEnabled: checked });
                }}
                label={t('chat.sendEnabled', { defaultValue: 'Allow sending messages' })}
              />
            </div>
          </CardBody>
        </Card>
      )}

      {/* YouTube */}
      {resolvedVisiblePlatforms.includes('youtube') && (
        <Card>
          <CardHeader>
            <div>
              <CardTitle>{t('chat.platforms.youtube')}</CardTitle>
              <CardDescription>
                {t('chat.youtube.description', { defaultValue: 'Connect to a YouTube live chat' })}
              </CardDescription>
            </div>
          </CardHeader>
          <CardBody>
            <Input
              label={t('chat.youtube.channelId', { defaultValue: 'Channel ID or @handle' })}
              value={youtubeChannelId}
              onChange={(e) => setYoutubeChannelId(e.target.value)}
              onBlur={(e) => handleBlur('youtube_channel', e.target.value)}
              placeholder="UCxxxxxxxxxx or @handle"
            />
            <div className="mt-3">
              <Toggle
                checked={youtubeUseApiKey}
                onChange={(checked) => {
                  setYoutubeUseApiKey(checked);
                  persist({ youtubeUseApiKey: checked });
                }}
                label={t('chat.youtube.useApiKey', {
                  defaultValue: 'Use API key (read-only) instead of OAuth',
                })}
              />
            </div>
            {youtubeUseApiKey && (
              <div className="mt-3">
                <Input
                  label={t('chat.youtube.apiKey', { defaultValue: 'API key' })}
                  value={youtubeApiKey}
                  onChange={(e) => setYoutubeApiKey(e.target.value)}
                  onBlur={(e) => {
                    if (e.target.value !== chatSettings.youtubeApiKey) {
                      persist({ youtubeApiKey: e.target.value });
                    }
                  }}
                  type="password"
                />
              </div>
            )}
            {!youtubeUseApiKey && (
              <>
                <PlatformSignInButton
                  provider="youtube"
                  signedInAs={currentProfile.settings.oauth.youtube.username}
                  configured={oauthFlags?.youtubeConfigured ?? false}
                  signInLabel={t('chat.youtube.loginWithYouTube', {
                    defaultValue: 'Login with YouTube',
                  })}
                />
                <div className="mt-3">
                  <Toggle
                    checked={youtubeSend}
                    onChange={(checked) => {
                      setYoutubeSend(checked);
                      persist({ youtubeSendEnabled: checked });
                    }}
                    label={t('chat.sendEnabled', { defaultValue: 'Allow sending messages' })}
                  />
                </div>
              </>
            )}
          </CardBody>
        </Card>
      )}

      {/* Trovo */}
      {resolvedVisiblePlatforms.includes('trovo') && (
        <Card>
          <CardHeader>
            <div>
              <CardTitle>{t('chat.platforms.trovo')}</CardTitle>
              <CardDescription>
                {t('chat.trovo.description', { defaultValue: 'Read Trovo chat using channel ID' })}
              </CardDescription>
            </div>
          </CardHeader>
          <CardBody>
            <Input
              label={t('chat.trovo.channelId', { defaultValue: 'Channel ID' })}
              value={trovoChannelId}
              onChange={(e) => setTrovoChannelId(e.target.value)}
              onBlur={(e) => handleBlur('trovo_channel', e.target.value)}
              placeholder={t('chat.trovo.channelIdPlaceholder', { defaultValue: 'e.g. 100000021' })}
              helper={t('chat.trovo.channelIdHint', {
                defaultValue: 'Requires SPIRITSTREAM_TROVO_CLIENT_ID in environment.',
              })}
            />
            <div className="mt-3">
              <Toggle
                checked={trovoSend}
                onChange={(checked) => {
                  setTrovoSend(checked);
                  persist({ trovoSendEnabled: checked });
                }}
                label={t('chat.sendEnabled', { defaultValue: 'Allow sending messages' })}
              />
            </div>
          </CardBody>
        </Card>
      )}

      {/* Kick */}
      {resolvedVisiblePlatforms.includes('kick') && (
        <Card>
          <CardHeader>
            <div>
              <CardTitle>{t('chat.platforms.kick')}</CardTitle>
              <CardDescription>
                {t('chat.kick.description', {
                  defaultValue: 'Connect to a Kick channel chat (anonymous read; OAuth for send)',
                })}
              </CardDescription>
            </div>
          </CardHeader>
          <CardBody>
            <Input
              label={t('chat.kick.channel', { defaultValue: 'Channel name' })}
              value={kickChannel}
              onChange={(e) => setKickChannel(e.target.value)}
              onBlur={(e) => handleBlur('kick_channel', e.target.value)}
              placeholder={t('chat.kick.channelPlaceholder', {
                defaultValue: 'e.g. kick_streamer',
              })}
            />
            <PlatformSignInButton
              provider="kick"
              signedInAs={currentProfile.settings.oauth.kick.username}
              signInLabel={t('chat.kick.loginWithKick', { defaultValue: 'Login with Kick' })}
              configured={oauthFlags?.kickConfigured ?? false}
            />
            <div className="mt-3">
              <Toggle
                checked={kickSend}
                onChange={(checked) => {
                  setKickSend(checked);
                  persist({ kickSendEnabled: checked });
                }}
                label={t('chat.kick.sendEnabled', {
                  defaultValue: 'Allow sending messages (requires Sign in with Kick)',
                })}
              />
            </div>
          </CardBody>
        </Card>
      )}

      {/* TikTok */}
      {resolvedVisiblePlatforms.includes('tiktok') && (
        <Card>
          <CardHeader>
            <div>
              <CardTitle>{t('chat.platforms.tiktok')}</CardTitle>
              <CardDescription>
                {t('chat.tiktok.description', {
                  defaultValue: 'Read TikTok Live chat by username (read-only)',
                })}
              </CardDescription>
            </div>
          </CardHeader>
          <CardBody>
            <Input
              label={t('chat.tiktok.username', { defaultValue: 'TikTok username' })}
              value={tiktokUsername}
              onChange={(e) => setTiktokUsername(e.target.value)}
              onBlur={(e) => handleBlur('tiktok_username', e.target.value)}
              placeholder="@username"
            />
            <p role="note" className="text-xs text-warning-text mt-3 p-2 bg-warning-subtle rounded">
              {t('chat.tiktok.readOnlyNotice')}
            </p>
          </CardBody>
        </Card>
      )}

      {/* Facebook — identity-revealing connect gate */}
      {resolvedVisiblePlatforms.includes('facebook') && (
        <FacebookConnectGate oauthConfigured={oauthFlags?.facebookConfigured ?? false} />
      )}
    </div>
  );
}
