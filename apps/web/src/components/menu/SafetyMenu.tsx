import React, { useCallback } from 'react';
import * as Menubar from '@radix-ui/react-menubar';
import { useTranslation } from 'react-i18next';
import { api } from '@/lib/client';
import { toast } from '@/hooks/useToast';
import {
  TRIGGER_CLASS,
  CONTENT_CLASS,
  ITEM_CLASS,
  SHORTCUT_CLASS,
  SEPARATOR_CLASS,
} from './menuStyles';
import type { SettingsSection } from '@/hooks/useModalRegistry';

interface SafetyMenuProps {
  onOpenSettings: (section: SettingsSection) => void;
}

export function SafetyMenu({ onOpenSettings }: SafetyMenuProps): React.ReactElement {
  const { t } = useTranslation();

  const handlePanic = useCallback(async (): Promise<void> => {
    try {
      const result = await api.safety.panic();
      toast.success(
        t('toast.panicStopped', {
          count: result.streamsStopped,
          ms: result.elapsedMs,
          defaultValue: 'Panic disconnect: stopped {{count}} streams in {{ms}}ms',
        })
      );
    } catch (err) {
      toast.error(
        t('toast.panicFailed', {
          defaultValue: 'Panic disconnect failed: {{error}}',
          error: err instanceof Error ? err.message : String(err),
        })
      );
    }
  }, [t]);

  return (
    <Menubar.Menu>
      <Menubar.Trigger className={TRIGGER_CLASS}>
        {t('menu.safety.label', { defaultValue: 'Safety' })}
      </Menubar.Trigger>
      <Menubar.Portal>
        <Menubar.Content className={CONTENT_CLASS} align="start" sideOffset={4}>
          <Menubar.Item className={ITEM_CLASS} onSelect={handlePanic}>
            <span>{t('menu.safety.panic', { defaultValue: 'Panic disconnect' })}</span>
            <span className={SHORTCUT_CLASS}>⌘P</span>
          </Menubar.Item>
          <Menubar.Separator className={SEPARATOR_CLASS} />
          <Menubar.Item className={ITEM_CLASS} onSelect={() => onOpenSettings('safetyWizard')}>
            {t('menu.safety.wizard', { defaultValue: 'Safety wizard…' })}
          </Menubar.Item>
        </Menubar.Content>
      </Menubar.Portal>
    </Menubar.Menu>
  );
}
