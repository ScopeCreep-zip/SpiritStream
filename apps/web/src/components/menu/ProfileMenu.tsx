import React from 'react';
import * as Menubar from '@radix-ui/react-menubar';
import { useTranslation } from 'react-i18next';
import { useProfileStore } from '@/stores/profileStore';
import { TRIGGER_CLASS, CONTENT_CLASS, ITEM_CLASS } from './menuStyles';
import type { ModalName } from '@/hooks/useModalRegistry';

interface ProfileMenuProps {
  onOpenModal: (name: ModalName) => void;
}

export function ProfileMenu({ onOpenModal }: ProfileMenuProps): React.ReactElement {
  const { t } = useTranslation();
  const current = useProfileStore((s) => s.current);

  return (
    <Menubar.Menu>
      <Menubar.Trigger className={TRIGGER_CLASS}>
        {t('menu.profile.label', { defaultValue: 'Profile' })}
      </Menubar.Trigger>
      <Menubar.Portal>
        <Menubar.Content className={CONTENT_CLASS} align="start" sideOffset={4}>
          <Menubar.Item
            className={ITEM_CLASS}
            disabled={!current}
            onSelect={() => onOpenModal('profileEdit')}
          >
            {t('menu.profile.edit', { defaultValue: 'Edit current…' })}
          </Menubar.Item>
        </Menubar.Content>
      </Menubar.Portal>
    </Menubar.Menu>
  );
}
