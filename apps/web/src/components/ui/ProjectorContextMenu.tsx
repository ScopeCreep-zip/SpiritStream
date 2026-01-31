/**
 * Projector Context Menu
 * Reusable context menu for opening projectors with monitor selection
 */
import { useState, useEffect, useRef, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { Monitor, ChevronRight, Maximize2, AppWindow } from 'lucide-react';
import { useProjectorStore } from '@/stores/projectorStore';
import type { ProjectorType, MonitorInfo } from '@/types/projector';

interface ProjectorContextMenuProps {
  /** Type of projector (scene, source, preview, program, multiview) */
  type: ProjectorType;
  /** Target ID (source ID or scene ID) - required for source/scene types */
  targetId?: string;
  /** Profile name */
  profileName: string;
  /** Menu position */
  position: { x: number; y: number };
  /** Called when menu should close */
  onClose: () => void;
  /** Optional label for the projector type in menu */
  typeLabel?: string;
  /** Additional menu items to show above projector options */
  additionalItems?: React.ReactNode;
  /** Additional menu items to show below projector options */
  additionalItemsAfter?: React.ReactNode;
}

export function ProjectorContextMenu({
  type,
  targetId,
  profileName,
  position,
  onClose,
  typeLabel,
  additionalItems,
  additionalItemsAfter,
}: ProjectorContextMenuProps) {
  const { t } = useTranslation();
  const menuRef = useRef<HTMLDivElement>(null);
  const [showMonitorSubmenu, setShowMonitorSubmenu] = useState(false);
  const [submenuPosition, setSubmenuPosition] = useState({ x: 0, y: 0 });

  const { openProjector, monitors, monitorsLoaded, refreshMonitors } = useProjectorStore();

  // Load monitors on mount if not loaded
  useEffect(() => {
    if (!monitorsLoaded) {
      refreshMonitors();
    }
  }, [monitorsLoaded, refreshMonitors]);

  // Close menu when clicking outside
  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        onClose();
      }
    };

    const handleEscape = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        onClose();
      }
    };

    document.addEventListener('mousedown', handleClickOutside);
    document.addEventListener('keydown', handleEscape);
    return () => {
      document.removeEventListener('mousedown', handleClickOutside);
      document.removeEventListener('keydown', handleEscape);
    };
  }, [onClose]);

  // Handle windowed projector
  const handleWindowedProjector = useCallback(() => {
    openProjector({
      type,
      displayMode: 'windowed',
      targetId,
      profileName,
      alwaysOnTop: false,
      hideCursor: false,
    });
    onClose();
  }, [type, targetId, profileName, openProjector, onClose]);

  // Handle fullscreen projector on specific monitor
  const handleFullscreenProjector = useCallback((monitor?: MonitorInfo) => {
    openProjector({
      type,
      displayMode: 'fullscreen',
      targetId,
      profileName,
      monitorId: monitor?.id,
      alwaysOnTop: true,
      hideCursor: true,
    });
    onClose();
  }, [type, targetId, profileName, openProjector, onClose]);

  // Show submenu on hover
  const handleMonitorHover = useCallback((e: React.MouseEvent<HTMLButtonElement>) => {
    const rect = e.currentTarget.getBoundingClientRect();
    setSubmenuPosition({ x: rect.right, y: rect.top });
    setShowMonitorSubmenu(true);
  }, []);

  // Get display label for projector type
  const getTypeDisplayLabel = () => {
    if (typeLabel) return typeLabel;
    switch (type) {
      case 'source': return t('projector.source', { defaultValue: 'Source' });
      case 'scene': return t('projector.scene', { defaultValue: 'Scene' });
      case 'preview': return t('projector.preview', { defaultValue: 'Preview' });
      case 'program': return t('projector.program', { defaultValue: 'Program' });
      case 'multiview': return t('projector.multiview', { defaultValue: 'Multiview' });
      default: return type;
    }
  };

  return (
    <div
      ref={menuRef}
      className="fixed z-50 bg-[var(--bg-elevated)] border border-[var(--border-default)] rounded-lg shadow-xl py-1 min-w-[200px]"
      style={{ left: position.x, top: position.y }}
    >
      {/* Additional items before projector options */}
      {additionalItems}

      {/* Fullscreen Projector with monitor submenu */}
      <button
        type="button"
        className="w-full px-3 py-2 text-left text-sm hover:bg-[var(--bg-hover)] flex items-center justify-between group"
        onMouseEnter={handleMonitorHover}
        onMouseLeave={() => setShowMonitorSubmenu(false)}
        onClick={() => handleFullscreenProjector(monitors[0])}
      >
        <span className="flex items-center gap-2">
          <Maximize2 className="w-4 h-4 text-[var(--text-muted)]" />
          {t('projector.fullscreen', { defaultValue: 'Fullscreen Projector' })}
          <span className="text-[var(--text-muted)]">({getTypeDisplayLabel()})</span>
        </span>
        {monitors.length > 1 && (
          <ChevronRight className="w-4 h-4 text-[var(--text-muted)]" />
        )}
      </button>

      {/* Monitor submenu */}
      {showMonitorSubmenu && monitors.length > 1 && (
        <div
          className="fixed z-50 bg-[var(--bg-elevated)] border border-[var(--border-default)] rounded-lg shadow-xl py-1 min-w-[180px]"
          style={{ left: submenuPosition.x, top: submenuPosition.y }}
          onMouseEnter={() => setShowMonitorSubmenu(true)}
          onMouseLeave={() => setShowMonitorSubmenu(false)}
        >
          {monitors.map((monitor) => (
            <button
              key={monitor.id}
              type="button"
              className="w-full px-3 py-2 text-left text-sm hover:bg-[var(--bg-hover)] flex items-center gap-2"
              onClick={() => handleFullscreenProjector(monitor)}
            >
              <Monitor className="w-4 h-4 text-[var(--text-muted)]" />
              <span className="flex-1">
                {monitor.name}
                {monitor.isPrimary && (
                  <span className="ml-1 text-xs text-[var(--text-muted)]">
                    ({t('projector.primary', { defaultValue: 'Primary' })})
                  </span>
                )}
              </span>
              <span className="text-xs text-[var(--text-muted)]">
                {monitor.width}×{monitor.height}
              </span>
            </button>
          ))}
        </div>
      )}

      {/* Windowed Projector */}
      <button
        type="button"
        className="w-full px-3 py-2 text-left text-sm hover:bg-[var(--bg-hover)] flex items-center gap-2"
        onClick={handleWindowedProjector}
      >
        <AppWindow className="w-4 h-4 text-[var(--text-muted)]" />
        {t('projector.windowed', { defaultValue: 'Windowed Projector' })}
        <span className="text-[var(--text-muted)]">({getTypeDisplayLabel()})</span>
      </button>

      {/* Additional items after projector options */}
      {additionalItemsAfter}
    </div>
  );
}

