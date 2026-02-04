/**
 * Sources Panel
 * OBS-style layer management panel showing layers in the active scene
 * Supports drag-and-drop reordering where top of list = highest zIndex (rendered on top)
 *
 * Performance optimizations:
 * - useDeferredValue for layer list to prevent UI blocking during rapid scene switches
 * - memo on SortableLayerItem and GroupHeader to prevent unnecessary re-renders
 * - useMemo for source lookup map and sorted layers
 */
import { useState, useMemo, useCallback, useRef, useEffect, memo, useDeferredValue } from 'react';
import { useContextMenu } from '@/hooks/useContextMenu';
import { useTranslation } from 'react-i18next';
import {
  Plus,
  Radio,
  Film,
  Monitor,
  Camera,
  Usb,
  Mic,
  Trash2,
  Eye,
  EyeOff,
  Palette,
  Type,
  Globe,
  FolderOpen,
  FolderClosed,
  Lock,
  Unlock,
  ChevronDown,
  ChevronRight,
  Maximize2,
  AppWindow,
} from 'lucide-react';
import {
  DndContext,
  closestCenter,
  PointerSensor,
  useSensor,
  useSensors,
  type DragEndEvent,
} from '@dnd-kit/core';
import {
  SortableContext,
  verticalListSortingStrategy,
  useSortable,
  arrayMove,
} from '@dnd-kit/sortable';
import { CSS } from '@dnd-kit/utilities';
import { Card, CardHeader, CardTitle, CardBody } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { AddSourceModal } from '@/components/modals/AddSourceModal';
import { HotkeyCaptureModal } from './HotkeyCaptureModal';
import { useHotkeyStore } from '@/stores/hotkeyStore';
import { formatHotkeyBinding } from '@/types/hotkeys';
import type { Profile, Scene, Source } from '@/types/profile';
import type { SourceLayer, LayerGroup } from '@/types/scene';
import { createDefaultTransform } from '@/types/scene';
import { useSceneStore } from '@/stores/sceneStore';
// Note: We use api.source.remove directly for linked source confirmation flow
import { useProfileStore } from '@/stores/profileStore';
import { useProjectorStore } from '@/stores/projectorStore';
import { toast, createErrorHandler } from '@/hooks/useToast';
import { api } from '@/lib/backend';
import { useWebRTCStream } from '@/hooks/useWebRTCStream';
import { isStaticMediaFile, isImageFile, isClientRenderedSource } from '@/lib/mediaTypes';
import type { Source as SourceDef } from '@/types/source';

interface SourcesPanelProps {
  profile: Profile;
  activeScene?: Scene;
}

const SourceIcon = ({ type }: { type: Source['type'] }) => {
  const iconClass = 'w-4 h-4';
  switch (type) {
    case 'rtmp':
      return <Radio className={iconClass} />;
    case 'mediaFile':
      return <Film className={iconClass} />;
    case 'screenCapture':
      return <Monitor className={iconClass} />;
    case 'camera':
      return <Camera className={iconClass} />;
    case 'captureCard':
      return <Usb className={iconClass} />;
    case 'audioDevice':
      return <Mic className={iconClass} />;
    case 'color':
      return <Palette className={iconClass} />;
    case 'text':
      return <Type className={iconClass} />;
    case 'browser':
      return <Globe className={iconClass} />;
    default:
      return null;
  }
};

interface SourceThumbnailProps {
  sourceId: string;
  sourceType: Source['type'];
  /** For mediaFile sources, the file path to check if it's a static image/HTML */
  filePath?: string;
}

/**
 * Live thumbnail preview for a source using WebRTC
 * Uses persistent WebRTC connections managed by WebRTCConnectionManager
 * Connections stay alive regardless of visibility to prevent reconnection delays
 * Memoized to prevent unnecessary re-renders when sibling components update
 */
