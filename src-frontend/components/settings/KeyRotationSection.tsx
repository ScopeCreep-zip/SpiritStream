import { useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { RotateCw } from 'lucide-react';
import { Button } from '@/components/ui/Button';
import { KeyRotationModal } from '@/components/modals/KeyRotationModal';
import { toast } from '@/hooks/useToast';
import { api } from '@/lib/tauri';
import type { RotationReport } from '@/types/api';

interface KeyRotationSectionProps {
  encryptStreamKeys: boolean;
  disabled?: boolean;
}

const formatRotationTimestamp = (timestamp: string) => {
  const date = new Date(timestamp);
  if (Number.isNaN(date.getTime())) {
    return timestamp;
  }
  return date.toLocaleString();
};

export function KeyRotationSection({ encryptStreamKeys, disabled = false }: KeyRotationSectionProps) {
  const { t } = useTranslation();
  const [confirmOpen, setConfirmOpen] = useState(false);
  const [isRotating, setIsRotating] = useState(false);
  const [rotationError, setRotationError] = useState<string | null>(null);
  const [lastRotated, setLastRotated] = useState<string | null>(null);

  const lastRotatedLabel = useMemo(() => {
    if (!lastRotated) {
      return t('settings.rotationNever');
    }
    return formatRotationTimestamp(lastRotated);
  }, [lastRotated, t]);

  const handleOpen = () => {
    setRotationError(null);
    setConfirmOpen(true);
  };

  const handleClose = () => {
    if (isRotating) return;
    setConfirmOpen(false);
    setRotationError(null);
  };

  const handleConfirm = async () => {
    setRotationError(null);
    setIsRotating(true);

    try {
      let activeCount = 0;
      try {
        activeCount = await api.stream.getActiveCount();
      } catch {
        const message = t('settings.rotationPreflightFailed');
        setRotationError(message);
        toast.error(message);
        return;
      }
      if (activeCount > 0) {
        const message = t('settings.rotationActiveStreams');
        setRotationError(message);
        toast.error(message);
        return;
      }

      const report: RotationReport = await api.settings.rotateMachineKey();
      setLastRotated(report.timestamp);
      toast.success(
        t('toast.keyRotationSuccess', {
          profiles: report.profilesUpdated,
          keys: report.keysReencrypted,
        })
      );
      setConfirmOpen(false);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      setRotationError(message);
      toast.error(t('toast.keyRotationFailed', { error: message }));
    } finally {
      setIsRotating(false);
    }
  };

  return (
    <>
      <div className="flex items-center justify-between" style={{ padding: '8px 0', gap: '12px' }}>
        <div>
          <div className="text-sm font-medium text-[var(--text-primary)]">
            {t('settings.machineKey')}
          </div>
          <div className="text-xs text-[var(--text-tertiary)]">
            {t('settings.lastRotated', { timestamp: lastRotatedLabel })}
          </div>
          {!encryptStreamKeys && (
            <div className="text-xs text-[var(--text-tertiary)]">
              {t('settings.rotationEncryptionOffHint')}
            </div>
          )}
        </div>
        <Button variant="outline" onClick={handleOpen} disabled={disabled || isRotating}>
          <RotateCw className="w-4 h-4" />
          {t('settings.rotateMachineKey')}
        </Button>
      </div>

      <KeyRotationModal
        open={confirmOpen}
        onClose={handleClose}
        onConfirm={handleConfirm}
        inProgress={isRotating}
        error={rotationError}
      />
    </>
  );
}
