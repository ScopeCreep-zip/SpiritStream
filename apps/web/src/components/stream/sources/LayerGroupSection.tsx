import { memo } from 'react';
import { useTranslation } from 'react-i18next';
import {
  Eye,
  EyeOff,
  Lock,
  Unlock,
  ChevronDown,
  ChevronRight,
  FolderOpen,
  FolderClosed,
} from 'lucide-react';
import { useContextMenu } from '@/hooks/useContextMenu';
import type { LayerGroup } from '@/types/scene';

interface LayerGroupSectionProps {
  group: LayerGroup;
  onToggleCollapsed: () => void;
  onToggleVisibility: () => void;
  onToggleLock: () => void;
  onUngroup: () => void;
}

export const LayerGroupSection = memo(function LayerGroupSection({
  group,
  onToggleCollapsed,
  onToggleVisibility,
  onToggleLock,
  onUngroup,
}: LayerGroupSectionProps) {
  const { t } = useTranslation();
  const { isOpen: showContextMenu, position: contextMenuPos, menuRef: contextMenuRef, openMenu: handleContextMenu, closeMenu } = useContextMenu();

  return (
    <>
      <div
        className={`flex items-center gap-2 p-2 rounded bg-[var(--bg-elevated)] cursor-pointer hover:bg-[var(--bg-hover)] ${
          !group.visible ? 'opacity-50' : ''
        }`}
        onClick={onToggleCollapsed}
        onContextMenu={handleContextMenu}
      >
        {/* Collapse indicator */}
        {group.collapsed ? (
          <ChevronRight className="w-4 h-4 text-[var(--text-muted)]" />
        ) : (
          <ChevronDown className="w-4 h-4 text-[var(--text-muted)]" />
        )}

        {/* Folder icon */}
        {group.collapsed ? (
          <FolderClosed className="w-4 h-4 text-[var(--primary)]" />
        ) : (
          <FolderOpen className="w-4 h-4 text-[var(--primary)]" />
        )}

        {/* Group name and count */}
        <span className="flex-1 text-sm font-medium">
          {group.name}
          <span className="ml-1 text-xs text-[var(--text-muted)]">
            ({group.layerIds.length})
          </span>
        </span>

        {/* Action buttons - stopPropagation on each button for robustness */}
        <div className="flex items-center gap-0.5">
          {/* Lock toggle */}
          <button
            className="p-1.5 rounded hover:bg-muted/50 transition-colors"
            onClick={(e) => {
              e.stopPropagation();
              onToggleLock();
            }}
            title={group.locked
              ? t('stream.unlockGroup', { defaultValue: 'Unlock group' })
              : t('stream.lockGroup', { defaultValue: 'Lock group' })
            }
            aria-label={group.locked
              ? t('stream.unlockGroup', { defaultValue: 'Unlock group' })
              : t('stream.lockGroup', { defaultValue: 'Lock group' })
            }
            aria-pressed={group.locked}
          >
            {group.locked ? (
              <Lock className="w-4 h-4 text-[var(--warning)]" />
            ) : (
              <Unlock className="w-4 h-4 text-[var(--text-muted)]" />
            )}
          </button>

          {/* Visibility toggle */}
          <button
            className="p-1.5 rounded hover:bg-muted/50 transition-colors"
            onClick={(e) => {
              e.stopPropagation();
              onToggleVisibility();
            }}
            title={group.visible
              ? t('stream.hideGroup', { defaultValue: 'Hide group' })
              : t('stream.showGroup', { defaultValue: 'Show group' })
            }
            aria-label={group.visible
              ? t('stream.hideGroup', { defaultValue: 'Hide group' })
              : t('stream.showGroup', { defaultValue: 'Show group' })
            }
            aria-pressed={group.visible}
          >
            {group.visible ? (
              <Eye className="w-4 h-4 text-primary" />
            ) : (
              <EyeOff className="w-4 h-4 text-muted" />
            )}
          </button>
        </div>
      </div>

      {/* Context menu */}
      {showContextMenu && (
        <div
          ref={contextMenuRef}
          className="fixed z-50 bg-[var(--bg-elevated)] border border-[var(--border-default)] rounded-lg shadow-lg py-1 min-w-40"
          style={{ left: contextMenuPos.x, top: contextMenuPos.y }}
          role="menu"
          aria-label={t('stream.groupContextMenu', { defaultValue: 'Group options' })}
        >
          <button
            type="button"
            role="menuitem"
            className="w-full px-3 py-1.5 text-left text-sm hover:bg-[var(--bg-hover)] text-[var(--text-secondary)]"
            onClick={() => {
              onToggleVisibility();
              closeMenu();
            }}
          >
            {group.visible
              ? t('stream.hideGroup', { defaultValue: 'Hide Group' })
              : t('stream.showGroup', { defaultValue: 'Show Group' })}
          </button>
          <button
            type="button"
            role="menuitem"
            className="w-full px-3 py-1.5 text-left text-sm hover:bg-[var(--bg-hover)] text-[var(--text-secondary)]"
            onClick={() => {
              onToggleLock();
              closeMenu();
            }}
          >
            {group.locked
              ? t('stream.unlockGroup', { defaultValue: 'Unlock Group' })
              : t('stream.lockGroup', { defaultValue: 'Lock Group' })}
          </button>
          <div className="h-px bg-[var(--border-default)] my-1" role="separator" />
          <button
            type="button"
            role="menuitem"
            className="w-full px-3 py-1.5 text-left text-sm hover:bg-destructive/10 text-destructive"
            onClick={() => {
              onUngroup();
              closeMenu();
            }}
          >
            {t('stream.ungroup', { defaultValue: 'Ungroup' })}
          </button>
        </div>
      )}
    </>
  );
});
