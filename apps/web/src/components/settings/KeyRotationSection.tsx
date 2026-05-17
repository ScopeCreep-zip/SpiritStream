import { useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { RotateCw } from 'lucide-react';
import { useQueryClient } from '@tanstack/react-query';
import { Button } from '@/components/ui/Button';
import { KeyRotationModal } from '@/components/modals/KeyRotationModal';
import { toast } from '@/hooks/useToast';
import { SETTINGS_QUERY_KEY } from '@/hooks/useSettings';
import { api } from '@/lib/client';
import { formatDateTime } from '@/lib/locale';
import { useProfileStore } from '@/stores/profileStore';
import type { RotationReport } from '@spiritstream/types';

interface KeyRotationSectionProps {
  encryptStreamKeys: boolean;
  disabled?: boolean;
}

// Delegate to the locale-aware helper so the timestamp
// honours the user's active language for date / time separators.
const formatRotationTimestamp = (timestamp: string) => {
  return formatDateTime(timestamp) || timestamp;
};

export function KeyRotationSection({ encryptStreamKeys, disabled = false }: KeyRotationSectionProps) {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const [confirmOpen, setConfirmOpen] = useState(false);
  const [isRotating, setIsRotating] = useState(false);
  const [rotationError, setRotationError] = useState<string | null>(null);
  const [lastRotated, setLastRotated] = useState<string | null>(null);
  const [encryptedProfiles, setEncryptedProfiles] = useState<string[]>([]);

  const lastRotatedLabel = useMemo(() => {
    if (!lastRotated) {
      return t('settings.rotationNever');
    }
    return formatRotationTimestamp(lastRotated);
  }, [lastRotated, t]);

  const handleOpen = async () => {
    setRotationError(null);
    try {
      const summaries = await api.profile.getSummaries();
      setEncryptedProfiles(
        summaries.filter((s) => s.isEncrypted).map((s) => s.name).sort(),
      );
    } catch {
      // Best-effort pre-flight; rotation will still refuse server-side if a
      // password is missing, surfacing the structured error in the modal.
      setEncryptedProfiles([]);
    }
    setConfirmOpen(true);
  };

  const handleClose = () => {
    if (isRotating) return;
    setConfirmOpen(false);
    setRotationError(null);
  };

  const handleConfirm = async (passwords: Record<string, string>) => {
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

      const report: RotationReport = await api.settings.rotateMachineKey(passwords);
      setLastRotated(report.timestamp);
      toast.success(
        t('toast.keyRotationSuccess', {
          profiles: report.profilesUpdated,
          keys: report.keysReencrypted,
        })
      );
      // Settings depends on server-decrypted state; profile catalog needs to
      // re-fetch because the active profile cache was invalidated server-side.
      queryClient.invalidateQueries({ queryKey: SETTINGS_QUERY_KEY });
      void useProfileStore.getState().loadProfiles();
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
      <div className="flex items-center justify-between py-2 gap-3">
        <div>
          <div className="text-sm font-medium text-text-primary">
            {t('settings.machineKey')}
          </div>
          <div className="text-xs text-text-tertiary">
            {t('settings.lastRotated', { timestamp: lastRotatedLabel })}
          </div>
          {!encryptStreamKeys && (
            <div className="text-xs text-text-tertiary">
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
        encryptedProfiles={encryptedProfiles}
        inProgress={isRotating}
        error={rotationError}
      />
    </>
  );
}
