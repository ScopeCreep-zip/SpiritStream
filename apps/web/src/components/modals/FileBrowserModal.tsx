import { useState, useEffect, useCallback, useMemo } from 'react';
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
  CornerDownLeft,
} from 'lucide-react';
import { getBackendBaseUrl, getAuthHeaders, safeFetch } from '@/lib/backend/env';

interface FileEntry {
  name: string;
  type: 'file' | 'directory';
  size?: number;
}

interface BrowseResponse {
  path: string;
  entries: FileEntry[];
  parent?: string | null;
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
type Platform = 'windows' | 'macos' | 'linux' | 'unknown';
type QuickPath = { label: string; path: string };

function detectPlatform(): Platform {
  if (typeof navigator === 'undefined') {
    return 'unknown';
  }

  const nav = navigator as Navigator & { userAgentData?: { platform?: string } };
  const platformHint =
    nav.userAgentData?.platform || nav.platform || nav.userAgent || '';

  if (/windows/i.test(platformHint)) return 'windows';
  if (/mac/i.test(platformHint)) return 'macos';
  if (/linux/i.test(platformHint)) return 'linux';
  return 'unknown';
}

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
  const platform = useMemo(() => detectPlatform(), []);
  const quickPaths = useMemo<QuickPath[]>(() => {
    if (platform === 'windows') {
      return [
        { label: 'Program Files', path: 'C:\\Program Files' },
        { label: 'Program Files (x86)', path: 'C:\\Program Files (x86)' },
      ];
    }

    if (platform === 'macos') {
      return [
        { label: '/usr/local/bin', path: '/usr/local/bin' },
        { label: '/opt/homebrew/bin', path: '/opt/homebrew/bin' },
      ];
    }

    if (platform === 'linux') {
      return [
        { label: '/usr/bin', path: '/usr/bin' },
        { label: '/usr/local/bin', path: '/usr/local/bin' },
        { label: '/opt', path: '/opt' },
      ];
    }

    return [];
  }, [platform]);
  const [currentPath, setCurrentPath] = useState('');
  const [pathInput, setPathInput] = useState('');
  const [isEditingPath, setIsEditingPath] = useState(false);
  const [entries, setEntries] = useState<FileEntry[]>([]);
  const [parentPath, setParentPath] = useState<string | null>(null);
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

        const response = await safeFetch(
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
        setPathInput(data.path);
        setIsEditingPath(false);
        setEntries(data.entries);
        setParentPath(data.parent ?? null);
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

  const getPathSeparator = (path: string) => (path.includes('\\') ? '\\' : '/');

  const joinPath = (base: string, entry: string) => {
    const separator = getPathSeparator(base);
    if (!base || base.endsWith(separator)) {
      return `${base}${entry}`;
    }
    return `${base}${separator}${entry}`;
  };

  const getInitialBrowsePath = (path: string) => {
    const trimmed = path.replace(/[\\/]+$/, '');
    const lastSlash = trimmed.lastIndexOf('/');
    const lastBackslash = trimmed.lastIndexOf('\\');
    const lastSep = Math.max(lastSlash, lastBackslash);
    if (lastSep > 0) {
      return trimmed.substring(0, lastSep);
    }

    const driveMatch = trimmed.match(/^[A-Za-z]:/);
    if (driveMatch) {
      return `${driveMatch[0]}\\`;
    }

    return '/';
  };

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
          const dirPath = getInitialBrowsePath(initialPath);
          browse(dirPath);
        }
        return;
      }

      // Otherwise fetch home directory
      const fetchHome = async () => {
        try {
          const baseUrl = getBackendBaseUrl();
          const response = await safeFetch(`${baseUrl}/api/files/home`, {
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
      setPathInput('');
      setIsEditingPath(false);
      setEntries([]);
      setSelectedEntry(null);
      setParentPath(null);
      setError(null);
      setFileName(defaultFileName || '');
    }
  }, [open, defaultFileName]);

  // Navigate to parent directory
  const goUp = () => {
    if (parentPath) {
      browse(parentPath);
    }
  };

  // Navigate to home directory
  const goHome = async () => {
    try {
      const baseUrl = getBackendBaseUrl();
      const response = await safeFetch(`${baseUrl}/api/files/home`, {
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

  // Navigate to typed path
  const goToPath = () => {
    if (pathInput.trim()) {
      browse(pathInput.trim());
    }
  };

  // Handle path input key press
  const handlePathKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'Enter') {
      goToPath();
    } else if (e.key === 'Escape') {
      setPathInput(currentPath);
      setIsEditingPath(false);
    }
  };

  // Handle entry click
  const handleEntryClick = (entry: FileEntry) => {
    if (entry.type === 'directory') {
      // Navigate into directory
      browse(joinPath(currentPath, entry.name));
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
            disabled={loading || !parentPath}
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
          <input
            type="text"
            value={pathInput}
            onChange={(e) => {
              setPathInput(e.target.value);
              setIsEditingPath(true);
            }}
            onKeyDown={handlePathKeyDown}
            onFocus={() => setIsEditingPath(true)}
            placeholder={t('fileBrowser.typePath', 'Type a path and press Enter...')}
            className="flex-1 px-3 py-1.5 bg-[var(--bg-sunken)] rounded text-sm font-mono text-[var(--text-secondary)] border border-transparent focus:border-[var(--primary)] focus:outline-none"
          />
          {isEditingPath && pathInput !== currentPath && (
            <Button
              variant="ghost"
              size="sm"
              onClick={goToPath}
              disabled={loading || !pathInput.trim()}
              title={t('fileBrowser.goToPath', 'Go to path (Enter)')}
            >
              <CornerDownLeft className="w-4 h-4" />
            </Button>
          )}
        </div>

        {/* Quick path shortcuts */}
        {quickPaths.length > 0 && (
          <div className="flex items-center gap-2 mb-3 flex-wrap">
            <span className="text-xs text-[var(--text-muted)]">
              {t('fileBrowser.quickPaths', 'Quick paths:')}
            </span>
            {quickPaths.map((quickPath) => (
              <button
                key={quickPath.path}
                type="button"
                onClick={() => browse(quickPath.path)}
                className="text-xs px-2 py-0.5 rounded bg-[var(--bg-muted)] hover:bg-[var(--bg-hover)] text-[var(--text-secondary)] transition-colors"
              >
                {quickPath.label}
              </button>
            ))}
          </div>
        )}

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
