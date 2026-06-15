import React from 'react';
import { useTranslation } from 'react-i18next';
import { Modal } from '@/components/ui/Modal';
import { OutputGroupForm } from '@/components/forms/OutputGroupForm';
import type { OutputGroup } from '@spiritstream/types';

export interface OutputGroupModalProps {
  open: boolean;
  onClose: () => void;
  mode: 'create' | 'edit';
  group?: OutputGroup;
}

/**
 * Custom output-group editor modal — a thin shell around {@link OutputGroupForm}.
 * Used for creating a group and for editing a SPECIFIC group from its pipeline
 * row. Editing the ACTIVE group's encoder ("Encoder settings") lives instead as
 * the "Encoder" section of the unified settings window, which renders the same
 * {@link OutputGroupForm}.
 */
export function OutputGroupModal({
  open,
  onClose,
  mode,
  group,
}: OutputGroupModalProps): React.ReactElement {
  const { t } = useTranslation();
  const title = mode === 'create' ? t('modals.createOutputGroup') : t('modals.editOutputGroup');
  return (
    <Modal open={open} onClose={onClose} title={title} maxWidth="600px">
      <OutputGroupForm mode={mode} group={group} onDone={onClose} onCancel={onClose} />
    </Modal>
  );
}
