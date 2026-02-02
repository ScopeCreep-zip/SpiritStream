import { useState, useEffect, useCallback, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import {
  MessageSquare,
  Send,
  Loader2,
  CheckCircle2,
  AlertCircle,
  Timer,
  Info,
  Image as ImageIcon,
  X,
  Smile,
} from 'lucide-react';
import { Card, CardHeader, CardTitle, CardDescription, CardBody } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { Input } from '@/components/ui/Input';
import { Toggle } from '@/components/ui/Toggle';
import { useProfileStore } from '@/stores/profileStore';
import { dialogs } from '@/lib/backend';
import { api } from '@/lib/backend';
import { toast } from '@/hooks/useToast';
import { cn } from '@/lib/cn';
import type { DiscordSettings } from '@/types/profile';

// Debounce delay for auto-save (ms)
const AUTO_SAVE_DELAY = 500;

// Common emojis for streaming
const EMOJI_CATEGORIES = [
  {
    name: 'Streaming',
    emojis: ['🎮', '🔴', '📺', '🎬', '🎥', '📡', '🎙️', '🎧', '🕹️', '💻'],
  },
  {
    name: 'Reactions',
    emojis: ['🔥', '💯', '⭐', '✨', '💪', '🎉', '🚀', '👀', '❤️', '💜'],
  },
  {
    name: 'Fun',
    emojis: ['😎', '🤩', '😄', '🥳', '👋', '🙌', '👏', '💬', '📢', '🔔'],
  },
];

export function DiscordPanel() {
  const { t } = useTranslation();

  // Get Discord settings from current profile
  const currentProfile = useProfileStore((state) => state.current);
  const updateProfileSettings = useProfileStore((state) => state.updateProfileSettings);
  const discordSettings = currentProfile?.settings?.discord;

  // Local form state
  const [webhookEnabled, setWebhookEnabled] = useState(false);
  const [webhookUrl, setWebhookUrl] = useState('');
  const [goLiveMessage, setGoLiveMessage] = useState('');
  const [cooldownEnabled, setCooldownEnabled] = useState(true);
  const [cooldownSeconds, setCooldownSeconds] = useState('60');
  const [imagePath, setImagePath] = useState('');

  // UI state
  const [isTesting, setIsTesting] = useState(false);
  const [testResult, setTestResult] = useState<{ success: boolean; message: string } | null>(null);
  const [showEmojiPicker, setShowEmojiPicker] = useState(false);

  // Refs
  const saveTimeoutRef = useRef<number | null>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const emojiPickerRef = useRef<HTMLDivElement>(null);

  // Sync form with profile settings when loaded
  useEffect(() => {
    if (discordSettings) {
      setWebhookEnabled(discordSettings.webhookEnabled);
      setWebhookUrl(discordSettings.webhookUrl);
      setGoLiveMessage(
        discordSettings.goLiveMessage || '**Stream is now live!** \n\nCome join the stream!'
      );
      setCooldownEnabled(discordSettings.cooldownEnabled);
      setCooldownSeconds(String(discordSettings.cooldownSeconds));
      setImagePath(discordSettings.imagePath);
    }
  }, [discordSettings]);

  // Cleanup debounce timer on unmount
  useEffect(() => {
    return () => {
      if (saveTimeoutRef.current) {
        window.clearTimeout(saveTimeoutRef.current);
      }
    };
  }, []);

  // Close emoji picker when clicking outside
  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (emojiPickerRef.current && !emojiPickerRef.current.contains(event.target as Node)) {
        setShowEmojiPicker(false);
      }
    };

    if (showEmojiPicker) {
      document.addEventListener('mousedown', handleClickOutside);
      return () => document.removeEventListener('mousedown', handleClickOutside);
    }
  }, [showEmojiPicker]);

  // Auto-save with debounce - saves to profile settings
  const autoSave = useCallback(
    (updates: Partial<DiscordSettings>) => {
      if (saveTimeoutRef.current) {
        window.clearTimeout(saveTimeoutRef.current);
      }

      saveTimeoutRef.current = window.setTimeout(async () => {
        if (!discordSettings) return;
        try {
          await updateProfileSettings({
            discord: { ...discordSettings, ...updates },
          });
        } catch (error) {
          console.error('Failed to save Discord setting:', error);
        }
      }, AUTO_SAVE_DELAY);
    },
    [discordSettings, updateProfileSettings]
  );

  // Handle webhook enabled toggle
  const handleEnabledChange = useCallback(
    async (checked: boolean) => {
      setWebhookEnabled(checked);
      if (!discordSettings) return;
      try {
        await updateProfileSettings({
          discord: { ...discordSettings, webhookEnabled: checked },
        });
      } catch (error) {
        console.error('Failed to save webhook enabled state:', error);
      }
    },
    [discordSettings, updateProfileSettings]
  );

  // Handle webhook URL blur (auto-save)
  const handleUrlBlur = useCallback(() => {
    if (discordSettings && webhookUrl !== discordSettings.webhookUrl) {
      autoSave({ webhookUrl });
    }
  }, [webhookUrl, discordSettings, autoSave]);

  // Handle message blur (auto-save)
  const handleMessageBlur = useCallback(() => {
    if (discordSettings && goLiveMessage !== discordSettings.goLiveMessage) {
      autoSave({ goLiveMessage });
    }
  }, [goLiveMessage, discordSettings, autoSave]);

  // Handle cooldown enabled toggle
  const handleCooldownEnabledChange = useCallback(
    async (checked: boolean) => {
      setCooldownEnabled(checked);
      if (!discordSettings) return;
      try {
        await updateProfileSettings({
          discord: { ...discordSettings, cooldownEnabled: checked },
        });
      } catch (error) {
        console.error('Failed to save cooldown enabled state:', error);
      }
    },
    [discordSettings, updateProfileSettings]
  );

  // Handle cooldown seconds blur (auto-save)
  const handleCooldownBlur = useCallback(() => {
    const seconds = parseInt(cooldownSeconds, 10) || 60;
    if (discordSettings && seconds !== discordSettings.cooldownSeconds) {
      autoSave({ cooldownSeconds: seconds });
    }
  }, [cooldownSeconds, discordSettings, autoSave]);

  // Handle image selection
  const handleSelectImage = useCallback(async () => {
    try {
      const result = await dialogs.openFilePath({
        title: t('discord.selectImage'),
        filters: [{ name: 'Images', extensions: ['png', 'jpg', 'jpeg', 'gif', 'webp'] }],
      });

      if (result && discordSettings) {
        setImagePath(result);
        await updateProfileSettings({
          discord: { ...discordSettings, imagePath: result },
        });
      }
    } catch (error) {
      console.error('Failed to select image:', error);
      toast.error(t('common.error'));
    }
  }, [t, discordSettings, updateProfileSettings]);

  // Handle image removal
  const handleRemoveImage = useCallback(async () => {
    setImagePath('');
    if (!discordSettings) return;
    try {
      await updateProfileSettings({
        discord: { ...discordSettings, imagePath: '' },
      });
    } catch (error) {
      console.error('Failed to remove image:', error);
    }
  }, [discordSettings, updateProfileSettings]);

  // Handle emoji selection
  const handleEmojiSelect = useCallback((emoji: string) => {
    const textarea = textareaRef.current;
    if (textarea) {
      const start = textarea.selectionStart;
      const end = textarea.selectionEnd;
      const newMessage = goLiveMessage.slice(0, start) + emoji + goLiveMessage.slice(end);
      setGoLiveMessage(newMessage);

      // Restore cursor position after emoji
      setTimeout(() => {
        textarea.focus();
        textarea.setSelectionRange(start + emoji.length, start + emoji.length);
      }, 0);
    } else {
      setGoLiveMessage((prev) => prev + emoji);
    }
    setShowEmojiPicker(false);
  }, [goLiveMessage]);

  // Test webhook
  const handleTestWebhook = useCallback(async () => {
    if (!webhookUrl.trim()) {
      toast.error(t('discord.enterWebhookFirst'));
      return;
    }

    setIsTesting(true);
    setTestResult(null);

    try {
      const result = await api.discord.testWebhook(webhookUrl);
      setTestResult(result);

      if (result.success) {
        toast.success(t('discord.testSuccess'));
      } else {
        toast.error(result.message);
      }
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      setTestResult({ success: false, message });
      toast.error(message);
    } finally {
      setIsTesting(false);
    }
  }, [webhookUrl, t]);

  // Validate webhook URL format
  const isValidWebhookUrl =
    webhookUrl.startsWith('https://discord.com/api/webhooks/') ||
    webhookUrl.startsWith('https://discordapp.com/api/webhooks/');

  // Get filename from path
  const imageFileName = imagePath ? imagePath.split(/[\\/]/).pop() : null;

  // Show message if no profile is loaded
  if (!currentProfile) {
    return (
      <div className="flex items-center justify-center p-8 text-[var(--text-tertiary)]">
        {t('common.loadProfileFirst', 'Please load a profile first')}
      </div>
    );
  }

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex items-center gap-3">
        <div className="p-2 rounded-lg bg-[var(--bg-elevated)]">
          <MessageSquare className="w-5 h-5 text-[#5865F2]" />
        </div>
        <div>
          <h2 className="text-lg font-semibold text-[var(--text-primary)]">{t('discord.title')}</h2>
          <p className="text-sm text-[var(--text-secondary)]">{t('discord.description')}</p>
        </div>
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        {/* Webhook Settings Card */}
        <Card>
          <CardHeader>
            <CardTitle>{t('discord.webhookSettings')}</CardTitle>
            <CardDescription>{t('discord.webhookSettingsDescription')}</CardDescription>
          </CardHeader>
          <CardBody className="space-y-4">
            {/* Enable Toggle */}
            <Toggle
              checked={webhookEnabled}
              onChange={handleEnabledChange}
              label={t('discord.enableWebhook')}
              description={t('discord.enableWebhookDescription')}
            />

            {/* Webhook URL */}
            <div className="space-y-2">
              <Input
                label={t('discord.webhookUrl')}
                value={webhookUrl}
                onChange={(e) => setWebhookUrl(e.target.value)}
                onBlur={handleUrlBlur}
                placeholder="https://discord.com/api/webhooks/..."
                disabled={!webhookEnabled}
              />
              {webhookUrl && !isValidWebhookUrl && (
                <div className="flex items-center gap-2 text-xs text-[var(--status-error)]">
                  <AlertCircle className="w-3 h-3" />
                  <span>{t('discord.invalidWebhookUrl')}</span>
                </div>
              )}
            </div>

            {/* Test Button */}
            <div className="flex items-center gap-3">
              <Button
                variant="outline"
                onClick={handleTestWebhook}
                disabled={!webhookEnabled || !webhookUrl || isTesting}
              >
                {isTesting ? (
                  <Loader2 className="w-4 h-4 animate-spin" />
                ) : (
                  <Send className="w-4 h-4" />
                )}
                {t('discord.testWebhook')}
              </Button>
              {testResult && (
                <div
                  className={cn(
                    'flex items-center gap-2 text-sm',
                    testResult.success ? 'text-[var(--status-live)]' : 'text-[var(--status-error)]'
                  )}
                >
                  {testResult.success ? (
                    <CheckCircle2 className="w-4 h-4" />
                  ) : (
                    <AlertCircle className="w-4 h-4" />
                  )}
                  <span>{testResult.success ? t('discord.testPassed') : t('discord.testFailed')}</span>
                </div>
              )}
            </div>
          </CardBody>
        </Card>

        {/* Cooldown Settings Card */}
        <Card>
          <CardHeader>
            <CardTitle>{t('discord.cooldownSettings')}</CardTitle>
            <CardDescription>{t('discord.cooldownDescription')}</CardDescription>
          </CardHeader>
          <CardBody className="space-y-4">
            {/* Cooldown Toggle */}
            <Toggle
              checked={cooldownEnabled}
              onChange={handleCooldownEnabledChange}
              label={t('discord.enableCooldown')}
              description={t('discord.enableCooldownDescription')}
              disabled={!webhookEnabled}
            />

            {/* Cooldown Seconds */}
            <div className="flex items-end gap-3">
              <div className="w-32">
                <Input
                  label={t('discord.cooldownSeconds')}
                  type="number"
                  min="0"
                  max="3600"
                  value={cooldownSeconds}
                  onChange={(e) => setCooldownSeconds(e.target.value)}
                  onBlur={handleCooldownBlur}
                  disabled={!webhookEnabled || !cooldownEnabled}
                />
              </div>
              <div className="flex items-center gap-2 pb-2 text-sm text-[var(--text-tertiary)]">
                <Timer className="w-4 h-4" />
                <span>{t('discord.seconds')}</span>
              </div>
            </div>

            {/* Info about cooldown */}
            <div className="flex items-start gap-2 p-3 rounded-lg bg-[var(--bg-base)] border border-[var(--border-default)]">
              <Info className="w-4 h-4 text-[var(--text-tertiary)] flex-shrink-0 mt-0.5" />
              <p className="text-xs text-[var(--text-tertiary)]">{t('discord.cooldownInfo')}</p>
            </div>
          </CardBody>
        </Card>
      </div>

      {/* Go-Live Message Card */}
      <Card>
        <CardHeader>
          <CardTitle>{t('discord.goLiveMessage')}</CardTitle>
          <CardDescription>{t('discord.goLiveMessageDescription')}</CardDescription>
        </CardHeader>
        <CardBody className="space-y-4">
          {/* Message Textarea with Emoji Button */}
          <div className="space-y-2">
            <div className="relative">
              <textarea
                ref={textareaRef}
                value={goLiveMessage}
                onChange={(e) => setGoLiveMessage(e.target.value)}
                onBlur={handleMessageBlur}
                disabled={!webhookEnabled}
                rows={4}
                className={cn(
                  'w-full px-3 py-2 pr-10 rounded-lg',
                  'bg-[var(--bg-input)] border border-[var(--border-default)]',
                  'text-sm text-[var(--text-primary)]',
                  'placeholder:text-[var(--text-placeholder)]',
                  'focus:outline-none focus:ring-2 focus:ring-[var(--ring-default)] focus:border-transparent',
                  'disabled:opacity-50 disabled:cursor-not-allowed',
                  'resize-y min-h-[100px]'
                )}
                placeholder={t('discord.messagePlaceholder')}
              />
              {/* Emoji Picker Button */}
              <div className="absolute right-2 top-2" ref={emojiPickerRef}>
                <button
                  type="button"
                  onClick={() => setShowEmojiPicker(!showEmojiPicker)}
                  disabled={!webhookEnabled}
                  className={cn(
                    'p-1.5 rounded-md transition-colors',
                    'text-[var(--text-tertiary)] hover:text-[var(--text-primary)]',
                    'hover:bg-[var(--bg-muted)]',
                    'disabled:opacity-50 disabled:cursor-not-allowed'
                  )}
                  title={t('discord.insertEmoji')}
                >
                  <Smile className="w-4 h-4" />
                </button>

                {/* Emoji Picker Dropdown */}
                {showEmojiPicker && (
                  <div className="absolute right-0 top-full mt-1 z-50 p-2 rounded-lg bg-[var(--bg-surface)] border border-[var(--border-default)] shadow-lg w-64">
                    {EMOJI_CATEGORIES.map((category) => (
                      <div key={category.name} className="mb-2 last:mb-0">
                        <div className="text-xs text-[var(--text-tertiary)] mb-1 px-1">
                          {category.name}
                        </div>
                        <div className="flex flex-wrap gap-1">
                          {category.emojis.map((emoji) => (
                            <button
                              key={emoji}
                              type="button"
                              onClick={() => handleEmojiSelect(emoji)}
                              className="p-1.5 rounded hover:bg-[var(--bg-muted)] text-lg transition-colors"
                            >
                              {emoji}
                            </button>
                          ))}
                        </div>
                      </div>
                    ))}
                  </div>
                )}
              </div>
            </div>
          </div>

          {/* Markdown hint */}
          <div className="flex items-start gap-2 p-3 rounded-lg bg-[var(--bg-base)] border border-[var(--border-default)]">
            <Info className="w-4 h-4 text-[var(--text-tertiary)] flex-shrink-0 mt-0.5" />
            <div className="text-xs text-[var(--text-tertiary)] space-y-1">
              <p>{t('discord.markdownSupport')}</p>
              <p className="font-mono">
                **bold** *italic* __underline__ ~~strikethrough~~ `code` [link](url)
              </p>
            </div>
          </div>

          {/* Image Attachment */}
          <div className="space-y-2">
            <label className="block text-sm font-medium text-[var(--text-primary)]">
              {t('discord.attachImage')}
            </label>
            {imagePath ? (
              <div className="flex items-center gap-3 p-3 rounded-lg bg-[var(--bg-base)] border border-[var(--border-default)]">
                <ImageIcon className="w-5 h-5 text-[var(--text-tertiary)]" />
                <span className="flex-1 text-sm text-[var(--text-primary)] truncate">
                  {imageFileName}
                </span>
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={handleRemoveImage}
                  disabled={!webhookEnabled}
                  title={t('discord.removeImage')}
                >
                  <X className="w-4 h-4" />
                </Button>
              </div>
            ) : (
              <Button
                variant="outline"
                onClick={handleSelectImage}
                disabled={!webhookEnabled}
              >
                <ImageIcon className="w-4 h-4" />
                {t('discord.selectImage')}
              </Button>
            )}
            <p className="text-xs text-[var(--text-tertiary)]">
              {t('discord.imageHint')}
            </p>
          </div>
        </CardBody>
      </Card>
    </div>
  );
}
