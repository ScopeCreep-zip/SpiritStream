import React, { useCallback, useState } from 'react';
import * as Menubar from '@radix-ui/react-menubar';
import { useTranslation } from 'react-i18next';
import { Check, ChevronRight } from 'lucide-react';
import { api } from '@/lib/client';
import { dialogs } from '@spiritstream/api-client';
import { ConfirmDialog } from '@spiritstream/ui';
import { toast } from '@/hooks/useToast';
import { useProfileStore } from '@/stores/profileStore';
import { logger } from '@/lib/logger';
import { TRIGGER_CLASS, CONTENT_CLASS, ITEM_CLASS, SEPARATOR_CLASS } from './menuStyles';
import type { ModalName } from '@/hooks/useModalRegistry';
import type { Profile } from '@spiritstream/types';

interface FileMenuProps {
  onOpenModal: (name: ModalName) => void;
}

export function FileMenu({ onOpenModal }: FileMenuProps): React.ReactElement {
  const { t } = useTranslation();
  const profileStore = useProfileStore();
  // ConfirmDialog state for the destructive Delete profile path.
  // Inline rather than via a hypothetical `dialogs.confirm()` because
  // the codebase already standardises on `<ConfirmDialog>` for this
  // shape (see DataManagementSection clear-data flow).
  const [pendingDelete, setPendingDelete] = useState<string | null>(null);

  const performDelete = useCallback(
    async (name: string): Promise<void> => {
      try {
        await profileStore.deleteProfile(name);
        toast.success(
          t('toast.profileDeleted', {
            defaultValue: 'Deleted profile {{name}}',
            name,
          })
        );
      } catch (err) {
        logger.error('[file-menu] delete profile failed', err);
        const message = err instanceof Error ? err.message : String(err);
        toast.error(
          t('toast.deleteProfileFailed', {
            defaultValue: 'Failed to delete {{name}}: {{error}}',
            name,
            error: message,
          })
        );
      }
    },
    [profileStore, t]
  );

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
        t('toast.importFailed', { error: err instanceof Error ? err.message : String(err) })
      );
    }
  }, [profileStore, t]);

  return (
    <Menubar.Menu>
      <Menubar.Trigger className={TRIGGER_CLASS}>
        {t('menu.file', { defaultValue: 'File' })}
      </Menubar.Trigger>
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
          <Menubar.Sub>
            <Menubar.SubTrigger className={ITEM_CLASS}>
              {t('menu.file.removeEncryption', { defaultValue: 'Remove encryption…' })}
              <ChevronRight className="w-4 h-4" />
            </Menubar.SubTrigger>
            <Menubar.Portal>
              <Menubar.SubContent className={CONTENT_CLASS}>
                {profileStore.profiles.filter((p) => p.isEncrypted).length === 0 ? (
                  <Menubar.Item className={ITEM_CLASS} disabled>
                    {t('menu.file.noEncryptedProfiles', {
                      defaultValue: 'No encrypted profiles',
                    })}
                  </Menubar.Item>
                ) : (
                  profileStore.profiles
                    .filter((p) => p.isEncrypted)
                    .map((p) => (
                      <Menubar.Item
                        key={p.name}
                        className={ITEM_CLASS}
                        onSelect={() => profileStore.unlockProfile(p.name)}
                      >
                        {p.name}
                      </Menubar.Item>
                    ))
                )}
              </Menubar.SubContent>
            </Menubar.Portal>
          </Menubar.Sub>
          <Menubar.Sub>
            <Menubar.SubTrigger className={ITEM_CLASS}>
              {t('menu.file.deleteProfile', { defaultValue: 'Delete profile' })}
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
                      onSelect={() => setPendingDelete(p.name)}
                    >
                      {p.name}
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
      <ConfirmDialog
        open={pendingDelete !== null}
        title={t('toast.deleteProfileTitle', { defaultValue: 'Delete profile?' })}
        message={t('toast.deleteProfileConfirm', {
          defaultValue: 'Delete profile "{{name}}"? This cannot be undone.',
          name: pendingDelete ?? '',
        })}
        confirmLabel={t('common.delete', { defaultValue: 'Delete' })}
        cancelLabel={t('common.cancel', { defaultValue: 'Cancel' })}
        confirmVariant="danger"
        onConfirm={() => {
          const name = pendingDelete;
          setPendingDelete(null);
          if (name) {
            void performDelete(name);
          }
        }}
        onCancel={() => setPendingDelete(null)}
      />
    </Menubar.Menu>
  );
}
