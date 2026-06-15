import React from 'react';
import { useTranslation } from 'react-i18next';
import { Modal } from '@/components/ui/Modal';
import { ProfileForm } from '@/components/forms/ProfileForm';

export interface ProfileModalProps {
  open: boolean;
  onClose: () => void;
}

/**
 * "New profile" modal — a thin shell around {@link ProfileForm} in create
 * mode. Editing an existing profile is no longer a standalone modal; it lives
 * as the "Edit profile" section of the unified settings window, which renders
 * the same {@link ProfileForm} in edit mode.
 */
export function ProfileModal({ open, onClose }: ProfileModalProps): React.ReactElement {
  const { t } = useTranslation();
  return (
    <Modal open={open} onClose={onClose} title={t('modals.createNewProfile')}>
      <ProfileForm mode="create" onDone={onClose} onCancel={onClose} />
    </Modal>
  );
}