const SourceThumbnail = memo(function SourceThumbnail({
  sourceId,
  sourceType,
  filePath,
}: SourceThumbnailProps) {
  // Check if this is a static media file (image/HTML) that doesn't need WebRTC
  const isStatic = filePath && isStaticMediaFile(filePath);
  const isImage = filePath && isImageFile(filePath);
  // Check if this is a client-rendered source (color, text, browser) that doesn't need WebRTC
  const isClientRendered = isClientRenderedSource(sourceType);

  // Get WebRTC stream from persistent connection store
  // Connection is managed by WebRTCConnectionManager, not this component
  // Skip WebRTC for static media files and client-rendered sources
  const { status, stream, retry } = useWebRTCStream(isStatic || isClientRendered ? '' : sourceId);
  const videoRef = useRef<HTMLVideoElement>(null);
  const [videoReady, setVideoReady] = useState(false);
  const mountedRef = useRef(true);

  // Track mounted state for cleanup - prevents state updates after unmount
  useEffect(() => {
    mountedRef.current = true;
    return () => { mountedRef.current = false; };
  }, []);

  // Attach stream to video element when it changes
  useEffect(() => {
    const video = videoRef.current;
    if (!video) return;
    video.srcObject = stream;
    setVideoReady(false); // Reset when stream changes - wait for decoder to initialize
  }, [stream]);

  // Listen for video ready events - using multiple signals for reliability
  // The green tint appears when H.264 decoder hasn't received a keyframe yet
  // We wait for BOTH dimensions AND readyState >= HAVE_CURRENT_DATA
  useEffect(() => {
    const video = videoRef.current;
    if (!video) return;

    let frameCheckId: number | null = null;
    let isChecking = false; // Prevent overlapping RAF loops from concurrent events

    // Check if video has actually decoded content
    // readyState >= 2 (HAVE_CURRENT_DATA) means decoder has rendered at least one frame
    // This is more reliable than just checking dimensions, which can be set from H.264 SPS
    // metadata before actual pixels are decoded
    const checkVideoReady = () => {
      if (!mountedRef.current) return true; // Stop if unmounted
      if (video.videoWidth > 0 &&
          video.videoHeight > 0 &&
          video.readyState >= HTMLMediaElement.HAVE_CURRENT_DATA) {
        setVideoReady(true);
        return true;
      }
      return false;
    };

    // Handle loadeddata/canplay events - but also verify decoder readiness
    const handleVideoEvent = () => {
      if (isChecking) return; // Prevent concurrent polling loops
      if (checkVideoReady()) return;

      isChecking = true;
      let attempts = 0;
      // Poll for up to ~1000ms to cover screen capture's 500ms keyframe interval
      // (15-frame keyframe interval @ 30fps = ~500ms for first keyframe)
      const MAX_POLL_ATTEMPTS = 60;

      const pollDimensions = () => {
        if (!mountedRef.current || checkVideoReady() || attempts++ >= MAX_POLL_ATTEMPTS) {
          frameCheckId = null;
          isChecking = false;
          return;
        }
        frameCheckId = requestAnimationFrame(pollDimensions);
      };
      frameCheckId = requestAnimationFrame(pollDimensions);
    };

    // Listen to multiple events for better coverage across stream types
    video.addEventListener('loadeddata', handleVideoEvent);
    video.addEventListener('canplay', handleVideoEvent);

    // Check immediately in case video is already ready
    handleVideoEvent();

    return () => {
      video.removeEventListener('loadeddata', handleVideoEvent);
      video.removeEventListener('canplay', handleVideoEvent);
      if (frameCheckId !== null) {
        cancelAnimationFrame(frameCheckId);
      }
    };
  }, [stream]);

  // Only show preview for video sources
  const hasVideo = sourceType !== 'audioDevice';

  if (!hasVideo) {
    // Audio-only placeholder
    return (
      <div className="w-16 h-9 bg-[var(--bg-sunken)] rounded flex items-center justify-center flex-shrink-0">
        <Mic className="w-4 h-4 text-muted" />
      </div>
    );
  }

  // Static media file (image/HTML) - render directly without WebRTC
  if (isStatic && filePath) {
    const fileUrl = api.preview.getStaticFileUrl(filePath);
    return (
      <div className="relative w-16 h-9 bg-[var(--bg-sunken)] rounded overflow-hidden flex-shrink-0">
        {isImage ? (
          <img
            src={fileUrl}
            alt=""
            className="w-full h-full object-cover"
            onError={(e) => {
              // Hide broken image and show fallback
              e.currentTarget.style.display = 'none';
            }}
          />
        ) : (
          // HTML file - just show an icon
          <div className="w-full h-full flex items-center justify-center">
            <SourceIcon type={sourceType} />
          </div>
        )}
      </div>
    );
  }

  // Client-rendered sources (color, text, browser) - show placeholder with icon
  if (isClientRendered) {
    return (
      <div className="relative w-16 h-9 bg-[var(--bg-sunken)] rounded overflow-hidden flex-shrink-0">
        <div className="w-full h-full flex items-center justify-center">
          <SourceIcon type={sourceType} />
        </div>
      </div>
    );
  }

  return (
    <div className="relative w-16 h-9 bg-[var(--bg-sunken)] rounded overflow-hidden flex-shrink-0">
      {/* Video element for WebRTC - with smooth fade-in transition */}
      {/* Only show when BOTH status is playing AND video has decoded first frame (videoReady) */}
      {/* This prevents the green tint that appears before the H.264 decoder receives a keyframe */}
      <video
        ref={videoRef}
        autoPlay
        muted
        playsInline
        className={`w-full h-full object-cover transition-opacity duration-300 ${
          status === 'playing' && videoReady ? 'opacity-100' : 'opacity-0'
        }`}
      />

      {/* Skeleton loading state - shows until video is actually ready to display */}
      {/* This covers: idle, loading, connecting, AND playing-but-not-yet-decoded states */}
      {(status === 'idle' || status === 'loading' || status === 'connecting' || (status === 'playing' && !videoReady)) && (
        <div className="absolute inset-0 bg-[var(--bg-sunken)] overflow-hidden">
          {/* Animated shimmer effect */}
          <div className="absolute inset-0 bg-gradient-to-r from-transparent via-[var(--bg-elevated)]/50 to-transparent skeleton-shimmer" />
          {/* Source type icon */}
          <div className="absolute inset-0 flex items-center justify-center">
            <SourceIcon type={sourceType} />
          </div>
        </div>
      )}

      {/* Error/Unavailable state - show icon with retry on click */}
      {(status === 'error' || status === 'unavailable') && (
        <div
          className="absolute inset-0 flex items-center justify-center cursor-pointer hover:bg-muted/20 transition-colors"
          onClick={(e) => {
            e.stopPropagation();
            retry();
          }}
          title="Click to retry preview"
        >
          <SourceIcon type={sourceType} />
        </div>
      )}
    </div>
  );
});

