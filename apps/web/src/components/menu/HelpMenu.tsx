import React from 'react';
import * as Menubar from '@radix-ui/react-menubar';
import { useTranslation } from 'react-i18next';
import { TRIGGER_CLASS, CONTENT_CLASS, ITEM_CLASS, SHORTCUT_CLASS } from './menuStyles';
import type { ModalName } from '@/hooks/useModalRegistry';

interface HelpMenuProps {
  onOpenModal: (name: ModalName) => void;
}

export function HelpMenu({ onOpenModal }: HelpMenuProps): React.ReactElement {
  const { t } = useTranslation();

  return (
    <Menubar.Menu>
      <Menubar.Trigger className={TRIGGER_CLASS}>
        {t('menu.help.label', { defaultValue: 'Help' })}
      </Menubar.Trigger>
      <Menubar.Portal>
        <Menubar.Content className={CONTENT_CLASS} align="start" sideOffset={4}>
          <Menubar.Item
            className={ITEM_CLASS}
            onSelect={() =>
              window.open(
                'https://github.com/ScopeCreep-zip/SpiritStream',
                '_blank',
                'noopener,noreferrer'
              )
            }
          >
            {t('menu.help.docs', { defaultValue: 'Documentation' })}
          </Menubar.Item>
          <Menubar.Item className={ITEM_CLASS} onSelect={() => onOpenModal('shortcuts')}>
            <span>{t('menu.help.shortcuts', { defaultValue: 'Keyboard shortcuts' })}</span>
            <span className={SHORTCUT_CLASS}>⌘/</span>
          </Menubar.Item>
        </Menubar.Content>
      </Menubar.Portal>
    </Menubar.Menu>
  );
}
