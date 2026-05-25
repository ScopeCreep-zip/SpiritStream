import React from 'react';
import { useTranslation } from 'react-i18next';
import { Timer, Info } from 'lucide-react';
import { Card, CardHeader, CardTitle, CardDescription, CardBody } from '@/components/ui/Card';
import { Input } from '@/components/ui/Input';
import { Toggle } from '@/components/ui/Toggle';

interface DiscordCooldownSettingsProps {
  webhookEnabled: boolean;
  cooldownEnabled: boolean;
  onCooldownEnabledChange: (checked: boolean) => void | Promise<void>;
  cooldownSeconds: string;
  setCooldownSeconds: (value: string) => void;
  onCooldownBlur: () => void;
}

export function DiscordCooldownSettings({
  webhookEnabled,
  cooldownEnabled,
  onCooldownEnabledChange,
  cooldownSeconds,
  setCooldownSeconds,
  onCooldownBlur,
}: DiscordCooldownSettingsProps): React.ReactElement {
  const { t } = useTranslation();

  return (
    <Card>
      <CardHeader>
        <CardTitle>{t('discord.cooldownSettings')}</CardTitle>
        <CardDescription>{t('discord.cooldownDescription')}</CardDescription>
      </CardHeader>
      <CardBody className="space-y-4">
        <Toggle
          checked={cooldownEnabled}
          onChange={onCooldownEnabledChange}
          label={t('discord.enableCooldown')}
          description={t('discord.enableCooldownDescription')}
          disabled={!webhookEnabled}
        />

        <div className="flex items-end gap-3">
          <div className="w-32">
            <Input
              label={t('discord.cooldownSeconds')}
              type="number"
              min="0"
              max="3600"
              value={cooldownSeconds}
              onChange={(e) => setCooldownSeconds(e.target.value)}
              onBlur={onCooldownBlur}
              disabled={!webhookEnabled || !cooldownEnabled}
            />
          </div>
          <div className="flex items-center gap-2 pb-2 text-sm text-text-tertiary">
            <Timer className="w-4 h-4" />
            <span>{t('discord.seconds')}</span>
          </div>
        </div>

        <div className="flex items-start gap-2 p-3 rounded-lg bg-bg-base border border-border-default">
          <Info className="w-4 h-4 text-text-tertiary flex-shrink-0 mt-0.5" />
          <p className="text-xs text-text-tertiary">{t('discord.cooldownInfo')}</p>
        </div>
      </CardBody>
    </Card>
  );
}
