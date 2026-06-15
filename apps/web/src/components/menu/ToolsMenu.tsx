import React from 'react';
import * as Menubar from '@radix-ui/react-menubar';
import { useTranslation } from 'react-i18next';
import {
  TRIGGER_CLASS,
  CONTENT_CLASS,
  ITEM_CLASS,
  SHORTCUT_CLASS,
  SEPARATOR_CLASS,
} from './menuStyles';
import type { SettingsSection } from '@/hooks/useModalRegistry';

interface ToolsMenuProps {
  onOpenSettings: (section: SettingsSection) => void;
}

export function ToolsMenu({ onOpenSettings }: ToolsMenuProps): React.ReactElement {
  const { t } = useTranslation();

  return (
    <Menubar.Menu>
      <Menubar.Trigger className={TRIGGER_CLASS}>
        {t('menu.tools.label', { defaultValue: 'Tools' })}
      </Menubar.Trigger>
      <Menubar.Portal>
        <Menubar.Content className={CONTENT_CLASS} align="start" sideOffset={4}>
          <Menubar.Item className={ITEM_CLASS} onSelect={() => onOpenSettings('obs')}>
            {t('menu.tools.obs', { defaultValue: 'OBS connection…' })}
          </Menubar.Item>
          <Menubar.Item className={ITEM_CLASS} onSelect={() => onOpenSettings('chat')}>
            {t('menu.tools.chat', { defaultValue: 'Chat platforms…' })}
          </Menubar.Item>
          <Menubar.Item className={ITEM_CLASS} onSelect={() => onOpenSettings('discord')}>
            {t('menu.tools.discord', { defaultValue: 'Discord notifications…' })}
          </Menubar.Item>
          <Menubar.Separator className={SEPARATOR_CLASS} />
          <Menubar.Item className={ITEM_CLASS} onSelect={() => onOpenSettings('settings')}>
            <span>{t('menu.tools.settings', { defaultValue: 'Settings…' })}</span>
            <span className={SHORTCUT_CLASS}>⌘,</span>
          </Menubar.Item>
          <Menubar.Separator className={SEPARATOR_CLASS} />
          <Menubar.Item className={ITEM_CLASS} onSelect={() => onOpenSettings('audit')}>
            {t('menu.tools.audit', { defaultValue: 'Audit log…' })}
          </Menubar.Item>
          <Menubar.Item className={ITEM_CLASS} onSelect={() => onOpenSettings('logs')}>
            {t('menu.tools.logs', { defaultValue: 'Logs…' })}
          </Menubar.Item>
        </Menubar.Content>
      </Menubar.Portal>
    </Menubar.Menu>
  );
}
