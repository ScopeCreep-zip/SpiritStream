import React, { useCallback } from 'react';
import * as Menubar from '@radix-ui/react-menubar';
import { useTranslation } from 'react-i18next';
import { Check, ChevronRight } from 'lucide-react';
import { api } from '@/lib/client';
import { dialogs } from '@spiritstream/api-client';
import { toast } from '@/hooks/useToast';
import { useProfileStore } from '@/stores/profileStore';
import {
  TRIGGER_CLASS,
  CONTENT_CLASS,
  ITEM_CLASS,
  SEPARATOR_CLASS,
} from './menuStyles';
import type { ModalName } from '@/hooks/useModalRegistry';
import type { Profile } from '@spiritstream/types';

interface FileMenuProps {
  onOpenModal: (name: ModalName) => void;
}

export function FileMenu({ onOpenModal }: FileMenuProps): React.ReactElement {
  const { t } = useTranslation();
  const profileStore = useProfileStore();

  const handleImport = useCallback(async (): Promise<void> => {
    try {
      const selected = await dialogs.openTextFile({
        multiple: false,
        filters: [{ name: 'Profile', extensions: ['json'] }],
      });
      if (!selected) return;
      const profile = JSON.parse(selected.content) as Profile;
      if (!profile.name || !profile.outputGroups) {
        throw new Error(t('errors.invalidProfileFormat'));
      }
      await api.profile.save(profile);
      await profileStore.loadProfiles();
      toast.success(t('toast.profileImported', { name: profile.name }));
    } catch (err) {
      toast.error(
        t('toast.importFailed', { error: err instanceof Error ? err.message : String(err) }),
      );
    }
  }, [profileStore, t]);

  return (
    <Menubar.Menu>
      <Menubar.Trigger className={TRIGGER_CLASS}>{t('menu.file', { defaultValue: 'File' })}</Menubar.Trigger>
      <Menubar.Portal>
        <Menubar.Content className={CONTENT_CLASS} align="start" sideOffset={4}>
          <Menubar.Item className={ITEM_CLASS} onSelect={() => onOpenModal('profileCreate')}>
            {t('menu.file.newProfile', { defaultValue: 'New profile…' })}
          </Menubar.Item>
          <Menubar.Sub>
            <Menubar.SubTrigger className={ITEM_CLASS}>
              {t('menu.file.switchProfile', { defaultValue: 'Switch profile' })}
              <ChevronRight className="w-4 h-4" />
            </Menubar.SubTrigger>
            <Menubar.Portal>
              <Menubar.SubContent className={CONTENT_CLASS}>
                {profileStore.profiles.length === 0 ? (
                  <Menubar.Item className={ITEM_CLASS} disabled>
                    {t('menu.file.noProfiles', { defaultValue: 'No profiles yet' })}
                  </Menubar.Item>
                ) : (
                  profileStore.profiles.map((p) => (
                    <Menubar.Item
                      key={p.name}
                      className={ITEM_CLASS}
                      onSelect={() => profileStore.selectProfile(p.name)}
                    >
                      <span>{p.name}</span>
                      {profileStore.current?.name === p.name && <Check className="w-4 h-4" />}
                    </Menubar.Item>
                  ))
                )}
              </Menubar.SubContent>
            </Menubar.Portal>
          </Menubar.Sub>
          <Menubar.Separator className={SEPARATOR_CLASS} />
          <Menubar.Item className={ITEM_CLASS} onSelect={handleImport}>
            {t('menu.file.import', { defaultValue: 'Import profile…' })}
          </Menubar.Item>
        </Menubar.Content>
      </Menubar.Portal>
    </Menubar.Menu>
  );
}
