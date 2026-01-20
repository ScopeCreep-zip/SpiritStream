import { useState, useEffect, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { Modal, ModalBody, ModalFooter } from '@/components/ui/Modal';
import { Input } from '@/components/ui/Input';
import { Button } from '@/components/ui/Button';
import {
  Folder,
  File,
  ChevronUp,
  Home,
  RefreshCw,
  FolderOpen,
} from 'lucide-react';
import { getBackendBaseUrl, getAuthHeaders } from '@/lib/backend/env';

interface FileEntry {
  name: string;
  type: 'file' | 'directory';
  size?: number;
}

interface BrowseResponse {
  path: string;
  entries: FileEntry[];
}

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

// Map server error messages to user-friendly messages
type TFunction = (key: string, defaultValue: string) => string;

function getFriendlyError(serverError: string, t: TFunction): string {
  if (serverError.includes('Access to this directory is not allowed')) {
    return t(
      'fileBrowser.accessDenied',
      'This location is outside the allowed browsing area. You can browse your home directory and common system folders.'
    );
  }
  if (serverError.includes('Directory not found')) {
    return t('fileBrowser.directoryNotFound', 'Directory not found.');
  }
  if (serverError.includes('Path is not a directory')) {
    return t('fileBrowser.notADirectory', 'The selected path is not a directory.');
  }
  if (serverError.includes('Failed to read directory')) {
    return t('fileBrowser.readError', 'Unable to read directory contents. Check permissions.');
  }
  // Return original error if no mapping found
  return serverError;
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
  const [currentPath, setCurrentPath] = useState('');
  const [entries, setEntries] = useState<FileEntry[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [selectedEntry, setSelectedEntry] = useState<string | null>(null);
  const [fileName, setFileName] = useState(defaultFileName || '');

  // Get default title based on mode
  const modalTitle =
    title ||
    (mode === 'directory'
      ? t('fileBrowser.selectDirectory', 'Select Directory')
      : mode === 'save'
        ? t('fileBrowser.saveFile', 'Save File')
        : t('fileBrowser.selectFile', 'Select File'));

  // Filter entries based on mode and filters
  const filteredEntries = entries.filter((entry) => {
    // Always show directories
    if (entry.type === 'directory') return true;

    // Show files in directory mode too (so users can see directory contents)
    // They just can't select files - only the current directory is selected

    // Apply extension filters
    if (filters && filters.length > 0) {
      // Check if any filter allows all files (wildcard)
      const hasWildcard = filters.some((f) =>
        f.extensions.some((e) => e === '*' || e === '.*')
      );
      if (hasWildcard) return true;

      // Get file extension (empty string if no extension)
      const lastDot = entry.name.lastIndexOf('.');
      const ext = lastDot > 0 ? entry.name.slice(lastDot + 1).toLowerCase() : '';

      return filters.some((f) =>
        f.extensions.some((e) => {
          const filterExt = e.toLowerCase().replace(/^\./, '');
          return filterExt === ext;
        })
      );
    }

    return true;
  });

  // Fetch directory contents
  const browse = useCallback(
    async (path: string) => {
      setLoading(true);
      setError(null);
      setSelectedEntry(null);

      try {
        const baseUrl = getBackendBaseUrl();
        const params = new URLSearchParams();
        if (path) params.set('path', path);

        const response = await fetch(
          `${baseUrl}/api/files/browse?${params.toString()}`,
          {
            method: 'GET',
            headers: getAuthHeaders(),
            credentials: 'include',
          }
        );

        if (!response.ok) {
          const text = await response.text();
          throw new Error(text || `HTTP ${response.status}`);
        }

        const json = await response.json();
        if (!json.ok) {
          throw new Error(json.error || 'Unknown error');
        }
        const data: BrowseResponse = json.data;
        setCurrentPath(data.path);
        setEntries(data.entries);
      } catch (err) {
        console.error('[FileBrowser] Browse failed:', err);
        // Map server errors to user-friendly messages
        const errorMessage = err instanceof Error ? err.message : 'Failed to browse directory';
        const friendlyError = getFriendlyError(errorMessage, t);
        setError(friendlyError);
      } finally {
        setLoading(false);
      }
    },
    []
  );

  // Get initial directory on mount
  useEffect(() => {
    if (open && !currentPath) {
      // If initialPath is provided, navigate to it
      if (initialPath) {
        if (mode === 'directory') {
          // For directory mode, browse directly to the specified directory
          browse(initialPath);
        } else {
          // For file/save mode, browse to the parent directory
          const lastSlash = initialPath.lastIndexOf('/');
          const dirPath = lastSlash > 0 ? initialPath.substring(0, lastSlash) : '/';
          browse(dirPath);
        }
        return;
      }

      // Otherwise fetch home directory
      const fetchHome = async () => {
        try {
          const baseUrl = getBackendBaseUrl();
          const response = await fetch(`${baseUrl}/api/files/home`, {
            method: 'GET',
            headers: getAuthHeaders(),
            credentials: 'include',
          });

          if (response.ok) {
            const json = await response.json();
            if (json.ok && json.data?.path) {
              browse(json.data.path);
            } else {
              browse('');
            }
          } else {
            browse('');
          }
        } catch {
          browse('');
        }
      };
      fetchHome();
    }
  }, [open, currentPath, browse, initialPath]);

  // Reset state when modal closes
  useEffect(() => {
    if (!open) {
      setCurrentPath('');
      setEntries([]);
      setSelectedEntry(null);
      setError(null);
      setFileName(defaultFileName || '');
    }
  }, [open, defaultFileName]);

  // Navigate to parent directory
  const goUp = () => {
    // Handle both absolute (/path/to/dir) and relative (data/profiles) paths
    const isAbsolute = currentPath.startsWith('/');
    const parts = currentPath.split('/').filter(Boolean);

    if (parts.length > 1) {
      parts.pop();
      const newPath = isAbsolute ? '/' + parts.join('/') : parts.join('/');
      browse(newPath);
    } else if (parts.length === 1 && isAbsolute) {
      // At root level of absolute path
      browse('/');
    }
    // For relative paths with only one part (e.g., "data"), can't go higher
  };

  // Navigate to home directory
  const goHome = async () => {
    try {
      const baseUrl = getBackendBaseUrl();
      const response = await fetch(`${baseUrl}/api/files/home`, {
        method: 'GET',
        headers: getAuthHeaders(),
        credentials: 'include',
      });

      if (response.ok) {
        const json = await response.json();
        if (json.ok && json.data?.path) {
          browse(json.data.path);
        }
      }
    } catch {
      // Ignore
    }
  };

  // Handle entry click
  const handleEntryClick = (entry: FileEntry) => {
    if (entry.type === 'directory') {
      // Navigate into directory
      const newPath = currentPath.endsWith('/')
        ? `${currentPath}${entry.name}`
        : `${currentPath}/${entry.name}`;
      browse(newPath);
    } else if (mode !== 'directory') {
      // Select file (not allowed in directory mode)
      setSelectedEntry(entry.name);
      if (mode === 'save') {
        setFileName(entry.name);
      }
    }
  };

  // Handle entry double-click
  const handleEntryDoubleClick = (entry: FileEntry) => {
    if (entry.type === 'directory') {
      // Already navigated on single click
      return;
    }
    // Confirm selection on double-click
    handleConfirm();
  };

  // Handle selection confirmation
  const handleConfirm = () => {
    if (mode === 'directory') {
      onSelect(currentPath);
    } else if (mode === 'save') {
      if (!fileName.trim()) {
        setError(t('fileBrowser.fileNameRequired', 'File name is required'));
        return;
      }
      const fullPath = currentPath.endsWith('/')
        ? `${currentPath}${fileName}`
        : `${currentPath}/${fileName}`;
      onSelect(fullPath);
    } else {
      if (!selectedEntry) {
        setError(t('fileBrowser.selectFileFirst', 'Please select a file'));
        return;
      }
      const fullPath = currentPath.endsWith('/')
        ? `${currentPath}${selectedEntry}`
        : `${currentPath}/${selectedEntry}`;
      onSelect(fullPath);
    }
    onClose();
  };

  // Handle cancel
  const handleCancel = () => {
    onSelect(null);
    onClose();
  };

  // Format file size
  const formatSize = (bytes?: number): string => {
    if (bytes === undefined) return '';
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  };

  return (
    <Modal open={open} onClose={handleCancel} title={modalTitle}>
      <ModalBody>
        {/* Toolbar */}
        <div className="flex items-center gap-2 mb-3">
          <Button
            variant="ghost"
            size="sm"
            onClick={goUp}
            disabled={loading || currentPath === '/'}
            title={t('fileBrowser.goUp', 'Go up')}
          >
            <ChevronUp className="w-4 h-4" />
          </Button>
          <Button
            variant="ghost"
            size="sm"
            onClick={goHome}
            disabled={loading}
            title={t('fileBrowser.goHome', 'Go home')}
          >
            <Home className="w-4 h-4" />
          </Button>
          <Button
            variant="ghost"
            size="sm"
            onClick={() => browse(currentPath)}
            disabled={loading}
            title={t('fileBrowser.refresh', 'Refresh')}
          >
            <RefreshCw className={`w-4 h-4 ${loading ? 'animate-spin' : ''}`} />
          </Button>
          <div className="flex-1 px-3 py-1.5 bg-[var(--bg-sunken)] rounded text-sm font-mono text-[var(--text-secondary)] truncate">
            {currentPath || '/'}
          </div>
        </div>

        {/* File list */}
        <div className="border border-[var(--border-default)] rounded-lg bg-[var(--bg-sunken)] h-[300px] overflow-y-auto">
          {loading ? (
            <div className="flex items-center justify-center h-full text-[var(--text-tertiary)]">
              {t('common.loading', 'Loading...')}
            </div>
          ) : error ? (
            <div className="flex flex-col items-center justify-center h-full p-4 text-center">
              <p className="text-[var(--error-text)] mb-2">{error}</p>
              <Button variant="outline" size="sm" onClick={() => browse(currentPath)}>
                {t('common.retry', 'Retry')}
              </Button>
            </div>
          ) : filteredEntries.length === 0 ? (
            <div className="flex items-center justify-center h-full text-[var(--text-tertiary)]">
              {mode === 'directory'
                ? t('fileBrowser.noSubdirectories', 'No subdirectories')
                : t('fileBrowser.noFiles', 'No matching files')}
            </div>
          ) : (
            <div className="divide-y divide-[var(--border-muted)]">
              {filteredEntries.map((entry) => (
                <div
                  key={entry.name}
                  onClick={() => handleEntryClick(entry)}
                  onDoubleClick={() => handleEntryDoubleClick(entry)}
                  className={`
                    flex items-center gap-3 px-3 py-2 cursor-pointer transition-colors
                    ${
                      selectedEntry === entry.name
                        ? 'bg-[var(--primary-subtle)]'
                        : 'hover:bg-[var(--bg-hover)]'
                    }
                  `}
                >
                  {entry.type === 'directory' ? (
                    <FolderOpen className="w-5 h-5 text-[var(--warning)]" />
                  ) : (
                    <File className="w-5 h-5 text-[var(--text-tertiary)]" />
                  )}
                  <span className="flex-1 text-sm text-[var(--text-primary)] truncate">
                    {entry.name}
                  </span>
                  {entry.type === 'file' && entry.size !== undefined && (
                    <span className="text-xs text-[var(--text-muted)]">
                      {formatSize(entry.size)}
                    </span>
                  )}
                  {entry.type === 'directory' && (
                    <Folder className="w-4 h-4 text-[var(--text-muted)]" />
                  )}
                </div>
              ))}
            </div>
          )}
        </div>

        {/* File name input for save mode */}
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

        {/* Current selection info */}
        {mode === 'directory' && currentPath && (
          <div className="mt-3 p-2 bg-[var(--bg-muted)] rounded text-sm">
            <span className="text-[var(--text-tertiary)]">
              {t('fileBrowser.selectedDirectory', 'Selected directory:')}
            </span>{' '}
            <span className="font-mono text-[var(--text-secondary)]">{currentPath}</span>
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
          {mode === 'save'
            ? t('common.save', 'Save')
            : t('common.select', 'Select')}
        </Button>
      </ModalFooter>
    </Modal>
  );
}
