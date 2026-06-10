import { useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { CheckCircle2, Eye, EyeOff, Loader2 } from 'lucide-react';
import { Modal } from '@/components/ui/Modal';
import { Alert } from '@/components/ui/Alert';
import { Button } from '@/components/ui/Button';
import { PasswordInput } from '@spiritstream/ui';

export interface KeyRotationModalProps {
  open: boolean;
  onClose: () => void;
  onConfirm: (passwords: Record<string, string>) => void;
  /** Names of every encrypted (`.mgs`) profile on disk. Empty if none. */
  encryptedProfiles: readonly string[];
  inProgress?: boolean;
  error?: string | null;
}

export function KeyRotationModal({
  open,
  onClose,
  onConfirm,
  encryptedProfiles,
  inProgress = false,
  error,
}: KeyRotationModalProps) {
  const { t } = useTranslation();
  const [passwords, setPasswords] = useState<Record<string, string>>({});

  // Reset password fields whenever the modal re-opens so a previous
  // typing session can't leak into the next rotation.
  useEffect(() => {
    if (open) {
      setPasswords({});
    }
  }, [open]);

  const allPasswordsProvided = useMemo(
    () => encryptedProfiles.every((name) => (passwords[name] ?? '').length > 0),
    [encryptedProfiles, passwords]
  );

  const handleClose = () => {
    if (inProgress) return;
    onClose();
  };

  const handleConfirm = () => {
    onConfirm({ ...passwords });
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
          <Button
            onClick={handleConfirm}
            disabled={inProgress || !allPasswordsProvided}
            loading={inProgress}
          >
            {t('settings.rotateMachineKey')}
          </Button>
        </>
      }
    >
      <div className="flex flex-col gap-4">
        <p className="text-sm text-text-secondary">{t('settings.rotateMachineKeyDescription')}</p>

        <div className="flex flex-col gap-2.5">
          <div className="text-sm font-medium text-text-primary">
            {t('settings.rotationStepsTitle')}
          </div>
          <div className="flex items-start gap-2">
            <CheckCircle2 className="w-4 h-4 text-success-text mt-0.5" />
            <span className="text-sm text-text-secondary">
              {t('settings.rotationStepGenerate')}
            </span>
          </div>
          <div className="flex items-start gap-2">
            <CheckCircle2 className="w-4 h-4 text-success-text mt-0.5" />
            <span className="text-sm text-text-secondary">{t('settings.rotationStepBackup')}</span>
          </div>
          <div className="flex items-start gap-2">
            <CheckCircle2 className="w-4 h-4 text-success-text mt-0.5" />
            <span className="text-sm text-text-secondary">
              {t('settings.rotationStepReencrypt')}
            </span>
          </div>
          <div className="flex items-start gap-2">
            <CheckCircle2 className="w-4 h-4 text-success-text mt-0.5" />
            <span className="text-sm text-text-secondary">{t('settings.rotationStepDelete')}</span>
          </div>
        </div>

        {encryptedProfiles.length > 0 && (
          <div className="flex flex-col gap-2.5">
            <div className="text-sm font-medium text-text-primary">
              {t('settings.rotationPasswordsTitle')}
            </div>
            <p className="text-xs text-text-tertiary">{t('settings.rotationPasswordsHint')}</p>
            {encryptedProfiles.map((name) => (
              <PasswordInput
                key={name}
                label={name}
                value={passwords[name] ?? ''}
                onChange={(e) => setPasswords((prev) => ({ ...prev, [name]: e.target.value }))}
                disabled={inProgress}
                placeholder={t('settings.rotationPasswordPlaceholder')}
                autoComplete="off"
                renderToggleIcon={(visible) =>
                  visible ? <EyeOff className="w-4 h-4" /> : <Eye className="w-4 h-4" />
                }
              />
            ))}
          </div>
        )}

        <Alert variant="warning" title={t('common.warning')}>
          {t('settings.rotationWarning')}
        </Alert>

        {error && (
          <Alert variant="error" title={t('common.error')}>
            {error}
          </Alert>
        )}

        {inProgress && (
          <div className="flex items-center gap-2 text-sm text-text-secondary">
            <Loader2 className="w-4 h-4 animate-spin" />
            {t('settings.rotationInProgress')}
          </div>
        )}
      </div>
    </Modal>
  );
}
