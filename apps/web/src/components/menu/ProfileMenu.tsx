import React from 'react';
import * as Menubar from '@radix-ui/react-menubar';
import { useTranslation } from 'react-i18next';
import { useProfileStore } from '@/stores/profileStore';
import { TRIGGER_CLASS, CONTENT_CLASS, ITEM_CLASS, SEPARATOR_CLASS } from './menuStyles';
import type { SettingsSection } from '@/hooks/useModalRegistry';

interface ProfileMenuProps {
  onOpenSettings: (section: SettingsSection) => void;
}

export function ProfileMenu({ onOpenSettings }: ProfileMenuProps): React.ReactElement {
  const { t } = useTranslation();
  const current = useProfileStore((s) => s.current);
  const signOut = useProfileStore((s) => s.signOut);

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
            onSelect={() => onOpenSettings('profileEdit')}
          >
            {t('menu.profile.edit', { defaultValue: 'Edit current…' })}
          </Menubar.Item>
          <Menubar.Separator className={SEPARATOR_CLASS} />
          <Menubar.Item className={ITEM_CLASS} disabled={!current} onSelect={() => void signOut()}>
            {t('menu.profile.signOut', { defaultValue: 'Sign out' })}
          </Menubar.Item>
        </Menubar.Content>
      </Menubar.Portal>
    </Menubar.Menu>
  );
}
