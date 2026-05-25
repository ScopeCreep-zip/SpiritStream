import { useState, useEffect, useCallback, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { MessageSquare } from 'lucide-react';
import { useProfileStore } from '@/stores/profileStore';
import { useFileBrowser } from '@/hooks/useFileBrowser';
import { api } from '@/lib/client';
import { toast } from '@/hooks/useToast';
import { logger } from '@/lib/logger';
import { clientConfig } from '@/lib/constants';
import { DiscordWebhookForm } from '@/components/discord/DiscordWebhookForm';
import { DiscordCooldownSettings } from '@/components/discord/DiscordCooldownSettings';
import { DiscordMessageTemplate } from '@/components/discord/DiscordMessageTemplate';
import type { DiscordSettings } from '@spiritstream/types';

/**
 * Discord integration orchestrator. Owns the per-profile setting state,
 * debounced auto-save, and unmount-flush behavior. Renders three focused
 * cards (webhook form, cooldown settings, message template) — each is its
 * own component under `components/discord/`.
 */
export function DiscordPanel() {
  const { t } = useTranslation();
  const { FileBrowser, openFilePath: browserOpenFile } = useFileBrowser();

  const currentProfile = useProfileStore((state) => state.current);
  const updateProfileSettings = useProfileStore((state) => state.updateProfileSettings);
  const discordSettings = currentProfile?.settings?.discord;

  // Local form state — mirrors profile settings; saved via autoSave / immediate update.
  const [webhookEnabled, setWebhookEnabled] = useState(false);
  const [webhookUrl, setWebhookUrl] = useState('');
  const [goLiveMessage, setGoLiveMessage] = useState('');
  const [cooldownEnabled, setCooldownEnabled] = useState(true);
  const [cooldownSeconds, setCooldownSeconds] = useState('60');
  const [imagePath, setImagePath] = useState('');

  // UI state.
  const [isTesting, setIsTesting] = useState(false);
  const [testResult, setTestResult] = useState<{ success: boolean; message: string } | null>(null);
  const [showWebhookUrl, setShowWebhookUrl] = useState(false);

  // Debounced save infrastructure.
  const saveTimeoutRef = useRef<number | null>(null);
  const pendingUpdatesRef = useRef<Partial<DiscordSettings> | null>(null);

  // Sync form with profile settings when loaded.
  useEffect(() => {
    if (discordSettings) {
      setWebhookEnabled(discordSettings.webhookEnabled);
      setWebhookUrl(discordSettings.webhookUrl);
      setGoLiveMessage(
        discordSettings.goLiveMessage || '**Stream is now live!** \n\nCome join the stream!',
      );
      setCooldownEnabled(discordSettings.cooldownEnabled);
      setCooldownSeconds(String(discordSettings.cooldownSeconds));
      setImagePath(discordSettings.imagePath);
    }
  }, [discordSettings]);

  // Flush pending saves on unmount (don't lose unsaved changes).
  useEffect(() => {
    return () => {
      if (saveTimeoutRef.current) {
        window.clearTimeout(saveTimeoutRef.current);
      }
      if (pendingUpdatesRef.current && discordSettings) {
        updateProfileSettings({
          discord: { ...discordSettings, ...pendingUpdatesRef.current },
        }).catch((error) => {
          logger.error('Failed to flush Discord settings on unmount:', error);
        });
        pendingUpdatesRef.current = null;
      }
    };
  }, [discordSettings, updateProfileSettings]);

  // Auto-save with debounce — saves to profile settings.
  const autoSave = useCallback(
    (updates: Partial<DiscordSettings>) => {
      pendingUpdatesRef.current = { ...pendingUpdatesRef.current, ...updates };

      if (saveTimeoutRef.current) {
        window.clearTimeout(saveTimeoutRef.current);
      }

      saveTimeoutRef.current = window.setTimeout(async () => {
        if (!discordSettings) return;
        try {
          await updateProfileSettings({
            discord: { ...discordSettings, ...pendingUpdatesRef.current },
          });
          pendingUpdatesRef.current = null;
        } catch (error) {
          logger.error('Failed to save Discord setting:', error);
        }
      }, clientConfig.AUTO_SAVE_DELAY_MS);
    },
    [discordSettings, updateProfileSettings],
  );

  // Webhook handlers.
  const handleEnabledChange = useCallback(
    async (checked: boolean) => {
      setWebhookEnabled(checked);
      if (!discordSettings) return;
      try {
        await updateProfileSettings({
          discord: { ...discordSettings, webhookEnabled: checked },
        });
      } catch (error) {
        logger.error('Failed to save webhook enabled state:', error);
      }
    },
    [discordSettings, updateProfileSettings],
  );

  const handleUrlBlur = useCallback(() => {
    if (discordSettings && webhookUrl !== discordSettings.webhookUrl) {
      autoSave({ webhookUrl });
    }
  }, [webhookUrl, discordSettings, autoSave]);

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

  const handleCopyWebhookUrl = useCallback(async () => {
    if (!webhookUrl) return;
    try {
      await navigator.clipboard.writeText(webhookUrl);
      toast.success(t('common.copied'));
    } catch {
      toast.error(t('common.error'));
    }
  }, [webhookUrl, t]);

  // Cooldown handlers.
  const handleCooldownEnabledChange = useCallback(
    async (checked: boolean) => {
      setCooldownEnabled(checked);
      if (!discordSettings) return;
      try {
        await updateProfileSettings({
          discord: { ...discordSettings, cooldownEnabled: checked },
        });
      } catch (error) {
        logger.error('Failed to save cooldown enabled state:', error);
      }
    },
    [discordSettings, updateProfileSettings],
  );

  const handleCooldownBlur = useCallback(() => {
    const seconds = parseInt(cooldownSeconds, 10) || 60;
    if (discordSettings && seconds !== discordSettings.cooldownSeconds) {
      autoSave({ cooldownSeconds: seconds });
    }
  }, [cooldownSeconds, discordSettings, autoSave]);

  // Message + image handlers.
  const handleMessageBlur = useCallback(() => {
    if (discordSettings && goLiveMessage !== discordSettings.goLiveMessage) {
      autoSave({ goLiveMessage });
    }
  }, [goLiveMessage, discordSettings, autoSave]);

  const handleSelectImage = useCallback(async () => {
    try {
      const result = await browserOpenFile({
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
      logger.error('Failed to select image:', error);
      toast.error(t('common.error'));
    }
  }, [t, discordSettings, updateProfileSettings, browserOpenFile]);

  const handleRemoveImage = useCallback(async () => {
    setImagePath('');
    if (!discordSettings) return;
    try {
      await updateProfileSettings({
        discord: { ...discordSettings, imagePath: '' },
      });
    } catch (error) {
      logger.error('Failed to remove image:', error);
    }
  }, [discordSettings, updateProfileSettings]);

  // Webhook URL prefix list is backend-authoritative
  // (`clientConfig.DISCORD_WEBHOOK_PREFIXES`, hydrated from
  // `GET /api/v1/system/client-config`). Backend is the only source
  // for "what counts as a Discord webhook" so this stays in lockstep
  // with `DiscordWebhookService` server-side.
  const isValidWebhookUrl = clientConfig.DISCORD_WEBHOOK_PREFIXES.some((p) =>
    webhookUrl.startsWith(p),
  );

  if (!currentProfile) {
    return (
      <div className="flex items-center justify-center p-8 text-text-tertiary">
        {t('common.loadProfileFirst', 'Please load a profile first')}
      </div>
    );
  }

  return (
    <div className="space-y-6">
      <FileBrowser />

      <div className="flex items-center gap-3">
        <div className="p-2 rounded-lg bg-bg-elevated">
          <MessageSquare className="w-5 h-5 text-[#5865F2]" />
        </div>
        <div>
          <h2 className="text-lg font-semibold text-text-primary">{t('discord.title')}</h2>
          <p className="text-sm text-text-secondary">{t('discord.description')}</p>
        </div>
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        <DiscordWebhookForm
          webhookEnabled={webhookEnabled}
          onEnabledChange={handleEnabledChange}
          webhookUrl={webhookUrl}
          setWebhookUrl={setWebhookUrl}
          onUrlBlur={handleUrlBlur}
          showWebhookUrl={showWebhookUrl}
          setShowWebhookUrl={setShowWebhookUrl}
          onCopyUrl={handleCopyWebhookUrl}
          onTest={handleTestWebhook}
          isTesting={isTesting}
          testResult={testResult}
          isValidWebhookUrl={isValidWebhookUrl}
        />
        <DiscordCooldownSettings
          webhookEnabled={webhookEnabled}
          cooldownEnabled={cooldownEnabled}
          onCooldownEnabledChange={handleCooldownEnabledChange}
          cooldownSeconds={cooldownSeconds}
          setCooldownSeconds={setCooldownSeconds}
          onCooldownBlur={handleCooldownBlur}
        />
      </div>

      <DiscordMessageTemplate
        webhookEnabled={webhookEnabled}
        goLiveMessage={goLiveMessage}
        setGoLiveMessage={setGoLiveMessage}
        onMessageBlur={handleMessageBlur}
        imagePath={imagePath}
        onSelectImage={handleSelectImage}
        onRemoveImage={handleRemoveImage}
      />
    </div>
  );
}
