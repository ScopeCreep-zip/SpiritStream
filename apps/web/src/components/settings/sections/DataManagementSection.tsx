import { useCallback, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { FolderOpen, Download, Trash2 } from 'lucide-react';
import { useQueryClient } from '@tanstack/react-query';
import { Card, CardHeader, CardTitle, CardDescription, CardBody } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { Input } from '@/components/ui/Input';
import { FormGroup, FormLabel } from '@/components/ui/Form';
import { ConfirmDialog } from '@spiritstream/ui';
import { useSettings, SETTINGS_QUERY_KEY } from '@/hooks/useSettings';
import { useFileBrowser } from '@/hooks/useFileBrowser';
import { api } from '@/lib/client';
import { logger } from '@/lib/logger';

/**
 * Profile storage path display, data export, and clear-all-data
 * (gated by a confirm-token dance handled inside api.settings.clearData).
 */
export function DataManagementSection() {
  const { t } = useTranslation();
  const { data: settings } = useSettings();
  const queryClient = useQueryClient();
  const { FileBrowser, openDirectoryPath: browserOpenDirectory } = useFileBrowser();

  const [clearConfirmOpen, setClearConfirmOpen] = useState(false);
  const [clearInProgress, setClearInProgress] = useState(false);
  const [clearError, setClearError] = useState<string | null>(null);

  const handleOpenProfileStorage = useCallback(async () => {
    try {
      await browserOpenDirectory({
        title: t('settings.profileStorage'),
        initialPath: settings?.profileStoragePath,
      });
    } catch (error) {
      logger.error('Failed to open profile storage:', error);
    }
  }, [browserOpenDirectory, settings, t]);

  const handleExportData = useCallback(async () => {
    try {
      const selected = await browserOpenDirectory({
        title: t('settings.selectExportLocation'),
        initialPath: settings?.profileStoragePath || undefined,
      });
      if (!selected) return;
      await api.settings.exportData(selected);
      alert(t('toast.dataExported'));
    } catch (error) {
      logger.error('Failed to export data:', error);
      alert(`${t('settings.exportFailed')}: ${error}`);
    }
  }, [browserOpenDirectory, settings, t]);

  const handleClearAllData = (): void => {
    setClearError(null);
    setClearConfirmOpen(true);
  };

  const handleClearCancel = (): void => {
    if (clearInProgress) return;
    setClearConfirmOpen(false);
    setClearError(null);
  };

  const handleClearConfirm = async (): Promise<void> => {
    setClearInProgress(true);
    setClearError(null);
    try {
      await api.settings.clearData();
      alert(t('toast.dataCleared'));
      queryClient.invalidateQueries({ queryKey: SETTINGS_QUERY_KEY });
      setClearConfirmOpen(false);
    } catch (error) {
      logger.error('Failed to clear data:', error);
      setClearError(`${t('settings.clearFailed')}: ${error}`);
    } finally {
      setClearInProgress(false);
    }
  };

  if (!settings) return null;

  return (
    <>
      <Card>
        <FileBrowser />
        <CardHeader>
          <div>
            <CardTitle>
              {t('settings.dataManagement', { defaultValue: 'Data Management' })}
            </CardTitle>
            <CardDescription>
              {t('settings.dataManagementDescription', {
                defaultValue: 'Export and manage your application data.',
              })}
            </CardDescription>
          </div>
        </CardHeader>
        <CardBody className="p-6 flex flex-col gap-4">
          <FormGroup>
            <FormLabel>{t('settings.profileStorage')}</FormLabel>
            <div className="flex gap-2">
              <Input
                value={settings.profileStoragePath}
                disabled
                className="flex-1 font-mono text-xs"
              />
              <Button variant="outline" onClick={handleOpenProfileStorage}>
                <FolderOpen className="w-4 h-4" />
                {t('settings.open')}
              </Button>
            </div>
          </FormGroup>
          <div className="flex gap-3">
            <Button variant="outline" onClick={handleExportData}>
              <Download className="w-4 h-4" />
              {t('settings.exportData')}
            </Button>
            <Button variant="destructive" onClick={handleClearAllData}>
              <Trash2 className="w-4 h-4" />
              {t('settings.clearAllData')}
            </Button>
          </div>
        </CardBody>
      </Card>

      {/* Clear Data — ConfirmDialog with confirm-token flow handled by api.settings.clearData. */}
      <ConfirmDialog
        open={clearConfirmOpen}
        title={t('settings.clearAllData')}
        confirmLabel={clearInProgress ? t('common.loading') : t('common.confirm')}
        cancelLabel={t('common.cancel')}
        confirmDisabled={clearInProgress}
        onConfirm={handleClearConfirm}
        onCancel={handleClearCancel}
        message={
          <>
            <p>{t('settings.clearConfirm')}</p>
            {clearError && (
              <div className="mt-4 p-3 rounded-lg bg-error-subtle border border-error-border">
                <p className="text-sm text-error-text">{clearError}</p>
              </div>
            )}
          </>
        }
      />
    </>
  );
}
