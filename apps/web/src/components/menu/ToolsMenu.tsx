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
import type { ModalName } from '@/hooks/useModalRegistry';

interface ToolsMenuProps {
  onOpenModal: (name: ModalName) => void;
}

export function ToolsMenu({ onOpenModal }: ToolsMenuProps): React.ReactElement {
  const { t } = useTranslation();

  return (
    <Menubar.Menu>
      <Menubar.Trigger className={TRIGGER_CLASS}>
        {t('menu.tools', { defaultValue: 'Tools' })}
      </Menubar.Trigger>
      <Menubar.Portal>
        <Menubar.Content className={CONTENT_CLASS} align="start" sideOffset={4}>
          <Menubar.Item className={ITEM_CLASS} onSelect={() => onOpenModal('obs')}>
            {t('menu.tools.obs', { defaultValue: 'OBS connection…' })}
          </Menubar.Item>
          <Menubar.Item className={ITEM_CLASS} onSelect={() => onOpenModal('chat')}>
            {t('menu.tools.chat', { defaultValue: 'Chat platforms…' })}
          </Menubar.Item>
          <Menubar.Item className={ITEM_CLASS} onSelect={() => onOpenModal('discord')}>
            {t('menu.tools.discord', { defaultValue: 'Discord notifications…' })}
          </Menubar.Item>
          <Menubar.Separator className={SEPARATOR_CLASS} />
          <Menubar.Item className={ITEM_CLASS} onSelect={() => onOpenModal('settings')}>
            <span>{t('menu.tools.settings', { defaultValue: 'Settings…' })}</span>
            <span className={SHORTCUT_CLASS}>⌘,</span>
          </Menubar.Item>
          <Menubar.Separator className={SEPARATOR_CLASS} />
          <Menubar.Item className={ITEM_CLASS} onSelect={() => onOpenModal('audit')}>
            {t('menu.tools.audit', { defaultValue: 'Audit log…' })}
          </Menubar.Item>
          <Menubar.Item className={ITEM_CLASS} onSelect={() => onOpenModal('logs')}>
            {t('menu.tools.logs', { defaultValue: 'Logs…' })}
          </Menubar.Item>
        </Menubar.Content>
      </Menubar.Portal>
    </Menubar.Menu>
  );
}
