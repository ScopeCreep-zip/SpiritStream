import React, { memo } from 'react';
import { useTranslation } from 'react-i18next';
import { useSortable } from '@dnd-kit/sortable';
import { CSS } from '@dnd-kit/utilities';
import { Eye, EyeOff, Trash2 } from 'lucide-react';
import { useContextMenu } from '@/hooks/useContextMenu';
import { useHotkeyStore } from '@/stores/hotkeyStore';
import { formatHotkeyBinding } from '@/types/hotkeys';
import { LayerContextMenu } from './LayerContextMenu';
import { SourceThumbnail } from './SourceThumbnail';
import { SourceIcon } from './SourceIcon';
import type { SourceLayer } from '@/types/scene';
import type { Source } from '@/types/profile';

export interface SortableLayerItemProps {
  layer: SourceLayer;
  source: Source | undefined;
  sceneId: string;
  profileName: string;
  isSelected?: boolean;
  isGrouped?: boolean;
  onToggleVisibility: (layerId: string, currentVisible: boolean) => void;
  onRemoveSource: (source: Source) => void;
  onSetHotkey: (layerId: string, layerName: string) => void;
  onClick?: (layerId: string, e: React.MouseEvent) => void;
  onRemoveFromGroup?: (layerId: string) => void;
}

export const SortableLayerItem = memo(function SortableLayerItem({
  layer,
  source,
  sceneId,
  profileName,
  isSelected = false,
  isGrouped = false,
  onToggleVisibility,
  onRemoveSource,
  onSetHotkey,
  onClick,
  onRemoveFromGroup,
}: SortableLayerItemProps) {
  const { t } = useTranslation();
  const { getLayerBinding } = useHotkeyStore();
  const { isOpen: showContextMenu, position: contextMenuPos, menuRef: contextMenuRef, openMenu: handleContextMenu, closeMenu } = useContextMenu();

  const {
    attributes,
    listeners,
    setNodeRef,
    transform,
    transition,
    isDragging,
  } = useSortable({ id: layer.id });

  // Get hotkey binding for this layer
  const hotkeyBinding = getLayerBinding(layer.id, sceneId);

  const style: React.CSSProperties = {
    transform: transform
      ? CSS.Transform.toString({ ...transform, scaleX: 1, scaleY: 1 })
      : undefined,
    transition,
    opacity: isDragging ? 0.5 : 1,
    zIndex: isDragging ? 50 : undefined,
  };

  if (!source) {
    // Source was deleted but layer still references it
    return (
      <div
        ref={setNodeRef}
        style={style}
        className="flex items-center gap-2 p-2 rounded bg-destructive/10 text-muted"
      >
        <div className="w-16 h-9 bg-[var(--bg-sunken)] rounded flex items-center justify-center flex-shrink-0">
          <span className="text-xs">?</span>
        </div>
        <span className="text-sm italic flex-1">{t('stream.missingSource', { defaultValue: 'Missing source' })}</span>
      </div>
    );
  }

  return (
    <>
      <div
        ref={setNodeRef}
        style={style}
        className={`flex items-center gap-2 p-2 rounded group transition-colors cursor-grab active:cursor-grabbing ${
          isDragging ? 'bg-muted/50' : 'hover:bg-muted/30'
        } ${isSelected ? 'ring-2 ring-primary bg-primary/10' : ''} ${isGrouped ? 'ml-4' : ''}`}
        onClick={(e) => onClick?.(layer.id, e)}
        onContextMenu={handleContextMenu}
        {...attributes}
        {...listeners}
      >
        {/* Live thumbnail preview - uses persistent WebRTC connections */}
        <SourceThumbnail
          sourceId={source.id}
          sourceType={source.type}
          filePath={source.type === 'mediaFile' && 'filePath' in source ? source.filePath : undefined}
        />

        {/* Source name and icon */}
        <div className="flex-1 min-w-0">
          <div className="flex items-start gap-1">
            <SourceIcon type={source.type} />
            <span className="text-sm break-words">{source.name}</span>
          </div>
          {/* Show hotkey indicator if set */}
          {hotkeyBinding && (
            <span className="text-[10px] text-[var(--text-muted)]">
              {formatHotkeyBinding(hotkeyBinding)}
            </span>
          )}
        </div>

        {/* Action buttons - grouped together */}
        <div className="flex items-center gap-0.5 ml-2">
          {/* Visibility toggle */}
          <button
            className="p-1.5 rounded hover:bg-muted/50 transition-colors min-w-[28px] min-h-[28px] flex items-center justify-center"
            onClick={(e) => {
              e.stopPropagation();
              onToggleVisibility(layer.id, layer.visible);
            }}
            title={layer.visible ? t('stream.hideInScene', { defaultValue: 'Hide in scene' }) : t('stream.showInScene', { defaultValue: 'Show in scene' })}
            aria-label={layer.visible ? t('stream.hideInScene', { defaultValue: 'Hide in scene' }) : t('stream.showInScene', { defaultValue: 'Show in scene' })}
            aria-pressed={layer.visible}
          >
            {layer.visible ? (
              <Eye className="w-4 h-4 text-primary" />
            ) : (
              <EyeOff className="w-4 h-4 text-muted" />
            )}
          </button>

          {/* Delete button - removes source from profile entirely */}
          <button
            className="opacity-40 group-hover:opacity-100 p-1.5 hover:bg-destructive/20 rounded transition-opacity min-w-[28px] min-h-[28px] flex items-center justify-center"
            onClick={(e) => {
              e.stopPropagation();
              onRemoveSource(source);
            }}
            title={t('stream.removeSource', { defaultValue: 'Remove source' })}
            aria-label={t('stream.removeSource', { defaultValue: 'Remove source' })}
          >
            <Trash2 className="w-4 h-4 text-destructive" />
          </button>
        </div>
      </div>

      {/* Context menu */}
      {showContextMenu && (
        <LayerContextMenu
          menuRef={contextMenuRef}
          position={contextMenuPos}
          source={source}
          layer={layer}
          profileName={profileName}
          isGrouped={isGrouped}
          onToggleVisibility={() => {
            onToggleVisibility(layer.id, layer.visible);
            closeMenu();
          }}
          onSetHotkey={() => {
            onSetHotkey(layer.id, source.name);
            closeMenu();
          }}
          onRemoveFromGroup={
            isGrouped && onRemoveFromGroup
              ? () => {
                  onRemoveFromGroup(layer.id);
                  closeMenu();
                }
              : undefined
          }
          onRemoveSource={() => {
            onRemoveSource(source);
            closeMenu();
          }}
        />
      )}
    </>
  );
});
