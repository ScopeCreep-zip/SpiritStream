import React, { memo } from 'react';
import { useTranslation } from 'react-i18next';
import { Maximize2, AppWindow } from 'lucide-react';
import { useProjectorStore } from '@/stores/projectorStore';
import type { Source } from '@/types/profile';
import type { SourceLayer } from '@/types/scene';

interface LayerContextMenuProps {
  menuRef: React.RefObject<HTMLDivElement | null>;
  position: { x: number; y: number };
  source: Source;
  layer: SourceLayer;
  profileName: string;
  isGrouped: boolean;
  onToggleVisibility: () => void;
  onSetHotkey: () => void;
  onRemoveFromGroup?: () => void;
  onRemoveSource: () => void;
}

export const LayerContextMenu = memo(function LayerContextMenu({
  menuRef,
  position,
  source,
  layer,
  profileName,
  isGrouped,
  onToggleVisibility,
  onSetHotkey,
  onRemoveFromGroup,
  onRemoveSource,
}: LayerContextMenuProps) {
  const { t } = useTranslation();
  const { openProjector } = useProjectorStore();

  return (
    <div
      ref={menuRef}
      className="fixed z-50 bg-[var(--bg-elevated)] border border-[var(--border-default)] rounded-lg shadow-lg py-1 min-w-[200px]"
      style={{ left: position.x, top: position.y }}
      role="menu"
      aria-label={t('stream.layerContextMenu', { defaultValue: 'Layer options' })}
    >
      {/* Projector options */}
      <button
        type="button"
        role="menuitem"
        className="w-full px-3 py-1.5 text-left text-sm hover:bg-[var(--bg-hover)] text-[var(--text-secondary)] flex items-center gap-2"
        onClick={() => {
          openProjector({
            type: 'source',
            displayMode: 'fullscreen',
            targetId: source.id,
            profileName,
            alwaysOnTop: true,
            hideCursor: true,
          });
        }}
      >
        <Maximize2 className="w-4 h-4 text-[var(--text-muted)]" />
        {t('projector.fullscreenSource', { defaultValue: 'Fullscreen Projector (Source)' })}
      </button>
      <button
        type="button"
        role="menuitem"
        className="w-full px-3 py-1.5 text-left text-sm hover:bg-[var(--bg-hover)] text-[var(--text-secondary)] flex items-center gap-2"
        onClick={() => {
          openProjector({
            type: 'source',
            displayMode: 'windowed',
            targetId: source.id,
            profileName,
            alwaysOnTop: false,
            hideCursor: false,
          });
        }}
      >
        <AppWindow className="w-4 h-4 text-[var(--text-muted)]" />
        {t('projector.windowedSource', { defaultValue: 'Windowed Projector (Source)' })}
      </button>
      <div className="h-px bg-[var(--border-default)] my-1" role="separator" />
      <button
        type="button"
        role="menuitem"
        className="w-full px-3 py-1.5 text-left text-sm hover:bg-[var(--bg-hover)] text-[var(--text-secondary)]"
        onClick={onToggleVisibility}
      >
        {layer.visible
          ? t('stream.hideLayer', { defaultValue: 'Hide Layer' })
          : t('stream.showLayer', { defaultValue: 'Show Layer' })}
      </button>
      <div className="h-px bg-[var(--border-default)] my-1" role="separator" />
      <button
        type="button"
        role="menuitem"
        className="w-full px-3 py-1.5 text-left text-sm hover:bg-[var(--bg-hover)] text-[var(--text-secondary)]"
        onClick={onSetHotkey}
      >
        {t('hotkeys.setVisibilityHotkey', { defaultValue: 'Set Visibility Hotkey...' })}
      </button>
      {isGrouped && onRemoveFromGroup && (
        <>
          <div className="h-px bg-[var(--border-default)] my-1" role="separator" />
          <button
            type="button"
            role="menuitem"
            className="w-full px-3 py-1.5 text-left text-sm hover:bg-[var(--bg-hover)] text-[var(--text-secondary)]"
            onClick={onRemoveFromGroup}
          >
            {t('stream.removeFromGroup', { defaultValue: 'Remove from Group' })}
          </button>
        </>
      )}
      <div className="h-px bg-[var(--border-default)] my-1" role="separator" />
      <button
        type="button"
        role="menuitem"
        className="w-full px-3 py-1.5 text-left text-sm hover:bg-destructive/10 text-destructive"
        onClick={onRemoveSource}
      >
        {t('stream.removeSource', { defaultValue: 'Remove Source' })}
      </button>
    </div>
  );
});
