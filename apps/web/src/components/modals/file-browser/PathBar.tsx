import { ChevronUp, CornerDownLeft, Home, RefreshCw } from 'lucide-react';
import type { KeyboardEvent, ReactElement } from 'react';
import { Button } from '@/components/ui/Button';

interface PathBarProps {
  pathInput: string;
  currentPath: string;
  isEditingPath: boolean;
  loading: boolean;
  parentPath: string | null;
  onPathInputChange: (value: string) => void;
  onPathInputFocus: () => void;
  onPathKeyDown: (e: KeyboardEvent<HTMLInputElement>) => void;
  onGoUp: () => void;
  onGoHome: () => void;
  onRefresh: () => void;
  onGoToPath: () => void;
  labels: {
    goUp: string;
    goHome: string;
    refresh: string;
    typePath: string;
    goToPath: string;
  };
}

export function PathBar({
  pathInput,
  currentPath,
  isEditingPath,
  loading,
  parentPath,
  onPathInputChange,
  onPathInputFocus,
  onPathKeyDown,
  onGoUp,
  onGoHome,
  onRefresh,
  onGoToPath,
  labels,
}: PathBarProps): ReactElement {
  return (
    <div className="flex items-center gap-2 mb-3">
      <Button
        variant="ghost"
        size="sm"
        onClick={onGoUp}
        disabled={loading || !parentPath}
        title={labels.goUp}
      >
        <ChevronUp className="w-4 h-4" />
      </Button>
      <Button
        variant="ghost"
        size="sm"
        onClick={onGoHome}
        disabled={loading}
        title={labels.goHome}
      >
        <Home className="w-4 h-4" />
      </Button>
      <Button
        variant="ghost"
        size="sm"
        onClick={onRefresh}
        disabled={loading}
        title={labels.refresh}
      >
        <RefreshCw className={`w-4 h-4 ${loading ? 'animate-spin' : ''}`} />
      </Button>
      <input
        type="text"
        value={pathInput}
        onChange={(e) => onPathInputChange(e.target.value)}
        onKeyDown={onPathKeyDown}
        onFocus={onPathInputFocus}
        placeholder={labels.typePath}
        className="flex-1 px-3 py-1.5 bg-bg-sunken rounded text-sm font-mono text-text-secondary border border-transparent focus:border-primary focus:outline-none"
      />
      {isEditingPath && pathInput !== currentPath && (
        <Button
          variant="ghost"
          size="sm"
          onClick={onGoToPath}
          disabled={loading || !pathInput.trim()}
          title={labels.goToPath}
        >
          <CornerDownLeft className="w-4 h-4" />
        </Button>
      )}
    </div>
  );
}
