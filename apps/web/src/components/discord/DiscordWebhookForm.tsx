import React from 'react';
import { useTranslation } from 'react-i18next';
import { Send, Loader2, CheckCircle2, AlertCircle, Eye, EyeOff, Copy } from 'lucide-react';
import { Card, CardHeader, CardTitle, CardDescription, CardBody } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { Input } from '@/components/ui/Input';
import { Toggle } from '@/components/ui/Toggle';
import { cn } from '@/lib/cn';

interface DiscordWebhookFormProps {
  webhookEnabled: boolean;
  onEnabledChange: (checked: boolean) => void | Promise<void>;

  webhookUrl: string;
  setWebhookUrl: (value: string) => void;
  onUrlBlur: () => void;

  showWebhookUrl: boolean;
  setShowWebhookUrl: (value: boolean) => void;

  onCopyUrl: () => void;
  onTest: () => void;
  isTesting: boolean;
  testResult: { success: boolean; message: string } | null;

  /** Backend-defined webhook URL is valid; controls inline error display. */
  isValidWebhookUrl: boolean;
}

export function DiscordWebhookForm({
  webhookEnabled,
  onEnabledChange,
  webhookUrl,
  setWebhookUrl,
  onUrlBlur,
  showWebhookUrl,
  setShowWebhookUrl,
  onCopyUrl,
  onTest,
  isTesting,
  testResult,
  isValidWebhookUrl,
}: DiscordWebhookFormProps): React.ReactElement {
  const { t } = useTranslation();

  return (
    <Card>
      <CardHeader>
        <CardTitle>{t('discord.webhookSettings')}</CardTitle>
        <CardDescription>{t('discord.webhookSettingsDescription')}</CardDescription>
      </CardHeader>
      <CardBody className="space-y-4">
        <Toggle
          checked={webhookEnabled}
          onChange={onEnabledChange}
          label={t('discord.enableWebhook')}
          description={t('discord.enableWebhookDescription')}
        />

        <div className="space-y-2">
          <div className="flex items-end gap-1">
            <div className="flex-1">
              <Input
                label={t('discord.webhookUrl')}
                type={showWebhookUrl ? 'text' : 'password'}
                value={webhookUrl}
                onChange={(e) => setWebhookUrl(e.target.value)}
                onBlur={onUrlBlur}
                placeholder={t('discord.webhookUrlPlaceholder')}
                disabled={!webhookEnabled}
                autoComplete="off"
              />
            </div>
            <Button
              variant="ghost"
              size="icon"
              onClick={() => setShowWebhookUrl(!showWebhookUrl)}
              aria-label={showWebhookUrl ? t('common.hide') : t('common.show')}
              disabled={!webhookEnabled}
            >
              {showWebhookUrl ? <EyeOff className="w-4 h-4" /> : <Eye className="w-4 h-4" />}
            </Button>
            <Button
              variant="ghost"
              size="icon"
              onClick={onCopyUrl}
              aria-label={t('common.copy')}
              disabled={!webhookEnabled || !webhookUrl}
            >
              <Copy className="w-4 h-4" />
            </Button>
          </div>
          {webhookUrl && !isValidWebhookUrl && (
            <div className="flex items-center gap-2 text-xs text-status-error">
              <AlertCircle className="w-3 h-3" />
              <span>{t('discord.invalidWebhookUrl')}</span>
            </div>
          )}
        </div>

        <div className="flex items-center gap-3">
          <Button
            variant="outline"
            onClick={onTest}
            disabled={!webhookEnabled || !webhookUrl || isTesting}
          >
            {isTesting ? <Loader2 className="w-4 h-4 animate-spin" /> : <Send className="w-4 h-4" />}
            {t('discord.testWebhook')}
          </Button>
          {testResult && (
            <div
              className={cn(
                'flex items-center gap-2 text-sm',
                testResult.success ? 'text-status-live' : 'text-status-error',
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
  );
}
