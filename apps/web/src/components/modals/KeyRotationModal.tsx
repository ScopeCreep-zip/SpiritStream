import { useTranslation } from 'react-i18next';
import { CheckCircle2, Loader2 } from 'lucide-react';
import { Modal } from '@/components/ui/Modal';
import { Alert } from '@/components/ui/Alert';
import { Button } from '@/components/ui/Button';

export interface KeyRotationModalProps {
  open: boolean;
  onClose: () => void;
  onConfirm: () => void;
  inProgress?: boolean;
  error?: string | null;
}

export function KeyRotationModal({
  open,
  onClose,
  onConfirm,
  inProgress = false,
  error,
}: KeyRotationModalProps) {
  const { t } = useTranslation();

  const handleClose = () => {
    if (inProgress) return;
    onClose();
  };

  return (
    <Modal
      open={open}
      onClose={handleClose}
      title={t('settings.rotateMachineKeyTitle')}
      footer={
        <>
          <Button variant="ghost" onClick={handleClose} disabled={inProgress}>
            {t('common.cancel')}
          </Button>
          <Button onClick={onConfirm} disabled={inProgress} loading={inProgress}>
            {t('settings.rotateMachineKey')}
          </Button>
        </>
      }
    >
      <div className="flex flex-col" style={{ gap: '16px' }}>
        <p className="text-sm text-[var(--text-secondary)]">
          {t('settings.rotateMachineKeyDescription')}
        </p>

        <div className="flex flex-col" style={{ gap: '10px' }}>
          <div className="text-sm font-medium text-[var(--text-primary)]">
            {t('settings.rotationStepsTitle')}
          </div>
          <div className="flex items-start" style={{ gap: '8px' }}>
            <CheckCircle2 className="w-4 h-4 text-[var(--success-text)] mt-0.5" />
            <span className="text-sm text-[var(--text-secondary)]">
              {t('settings.rotationStepGenerate')}
            </span>
          </div>
          <div className="flex items-start" style={{ gap: '8px' }}>
            <CheckCircle2 className="w-4 h-4 text-[var(--success-text)] mt-0.5" />
            <span className="text-sm text-[var(--text-secondary)]">
              {t('settings.rotationStepBackup')}
            </span>
          </div>
          <div className="flex items-start" style={{ gap: '8px' }}>
            <CheckCircle2 className="w-4 h-4 text-[var(--success-text)] mt-0.5" />
            <span className="text-sm text-[var(--text-secondary)]">
              {t('settings.rotationStepReencrypt')}
            </span>
          </div>
          <div className="flex items-start" style={{ gap: '8px' }}>
            <CheckCircle2 className="w-4 h-4 text-[var(--success-text)] mt-0.5" />
            <span className="text-sm text-[var(--text-secondary)]">
              {t('settings.rotationStepDelete')}
            </span>
          </div>
        </div>

        <Alert variant="warning" title={t('common.warning')}>
          {t('settings.rotationWarning')}
        </Alert>

        {error && (
          <Alert variant="error" title={t('common.error')}>
            {error}
          </Alert>
        )}

        {inProgress && (
          <div className="flex items-center gap-2 text-sm text-[var(--text-secondary)]">
            <Loader2 className="w-4 h-4 animate-spin" />
            {t('settings.rotationInProgress')}
          </div>
        )}
      </div>
    </Modal>
  );
}