/**
 * Separator for context menu
 */
export function ContextMenuSeparator() {
  return <div className="h-px bg-[var(--border-default)] my-1" />;
}

/**
 * Menu item component for context menu
 */
interface ContextMenuItemProps {
  icon?: React.ReactNode;
  label: string;
  onClick: () => void;
  destructive?: boolean;
  disabled?: boolean;
}

export function ContextMenuItem({ icon, label, onClick, destructive, disabled }: ContextMenuItemProps) {
  return (
    <button
      type="button"
      className={`w-full px-3 py-2 text-left text-sm flex items-center gap-2 ${
        disabled
          ? 'opacity-50 cursor-not-allowed'
          : destructive
          ? 'hover:bg-destructive/10 text-destructive'
          : 'hover:bg-[var(--bg-hover)] text-[var(--text-secondary)]'
      }`}
      onClick={disabled ? undefined : onClick}
      disabled={disabled}
    >
      {icon && <span className="w-4 h-4">{icon}</span>}
      {label}
    </button>
  );
}

/**
 * Hook to manage context menu state
 */
export function useContextMenu() {
  const [isOpen, setIsOpen] = useState(false);
  const [position, setPosition] = useState({ x: 0, y: 0 });

  const open = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    e.stopPropagation();
    setPosition({ x: e.clientX, y: e.clientY });
    setIsOpen(true);
  }, []);

  const close = useCallback(() => {
    setIsOpen(false);
  }, []);

  return { isOpen, position, open, close };
}
