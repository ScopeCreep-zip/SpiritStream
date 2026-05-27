import { useEffect, useState, useCallback, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { MessageSquare } from 'lucide-react';
import { useProfileStore } from '@/stores/profileStore';
import { toast } from '@/hooks/useToast';
import { logger } from '@/lib/logger';
import { createDefaultChatSettings } from '@/lib/profile-helpers';
import { Card, CardHeader, CardTitle, CardDescription, CardBody } from '@/components/ui/Card';
import { Input } from '@/components/ui/Input';
import { Toggle } from '@/components/ui/Toggle';
import { FacebookConnectGate } from '@/components/chat/settings/FacebookConnectGate';
import { PlatformSignInButton } from '@/components/chat/settings/PlatformSignInButton';

type ChatField =
  | 'twitch_channel'
  | 'youtube_channel'
  | 'trovo_channel'
  | 'stripchat_username'
  | 'kick_channel'
  | 'tiktok_username'
  | 'facebook_video_id';

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

  const chatSettings = useMemo(
    () => currentProfile?.settings?.chat ?? createDefaultChatSettings(),
    [currentProfile],
  );

  const [twitchChannel, setTwitchChannel] = useState('');
  const [twitchSend, setTwitchSend] = useState(false);
  const [youtubeChannelId, setYoutubeChannelId] = useState('');
  const [youtubeApiKey, setYoutubeApiKey] = useState('');
  const [youtubeUseApiKey, setYoutubeUseApiKey] = useState(false);
  const [youtubeSend, setYoutubeSend] = useState(false);
  const [trovoChannelId, setTrovoChannelId] = useState('');
  const [trovoSend, setTrovoSend] = useState(false);
  const [stripchatUsername, setStripchatUsername] = useState('');
  const [kickChannel, setKickChannel] = useState('');
  const [kickSend, setKickSend] = useState(false);
  const [tiktokUsername, setTiktokUsername] = useState('');

  useEffect(() => {
    setTwitchChannel(chatSettings.twitchChannel);
    setTwitchSend(chatSettings.twitchSendEnabled);
    setYoutubeChannelId(chatSettings.youtubeChannelId);
    setYoutubeApiKey(chatSettings.youtubeApiKey);
    setYoutubeUseApiKey(chatSettings.youtubeUseApiKey);
    setYoutubeSend(chatSettings.youtubeSendEnabled);
    setTrovoChannelId(chatSettings.trovoChannelId);
    setTrovoSend(chatSettings.trovoSendEnabled);
    setStripchatUsername(chatSettings.stripchatUsername);
    setKickChannel(chatSettings.kickChannel);
    setKickSend(chatSettings.kickSendEnabled);
    setTiktokUsername(chatSettings.tiktokUsername);
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
    [chatSettings, updateProfileSettings, t],
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
        case 'stripchat_username':
          if (value !== chatSettings.stripchatUsername) persist({ stripchatUsername: value });
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
    [chatSettings, persist],
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
      {/* Twitch */}
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
            placeholder={t('chat.twitch.channelPlaceholder', { defaultValue: 'e.g. spiritartlife' })}
          />
          <PlatformSignInButton
            provider="twitch"
            signedInAs={currentProfile.settings.oauth.twitch.username}
            signInLabel={t('chat.twitch.loginWithTwitch', { defaultValue: 'Login with Twitch' })}
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

      {/* YouTube */}
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

      {/* Trovo */}
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

      {/* Stripchat */}
      <Card>
        <CardHeader>
          <div>
            <CardTitle>{t('chat.platforms.stripchat')}</CardTitle>
            <CardDescription>
              {t('chat.stripchat.description', { defaultValue: 'Stripchat (configuration placeholder)' })}
            </CardDescription>
          </div>
        </CardHeader>
        <CardBody>
          <Input
            label={t('chat.stripchat.username', { defaultValue: 'Model username' })}
            value={stripchatUsername}
            onChange={(e) => setStripchatUsername(e.target.value)}
            onBlur={(e) => handleBlur('stripchat_username', e.target.value)}
            placeholder="model_username"
          />
          <p className="text-xs text-text-tertiary mt-2">
            {t('chat.stripchat.unavailableHint', {
              defaultValue:
                'Public Stripchat docs currently expose studio stats APIs, not a stable chat API.',
            })}
          </p>
        </CardBody>
      </Card>

      {/* Kick */}
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
            placeholder={t('chat.kick.channelPlaceholder', { defaultValue: 'e.g. kick_streamer' })}
          />
          <PlatformSignInButton
            provider="kick"
            signedInAs={currentProfile.settings.oauth.kick.username}
            signInLabel={t('chat.kick.loginWithKick', { defaultValue: 'Login with Kick' })}
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

      {/* TikTok */}
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

      {/* Facebook — identity-revealing connect gate */}
      <FacebookConnectGate />
    </div>
  );
}