interface SortableLayerItemProps {
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

const SortableLayerItem = memo(function SortableLayerItem({
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
  const { openProjector } = useProjectorStore();
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
        <div
          ref={contextMenuRef}
          className="fixed z-50 bg-[var(--bg-elevated)] border border-[var(--border-default)] rounded-lg shadow-lg py-1 min-w-[200px]"
          style={{ left: contextMenuPos.x, top: contextMenuPos.y }}
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
              closeMenu();
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
              closeMenu();
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
            onClick={() => {
              onToggleVisibility(layer.id, layer.visible);
              closeMenu();
            }}
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
            onClick={() => {
              onSetHotkey(layer.id, source.name);
              closeMenu();
            }}
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
                onClick={() => {
                  onRemoveFromGroup(layer.id);
                  closeMenu();
                }}
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
            onClick={() => {
              onRemoveSource(source);
              closeMenu();
            }}
          >
            {t('stream.removeSource', { defaultValue: 'Remove Source' })}
          </button>
        </div>
      )}
    </>
  );
});

interface GroupHeaderProps {
  group: LayerGroup;
  onToggleCollapsed: () => void;
  onToggleVisibility: () => void;
  onToggleLock: () => void;
  onUngroup: () => void;
}

const GroupHeader = memo(function GroupHeader({
  group,
  onToggleCollapsed,
  onToggleVisibility,
  onToggleLock,
  onUngroup,
}: GroupHeaderProps) {
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
          className="fixed z-50 bg-[var(--bg-elevated)] border border-[var(--border-default)] rounded-lg shadow-lg py-1 min-w-[160px]"
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

export function SourcesPanel({ profile, activeScene }: SourcesPanelProps) {
  const { t } = useTranslation();
  const {
    addLayer,
    updateLayer,
    reorderLayers,
    selectedLayerIds,
    toggleLayerSelection,
    clearLayerSelection,
    createGroup,
    deleteGroup,
    toggleGroupVisibility,
    toggleGroupLock,
    toggleGroupCollapsed,
    removeLayerFromGroup,
  } = useSceneStore();
  // Note: We use api.source.remove directly in handleRemoveSource for linked source confirmation flow
  const { removeCurrentSource, updateCurrentLayer, reorderCurrentLayers, addCurrentLayer, updateCurrentScene } = useProfileStore();
  const [showAddModal, setShowAddModal] = useState(false);
  const [hotkeyModalOpen, setHotkeyModalOpen] = useState(false);
  const [hotkeyTargetLayer, setHotkeyTargetLayer] = useState<{ id: string; name: string } | null>(null);
  const [isMultiSelectMode, setIsMultiSelectMode] = useState(false);

  // Sensors for drag and drop
  const sensors = useSensors(
    useSensor(PointerSensor, { activationConstraint: { distance: 6 } })
  );

  // Use deferred value for active scene to prevent UI blocking during rapid scene switches
  // This allows the UI to remain responsive while the layer list updates in the background
  const deferredActiveScene = useDeferredValue(activeScene);

  // Sort layers by zIndex descending (highest zIndex = top of list = rendered on top)
  // Uses deferred scene to prevent blocking during scene transitions
  const sortedLayers = useMemo(() => {
    if (!deferredActiveScene) return [];
    return [...deferredActiveScene.layers].sort((a, b) => b.zIndex - a.zIndex);
  }, [deferredActiveScene?.layers]);

  // Memoize the layer IDs array for SortableContext to prevent re-renders
  // SortableContext does shallow comparison on items array, so we need stable reference
  const sortedLayerIds = useMemo(
    () => sortedLayers.map((l) => l.id),
    [sortedLayers]
  );

  // Organize layers: ungrouped layers and groups with their children
  // Uses deferred scene to prevent blocking during scene transitions
  const organizedLayers = useMemo(() => {
    if (!deferredActiveScene) return { ungrouped: [], groups: [] };

    const groupedLayerIds = new Set(
      deferredActiveScene.groups?.flatMap((g) => g.layerIds) ?? []
    );

    // Ungrouped layers sorted by zIndex (descending)
    const ungrouped = sortedLayers.filter((l) => !groupedLayerIds.has(l.id));

    // Groups with their layers
    const groups = (deferredActiveScene.groups ?? []).map((group) => ({
      group,
      layers: sortedLayers.filter((l) => group.layerIds.includes(l.id)),
    }));

    return { ungrouped, groups };
  }, [deferredActiveScene, sortedLayers]);

  // Use deferred value for profile sources to prevent UI blocking during source updates
  const deferredSources = useDeferredValue(profile.sources);

  // Create source lookup map for O(1) access instead of O(n) find()
  const sourceMap = useMemo(
    () => new Map(deferredSources.map((s) => [s.id, s])),
    [deferredSources]
  );

  // Create reusable error handlers
  const handleVisibilityError = createErrorHandler(t, 'stream.visibilityToggleFailed', 'Failed to toggle visibility');
  const handleRemoveSourceError = createErrorHandler(t, 'stream.sourceRemoveFailed', 'Failed to remove source');
  const handleGroupCreateError = createErrorHandler(t, 'stream.groupCreateFailed', 'Failed to create group');
  const handleReorderError = createErrorHandler(t, 'stream.reorderFailed', 'Failed to reorder layers');
  const handleLayerAddError = createErrorHandler(t, 'stream.layerAddFailed', 'Source added but failed to add to scene');

  const handleToggleVisibility = useCallback(async (layerId: string, currentVisible: boolean) => {
    if (!activeScene) return;

    try {
      await updateLayer(profile.name, activeScene.id, layerId, { visible: !currentVisible });
      // Update local state instead of reloading entire profile
      updateCurrentLayer(activeScene.id, layerId, { visible: !currentVisible });
    } catch (err) {
      handleVisibilityError(err);
    }
  }, [activeScene, profile.name, updateLayer, updateCurrentLayer, handleVisibilityError]);

  const handleRemoveSource = useCallback(async (source: Source) => {
    if (!confirm(t('stream.confirmRemoveSource', { name: source.name, defaultValue: `Remove "${source.name}" from profile? This will also remove it from all scenes.` }))) {
      return;
    }

    try {
      // Stop any running preview for this source first
      try {
        await api.preview.stopSourcePreview(source.id);
      } catch {
        // Ignore errors - preview may not be running
      }

      // First call with removeLinked=false to check for linked sources
      const result = await api.source.remove(profile.name, source.id, false);

      if ('requiresConfirmation' in result && result.requiresConfirmation) {
        // Source has linked audio - ask user what to do
        const linkedNames = result.linkedSourceNames.join(', ');
        const removeLinked = confirm(
          t('stream.confirmRemoveLinked', {
            name: source.name,
            linkedNames,
            defaultValue: `"${source.name}" has linked audio source(s): ${linkedNames}\n\nClick OK to remove both, or Cancel to remove only the video source.`
          })
        );

        // Call again with user's choice
        const finalResult = await api.source.remove(profile.name, source.id, removeLinked);

        if ('removed' in finalResult && finalResult.removed) {
          // Update local state
          removeCurrentSource(source.id);
          if (removeLinked && finalResult.linkedRemoved) {
            finalResult.linkedRemoved.forEach((id) => removeCurrentSource(id));
          }
          toast.success(t('stream.sourceRemoved', { name: source.name, defaultValue: `Removed ${source.name}` }));
        }
      } else if ('removed' in result && result.removed) {
        // No linked sources, already removed
        removeCurrentSource(source.id);
        if (result.linkedRemoved) {
          result.linkedRemoved.forEach((id) => removeCurrentSource(id));
        }
        toast.success(t('stream.sourceRemoved', { name: source.name, defaultValue: `Removed ${source.name}` }));
      }
    } catch (err) {
      handleRemoveSourceError(err);
    }
  }, [profile.name, removeCurrentSource, t, handleRemoveSourceError]);

  const handleSetHotkey = useCallback((layerId: string, layerName: string) => {
    setHotkeyTargetLayer({ id: layerId, name: layerName });
    setHotkeyModalOpen(true);
  }, []);

  // Group operations
  const handleCreateGroup = useCallback(async () => {
    if (!activeScene || selectedLayerIds.length < 2) return;

    try {
      const newGroup = await createGroup(
        profile.name,
        activeScene,
        selectedLayerIds,
        t('stream.newGroup', { defaultValue: 'New Group' })
      );
      // Update local scene state
      updateCurrentScene(activeScene.id, {
        groups: [...(activeScene.groups || []), newGroup],
      });
      toast.success(t('stream.groupCreated', { defaultValue: 'Group created' }));
    } catch (err) {
      handleGroupCreateError(err);
    }
  }, [activeScene, selectedLayerIds, createGroup, profile.name, t, updateCurrentScene, handleGroupCreateError]);

  const handleToggleGroupVisibility = useCallback(async (groupId: string) => {
    if (!activeScene) return;

    try {
      await toggleGroupVisibility(profile.name, activeScene, groupId);
      const group = activeScene.groups?.find((g) => g.id === groupId);
      if (group) {
        // Update local state
        updateCurrentScene(activeScene.id, {
          groups: activeScene.groups?.map((g) =>
            g.id === groupId ? { ...g, visible: !g.visible } : g
          ),
          layers: activeScene.layers.map((l) =>
            group.layerIds.includes(l.id) ? { ...l, visible: !group.visible } : l
          ),
        });
      }
    } catch {
      toast.error(t('stream.groupVisibilityFailed', { defaultValue: 'Failed to toggle group visibility' }));
    }
  }, [activeScene, profile.name, toggleGroupVisibility, t, updateCurrentScene]);

  const handleToggleGroupLock = useCallback(async (groupId: string) => {
    if (!activeScene) return;

    try {
      await toggleGroupLock(profile.name, activeScene, groupId);
      const group = activeScene.groups?.find((g) => g.id === groupId);
      if (group) {
        updateCurrentScene(activeScene.id, {
          groups: activeScene.groups?.map((g) =>
            g.id === groupId ? { ...g, locked: !g.locked } : g
          ),
          layers: activeScene.layers.map((l) =>
            group.layerIds.includes(l.id) ? { ...l, locked: !group.locked } : l
          ),
        });
      }
    } catch {
      toast.error(t('stream.groupLockFailed', { defaultValue: 'Failed to toggle group lock' }));
    }
  }, [activeScene, profile.name, toggleGroupLock, t, updateCurrentScene]);

  const handleToggleGroupCollapsed = useCallback(async (groupId: string) => {
    if (!activeScene) return;

    try {
      await toggleGroupCollapsed(profile.name, activeScene, groupId);
      updateCurrentScene(activeScene.id, {
        groups: activeScene.groups?.map((g) =>
          g.id === groupId ? { ...g, collapsed: !g.collapsed } : g
        ),
      });
    } catch (err) {
      // Silently fail - this is just a UI preference
    }
  }, [activeScene, profile.name, toggleGroupCollapsed, updateCurrentScene]);

  const handleUngroup = useCallback(async (groupId: string) => {
    if (!activeScene) return;

    try {
      await deleteGroup(profile.name, activeScene, groupId);
      updateCurrentScene(activeScene.id, {
        groups: activeScene.groups?.filter((g) => g.id !== groupId),
      });
      toast.success(t('stream.ungrouped', { defaultValue: 'Layers ungrouped' }));
    } catch {
      toast.error(t('stream.ungroupFailed', { defaultValue: 'Failed to ungroup' }));
    }
  }, [activeScene, profile.name, deleteGroup, t, updateCurrentScene]);

  // Toggle multi-select mode with Ctrl/Cmd key
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Control' || e.key === 'Meta') {
        setIsMultiSelectMode(true);
      }
    };
    const handleKeyUp = (e: KeyboardEvent) => {
      if (e.key === 'Control' || e.key === 'Meta') {
        setIsMultiSelectMode(false);
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    window.addEventListener('keyup', handleKeyUp);
    return () => {
      window.removeEventListener('keydown', handleKeyDown);
      window.removeEventListener('keyup', handleKeyUp);
    };
  }, []);

  // Layer click handler for multi-select
  const handleLayerClick = useCallback((layerId: string, e: React.MouseEvent) => {
    if (isMultiSelectMode || e.ctrlKey || e.metaKey) {
      e.preventDefault();
      e.stopPropagation();
      toggleLayerSelection(layerId);
    }
  }, [isMultiSelectMode, toggleLayerSelection]);

  // Remove layer from its group
  const handleRemoveLayerFromGroup = useCallback(async (layerId: string) => {
    if (!activeScene) return;

    try {
      await removeLayerFromGroup(profile.name, activeScene, layerId);
      // Update local state
      updateCurrentScene(activeScene.id, {
        groups: activeScene.groups?.map((g) => ({
          ...g,
          layerIds: g.layerIds.filter((id) => id !== layerId),
        })),
      });
    } catch {
      toast.error(t('stream.removeFromGroupFailed', { defaultValue: 'Failed to remove from group' }));
    }
  }, [activeScene, profile.name, removeLayerFromGroup, t, updateCurrentScene]);

  // When a source is added via the modal, also add it as a layer to the active scene
  const handleSourceAdded = useCallback(async (source: SourceDef) => {
    if (!activeScene) return;

    try {
      // Add layer to backend - returns the layerId
      const layerId = await addLayer(profile.name, activeScene.id, source.id);

      // Create the layer object for local state update
      // Calculate zIndex as max + 1 to place on top
      const maxZIndex = activeScene.layers.length > 0
        ? Math.max(...activeScene.layers.map(l => l.zIndex))
        : -1;

      const newLayer: SourceLayer = {
        id: layerId,
        sourceId: source.id,
        visible: true,
        locked: false,
        transform: createDefaultTransform(activeScene.canvasWidth, activeScene.canvasHeight),
        zIndex: maxZIndex + 1,
      };

      // Update local state instead of reloading entire profile
      addCurrentLayer(activeScene.id, newLayer);
      toast.success(t('stream.sourceAdded', { name: source.name, defaultValue: `Added ${source.name} to scene` }));
    } catch (err) {
      // Source was added to profile but layer creation failed
      handleLayerAddError(err);
    }
  }, [activeScene, profile.name, addLayer, addCurrentLayer, t, handleLayerAddError]);

  const handleDragEnd = async (event: DragEndEvent) => {
    const { active, over } = event;
    if (!over || active.id === over.id || !activeScene) return;

    const layerIds = sortedLayers.map((l) => l.id);
    const fromIdx = layerIds.indexOf(String(active.id));
    const toIdx = layerIds.indexOf(String(over.id));

    if (fromIdx === -1 || toIdx === -1) return;

    // Reorder in UI order (highest zIndex first)
    const newOrder = arrayMove(layerIds, fromIdx, toIdx);

    // Reverse for server: server assigns zIndex = arrayIndex
    // So first in array gets zIndex 0 (bottom), last gets highest zIndex (top)
    // We want top of UI list (newOrder[0]) to have highest zIndex,
    // so we reverse the array before sending to server
    const serverOrder = [...newOrder].reverse();

    try {
      await reorderLayers(profile.name, activeScene.id, serverOrder);
      // Update local state instead of reloading entire profile
      reorderCurrentLayers(activeScene.id, serverOrder);
    } catch (err) {
      handleReorderError(err);
    }
  };

  // No active scene selected
  if (!activeScene) {
    return (
      <Card className="h-full flex flex-col">
        <CardHeader className="flex-shrink-0" style={{ padding: '12px 16px' }}>
          <div className="flex items-center justify-between">
            <CardTitle className="text-sm">{t('stream.sources', { defaultValue: 'Sources' })}</CardTitle>
            <Button
              variant="ghost"
              size="sm"
              className="min-w-[36px] min-h-[36px]"
              disabled
              title={t('stream.addSource', { defaultValue: 'Add Source' })}
            >
              <Plus className="w-4 h-4" />
            </Button>
          </div>
        </CardHeader>
        <CardBody className="flex-1 overflow-y-auto" style={{ padding: '12px' }}>
          <div className="text-center text-muted text-sm py-8">
            <p>{t('stream.noActiveScene', { defaultValue: 'No scene selected' })}</p>
          </div>
        </CardBody>
      </Card>
    );
  }

  return (
    <Card className="h-full flex flex-col">
      <CardHeader className="flex-shrink-0" style={{ padding: '12px 16px' }}>
        <div className="flex items-center justify-between">
          <CardTitle className="text-sm">{t('stream.sources', { defaultValue: 'Sources' })}</CardTitle>
          <div className="flex items-center gap-1">
            {selectedLayerIds.length >= 2 && (
              <Button
                variant="ghost"
                size="sm"
                className="text-xs px-2 py-1 h-auto"
                onClick={handleCreateGroup}
                title={t('stream.groupSelected', { defaultValue: 'Group selected layers' })}
              >
                <FolderOpen className="w-3.5 h-3.5 mr-1" />
                {t('stream.group', { defaultValue: 'Group' })} ({selectedLayerIds.length})
              </Button>
            )}
            {selectedLayerIds.length > 0 && (
              <Button
                variant="ghost"
                size="sm"
                className="text-xs px-2 py-1 h-auto"
                onClick={clearLayerSelection}
                title={t('common.clearSelection', { defaultValue: 'Clear selection' })}
              >
                ×
              </Button>
            )}
            <Button
              variant="ghost"
              size="sm"
              className="min-w-[36px] min-h-[36px]"
              onClick={() => setShowAddModal(true)}
              title={t('stream.addSource', { defaultValue: 'Add Source' })}
            >
              <Plus className="w-4 h-4" />
            </Button>
          </div>
        </div>
        {isMultiSelectMode && (
          <p className="text-xs text-[var(--text-muted)] mt-1">
            {t('stream.multiSelectMode', { defaultValue: 'Click layers to select multiple' })}
          </p>
        )}
      </CardHeader>
      <CardBody className="flex-1 overflow-y-auto" style={{ padding: '12px' }}>
        {sortedLayers.length === 0 ? (
          <div className="text-center text-muted text-sm py-8">
            <p>{t('stream.noSourcesInScene', { defaultValue: 'No sources in scene' })}</p>
            <Button
              variant="ghost"
              size="sm"
              className="mt-2"
              onClick={() => setShowAddModal(true)}
            >
              <Plus className="w-4 h-4 mr-1" />
              {t('stream.addSource', { defaultValue: 'Add Source' })}
            </Button>
          </div>
        ) : (
          <DndContext
            sensors={sensors}
            collisionDetection={closestCenter}
            onDragEnd={handleDragEnd}
          >
            <SortableContext
              items={sortedLayerIds}
              strategy={verticalListSortingStrategy}
            >
              <div className="space-y-1">
                {/* Ungrouped layers */}
                {organizedLayers.ungrouped.map((layer) => (
                  <SortableLayerItem
                    key={layer.id}
                    layer={layer}
                    source={sourceMap.get(layer.sourceId)}
                    sceneId={activeScene.id}
                    profileName={profile.name}
                    isSelected={selectedLayerIds.includes(layer.id)}
                    isGrouped={false}
                    onToggleVisibility={handleToggleVisibility}
                    onRemoveSource={handleRemoveSource}
                    onSetHotkey={handleSetHotkey}
                    onClick={handleLayerClick}
                  />
                ))}

                {/* Groups with their layers */}
                {organizedLayers.groups.map(({ group, layers }) => (
                  <div key={group.id} className="space-y-1">
                    <GroupHeader
                      group={group}
                      onToggleCollapsed={() => handleToggleGroupCollapsed(group.id)}
                      onToggleVisibility={() => handleToggleGroupVisibility(group.id)}
                      onToggleLock={() => handleToggleGroupLock(group.id)}
                      onUngroup={() => handleUngroup(group.id)}
                    />
                    {!group.collapsed && layers.map((layer) => (
                      <SortableLayerItem
                        key={layer.id}
                        layer={layer}
                        source={sourceMap.get(layer.sourceId)}
                        sceneId={activeScene.id}
                        profileName={profile.name}
                        isSelected={selectedLayerIds.includes(layer.id)}
                        isGrouped={true}
                        onToggleVisibility={handleToggleVisibility}
                        onRemoveSource={handleRemoveSource}
                        onSetHotkey={handleSetHotkey}
                        onClick={handleLayerClick}
                        onRemoveFromGroup={handleRemoveLayerFromGroup}
                      />
                    ))}
                  </div>
                ))}
              </div>
            </SortableContext>
          </DndContext>
        )}
      </CardBody>

      {/* Add Source Modal */}
      <AddSourceModal
        open={showAddModal}
        onClose={() => setShowAddModal(false)}
        profileName={profile.name}
        excludeTypes={['audioDevice']}
        onSourceAdded={handleSourceAdded}
      />

      {/* Hotkey Capture Modal */}
      {hotkeyTargetLayer && (
        <HotkeyCaptureModal
          open={hotkeyModalOpen}
          onClose={() => {
            setHotkeyModalOpen(false);
            setHotkeyTargetLayer(null);
          }}
          layerId={hotkeyTargetLayer.id}
          sceneId={activeScene.id}
          layerName={hotkeyTargetLayer.name}
        />
      )}
    </Card>
  );
}
