import type { ReactElement } from 'react';
import { File, Folder, FolderOpen } from 'lucide-react';
import type { FileEntry } from '@spiritstream/api-client';
import { Button } from '@/components/ui/Button';
import { formatSize } from './platform';

interface DirectoryTreeProps {
  loading: boolean;
  error: string | null;
  entries: FileEntry[];
  selectedEntry: string | null;
  onEntryClick: (entry: FileEntry) => void;
  onEntryDoubleClick: (entry: FileEntry) => void;
  onRetry: () => void;
  loadingLabel: string;
  emptyLabel: string;
  retryLabel: string;
}

export function DirectoryTree({
  loading,
  error,
  entries,
  selectedEntry,
  onEntryClick,
  onEntryDoubleClick,
  onRetry,
  loadingLabel,
  emptyLabel,
  retryLabel,
}: DirectoryTreeProps): ReactElement {
  if (loading) {
    return (
      <div className="flex items-center justify-center h-full text-text-tertiary">
        {loadingLabel}
      </div>
    );
  }
  if (error) {
    return (
      <div className="flex flex-col items-center justify-center h-full p-4 text-center">
        <p className="text-error-text mb-2">{error}</p>
        <Button variant="outline" size="sm" onClick={onRetry}>
          {retryLabel}
        </Button>
      </div>
    );
  }
  if (entries.length === 0) {
    return (
      <div className="flex items-center justify-center h-full text-text-tertiary">{emptyLabel}</div>
    );
  }
  return (
    <div className="divide-y divide-border-muted">
      {entries.map((entry) => (
        <div
          key={entry.name}
          onClick={() => onEntryClick(entry)}
          onDoubleClick={() => onEntryDoubleClick(entry)}
          className={`
            flex items-center gap-3 px-3 py-2 cursor-pointer transition-colors
            ${selectedEntry === entry.name ? 'bg-primary-subtle' : 'hover:bg-bg-hover'}
          `}
        >
          {entry.type === 'directory' ? (
            <FolderOpen className="w-5 h-5 text-warning" />
          ) : (
            <File className="w-5 h-5 text-text-tertiary" />
          )}
          <span className="flex-1 text-sm text-text-primary truncate">{entry.name}</span>
          {entry.type === 'file' && entry.size != null && (
            <span className="text-xs text-text-muted">{formatSize(entry.size)}</span>
          )}
          {entry.type === 'directory' && <Folder className="w-4 h-4 text-text-muted" />}
        </div>
      ))}
    </div>
  );
}
