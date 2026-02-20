import { memo } from 'react';
import { useTranslation } from 'react-i18next';
import { Plus, FolderOpen } from 'lucide-react';
import { Button } from '@/components/ui/Button';

interface SourcesPanelToolbarProps {
  hasActiveScene: boolean;
  selectedLayerCount: number;
  isMultiSelectMode: boolean;
  onAddSource: () => void;
  onCreateGroup: () => void;
  onClearSelection: () => void;
}

export const SourcesPanelToolbar = memo(function SourcesPanelToolbar({
  hasActiveScene,
  selectedLayerCount,
  isMultiSelectMode,
  onAddSource,
  onCreateGroup,
  onClearSelection,
}: SourcesPanelToolbarProps) {
  const { t } = useTranslation();

  return (
    <div>
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-1">
          {selectedLayerCount >= 2 && (
            <Button
              variant="ghost"
              size="sm"
              className="text-xs px-2 py-1 h-auto"
              onClick={onCreateGroup}
              title={t('stream.groupSelected', { defaultValue: 'Group selected layers' })}
            >
              <FolderOpen className="w-3.5 h-3.5 mr-1" />
              {t('stream.group', { defaultValue: 'Group' })} ({selectedLayerCount})
            </Button>
          )}
          {selectedLayerCount > 0 && (
            <Button
              variant="ghost"
              size="sm"
              className="text-xs px-2 py-1 h-auto"
              onClick={onClearSelection}
              title={t('common.clearSelection', { defaultValue: 'Clear selection' })}
            >
              ×
            </Button>
          )}
        </div>
        <Button
          variant="ghost"
          size="sm"
          className="min-w-[36px] min-h-[36px]"
          onClick={onAddSource}
          disabled={!hasActiveScene}
          title={t('stream.addSource', { defaultValue: 'Add Source' })}
        >
          <Plus className="w-4 h-4" />
        </Button>
      </div>
      {isMultiSelectMode && (
        <p className="text-xs text-[var(--text-muted)] mt-1">
          {t('stream.multiSelectMode', { defaultValue: 'Click layers to select multiple' })}
        </p>
      )}
    </div>
  );
});
