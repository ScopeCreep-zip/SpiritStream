import { useState, useEffect, useCallback, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { Modal, ModalBody, ModalFooter } from '@/components/ui/Modal';
import { Input } from '@/components/ui/Input';
import { Button } from '@/components/ui/Button';
import { api } from '@/lib/client';
import { logger } from '@/lib/logger';
import type { FileEntry } from '@spiritstream/api-client';
import {
  detectPlatform,
  getFriendlyError,
  getInitialBrowsePath,
  joinPath,
  quickPathsFor,
} from './file-browser/platform';
import { DirectoryTree } from './file-browser/DirectoryTree';
import { PathBar } from './file-browser/PathBar';

export interface FileBrowserModalProps {
  open: boolean;
  onClose: () => void;
  onSelect: (path: string | null) => void;
  mode: 'file' | 'directory' | 'save';
  title?: string;
  filters?: { name: string; extensions: string[] }[];
  defaultFileName?: string;
  initialPath?: string;
}

/**
 * File browser modal for HTTP mode.
 * Allows browsing server-side file system within allowed directories.
 */
export function FileBrowserModal({
  open,
  onClose,
  onSelect,
  mode,
  title,
  filters,
  defaultFileName,
  initialPath,
}: FileBrowserModalProps) {
  const { t } = useTranslation();
  const platform = useMemo(() => detectPlatform(), []);
  const quickPaths = useMemo(() => quickPathsFor(platform), [platform]);
  const [currentPath, setCurrentPath] = useState('');
  const [pathInput, setPathInput] = useState('');
  const [isEditingPath, setIsEditingPath] = useState(false);
  const [entries, setEntries] = useState<FileEntry[]>([]);
  const [parentPath, setParentPath] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [selectedEntry, setSelectedEntry] = useState<string | null>(null);
  const [fileName, setFileName] = useState(defaultFileName || '');

  const defaultTitleByMode = (() => {
    if (mode === 'directory') return t('fileBrowser.selectDirectory', 'Select Directory');
    if (mode === 'save') return t('fileBrowser.saveFile', 'Save File');
    return t('fileBrowser.selectFile', 'Select File');
  })();
  const modalTitle = title || defaultTitleByMode;

  const filteredEntries = entries.filter((entry) => {
    if (entry.type === 'directory') return true;

    if (filters && filters.length > 0) {
      const hasWildcard = filters.some((f) =>
        f.extensions.some((e) => e === '*' || e === '.*'),
      );
      if (hasWildcard) return true;

      const lastDot = entry.name.lastIndexOf('.');
      const ext = lastDot > 0 ? entry.name.slice(lastDot + 1).toLowerCase() : '';

      return filters.some((f) =>
        f.extensions.some((e) => {
          const filterExt = e.toLowerCase().replace(/^\./, '');
          return filterExt === ext;
        }),
      );
    }

    return true;
  });

  const browse = useCallback(
    async (path: string) => {
      setLoading(true);
      setError(null);
      setSelectedEntry(null);

      try {
        const data = await api.files.browse(path);
        setCurrentPath(data.path);
        setPathInput(data.path);
        setIsEditingPath(false);
        setEntries(data.entries);
        setParentPath(data.parent ?? null);
      } catch (err) {
        logger.error('[FileBrowser] Browse failed:', err);
        const errorMessage = err instanceof Error ? err.message : 'Failed to browse directory';
        setError(getFriendlyError(errorMessage, t));
      } finally {
        setLoading(false);
      }
    },
    [t],
  );

  useEffect(() => {
    if (open && !currentPath) {
      if (initialPath) {
        if (mode === 'directory') {
          browse(initialPath);
        } else {
          const dirPath = getInitialBrowsePath(initialPath);
          browse(dirPath);
        }
        return;
      }

      const fetchHome = async () => {
        try {
          const { path: homePath } = await api.files.home();
          browse(homePath || '');
        } catch {
          browse('');
        }
      };
      fetchHome();
    }
  }, [open, currentPath, browse, initialPath, mode]);

  useEffect(() => {
    if (!open) {
      setCurrentPath('');
      setPathInput('');
      setIsEditingPath(false);
      setEntries([]);
      setSelectedEntry(null);
      setParentPath(null);
      setError(null);
      setFileName(defaultFileName || '');
    }
  }, [open, defaultFileName]);

  const goUp = () => {
    if (parentPath) {
      browse(parentPath);
    }
  };

  const goHome = async () => {
    try {
      const { path: homePath } = await api.files.home();
      if (homePath) browse(homePath);
    } catch {
      // Ignore
    }
  };

  const goToPath = () => {
    if (pathInput.trim()) {
      browse(pathInput.trim());
    }
  };

  const handlePathKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'Enter') {
      goToPath();
    } else if (e.key === 'Escape') {
      setPathInput(currentPath);
      setIsEditingPath(false);
    }
  };

  const handleEntryClick = (entry: FileEntry) => {
    if (entry.type === 'directory') {
      browse(joinPath(currentPath, entry.name));
    } else if (mode !== 'directory') {
      setSelectedEntry(entry.name);
      if (mode === 'save') {
        setFileName(entry.name);
      }
    }
  };

  const handleEntryDoubleClick = (entry: FileEntry) => {
    if (entry.type === 'directory') {
      return;
    }
    handleConfirm();
  };

  const handleConfirm = () => {
    if (mode === 'directory') {
      onSelect(currentPath);
    } else if (mode === 'save') {
      if (!fileName.trim()) {
        setError(t('fileBrowser.fileNameRequired', 'File name is required'));
        return;
      }
      const fullPath = joinPath(currentPath, fileName);
      onSelect(fullPath);
    } else {
      if (!selectedEntry) {
        setError(t('fileBrowser.selectFileFirst', 'Please select a file'));
        return;
      }
      const fullPath = joinPath(currentPath, selectedEntry);
      onSelect(fullPath);
    }
    onClose();
  };

  const handleCancel = () => {
    onSelect(null);
    onClose();
  };

  return (
    <Modal open={open} onClose={handleCancel} title={modalTitle}>
      <ModalBody>
        <PathBar
          pathInput={pathInput}
          currentPath={currentPath}
          isEditingPath={isEditingPath}
          loading={loading}
          parentPath={parentPath}
          onPathInputChange={(value) => {
            setPathInput(value);
            setIsEditingPath(true);
          }}
          onPathInputFocus={() => setIsEditingPath(true)}
          onPathKeyDown={handlePathKeyDown}
          onGoUp={goUp}
          onGoHome={goHome}
          onRefresh={() => browse(currentPath)}
          onGoToPath={goToPath}
          labels={{
            goUp: t('fileBrowser.goUp', 'Go up'),
            goHome: t('fileBrowser.goHome', 'Go home'),
            refresh: t('fileBrowser.refresh', 'Refresh'),
            typePath: t('fileBrowser.typePath', 'Type a path and press Enter...'),
            goToPath: t('fileBrowser.goToPath', 'Go to path (Enter)'),
          }}
        />

        {quickPaths.length > 0 && (
          <div className="flex items-center gap-2 mb-3 flex-wrap">
            <span className="text-xs text-text-muted">
              {t('fileBrowser.quickPaths', 'Quick paths:')}
            </span>
            {quickPaths.map((quickPath) => (
              <button
                key={quickPath.path}
                type="button"
                onClick={() => browse(quickPath.path)}
                className="text-xs px-2 py-0.5 rounded bg-bg-muted hover:bg-bg-hover text-text-secondary transition-colors"
              >
                {quickPath.label}
              </button>
            ))}
          </div>
        )}

        <div className="border border-border-default rounded-lg bg-bg-sunken h-[300px] overflow-y-auto">
          <DirectoryTree
            loading={loading}
            error={error}
            entries={filteredEntries}
            selectedEntry={selectedEntry}
            onEntryClick={handleEntryClick}
            onEntryDoubleClick={handleEntryDoubleClick}
            onRetry={() => browse(currentPath)}
            loadingLabel={t('common.loading', 'Loading...')}
            emptyLabel={
              mode === 'directory'
                ? t('fileBrowser.noSubdirectories', 'No subdirectories')
                : t('fileBrowser.noFiles', 'No matching files')
            }
            retryLabel={t('common.retry', 'Retry')}
          />
        </div>

        {mode === 'save' && (
          <div className="mt-3">
            <Input
              label={t('fileBrowser.fileName', 'File name')}
              value={fileName}
              onChange={(e) => setFileName(e.target.value)}
              placeholder={t('fileBrowser.enterFileName', 'Enter file name')}
            />
          </div>
        )}

        {mode === 'directory' && currentPath && (
          <div className="mt-3 p-2 bg-bg-muted rounded text-sm">
            <span className="text-text-tertiary">
              {t('fileBrowser.selectedDirectory', 'Selected directory:')}
            </span>{' '}
            <span className="font-mono text-text-secondary">{currentPath}</span>
          </div>
        )}
      </ModalBody>

      <ModalFooter>
        <Button variant="ghost" onClick={handleCancel}>
          {t('common.cancel', 'Cancel')}
        </Button>
        <Button
          variant="primary"
          onClick={handleConfirm}
          disabled={mode === 'file' && !selectedEntry}
        >
          {mode === 'save' ? t('common.save', 'Save') : t('common.select', 'Select')}
        </Button>
      </ModalFooter>
    </Modal>
  );
}
