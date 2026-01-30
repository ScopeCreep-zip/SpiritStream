/**
 * Sources Panel
 * OBS-style layer management panel showing layers in the active scene
 * Supports drag-and-drop reordering where top of list = highest zIndex (rendered on top)
 */
import { useState, useMemo, useCallback, useRef, useEffect, memo } from 'react';
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
import type { Profile, Scene, Source } from '@/types/profile';
import type { SourceLayer } from '@/types/scene';
import { createDefaultTransform } from '@/types/scene';
import { useSceneStore } from '@/stores/sceneStore';
import { useSourceStore } from '@/stores/sourceStore';
import { useProfileStore } from '@/stores/profileStore';
import { toast } from '@/hooks/useToast';
import { api } from '@/lib/backend';
import { useWebRTCStream } from '@/hooks/useWebRTCStream';
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
    default:
      return null;
  }
};

// File extensions for static media (images, HTML) that don't need WebRTC streaming
const IMAGE_EXTENSIONS = ['png', 'jpg', 'jpeg', 'gif', 'webp', 'bmp', 'svg'];
const HTML_EXTENSIONS = ['html', 'htm'];
const STATIC_EXTENSIONS = [...IMAGE_EXTENSIONS, ...HTML_EXTENSIONS];

function isStaticMediaFile(filePath: string): boolean {
  const ext = filePath.split('.').pop()?.toLowerCase() ?? '';
  return STATIC_EXTENSIONS.includes(ext);
}

function isImageFile(filePath: string): boolean {
  const ext = filePath.split('.').pop()?.toLowerCase() ?? '';
  return IMAGE_EXTENSIONS.includes(ext);
}

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

  // Get WebRTC stream from persistent connection store
  // Connection is managed by WebRTCConnectionManager, not this component
  const { status, stream, retry } = useWebRTCStream(isStatic ? '' : sourceId);
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
  onToggleVisibility: (layerId: string, currentVisible: boolean) => void;
  onRemoveSource: (source: Source) => void;
}

const SortableLayerItem = memo(function SortableLayerItem({
  layer,
  source,
  onToggleVisibility,
  onRemoveSource,
}: SortableLayerItemProps) {
  const { t } = useTranslation();
  const {
    attributes,
    listeners,
    setNodeRef,
    transform,
    transition,
    isDragging,
  } = useSortable({ id: layer.id });

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
    <div
      ref={setNodeRef}
      style={style}
      className={`flex items-center gap-2 p-2 rounded group transition-colors cursor-grab active:cursor-grabbing ${
        isDragging ? 'bg-muted/50' : 'hover:bg-muted/30'
      }`}
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
        >
          <Trash2 className="w-4 h-4 text-destructive" />
        </button>
      </div>
    </div>
  );
});

export function SourcesPanel({ profile, activeScene }: SourcesPanelProps) {
  const { t } = useTranslation();
  const { addLayer, updateLayer, reorderLayers } = useSceneStore();
  const { removeSource } = useSourceStore();
  const { removeCurrentSource, updateCurrentLayer, reorderCurrentLayers, addCurrentLayer } = useProfileStore();
  const [showAddModal, setShowAddModal] = useState(false);

  // Sensors for drag and drop
  const sensors = useSensors(
    useSensor(PointerSensor, { activationConstraint: { distance: 6 } })
  );

  // Sort layers by zIndex descending (highest zIndex = top of list = rendered on top)
  const sortedLayers = useMemo(() => {
    if (!activeScene) return [];
    return [...activeScene.layers].sort((a, b) => b.zIndex - a.zIndex);
  }, [activeScene?.layers]);

  // Create source lookup map for O(1) access instead of O(n) find()
  const sourceMap = useMemo(
    () => new Map(profile.sources.map((s) => [s.id, s])),
    [profile.sources]
  );

  const handleToggleVisibility = useCallback(async (layerId: string, currentVisible: boolean) => {
    if (!activeScene) return;

    try {
      await updateLayer(profile.name, activeScene.id, layerId, { visible: !currentVisible });
      // Update local state instead of reloading entire profile
      updateCurrentLayer(activeScene.id, layerId, { visible: !currentVisible });
    } catch (err) {
      toast.error(t('stream.visibilityToggleFailed', { error: err instanceof Error ? err.message : String(err), defaultValue: `Failed to toggle visibility: ${err instanceof Error ? err.message : String(err)}` }));
    }
  }, [activeScene, profile.name, updateLayer, updateCurrentLayer, t]);

  const handleRemoveSource = useCallback(async (source: Source) => {
    if (confirm(t('stream.confirmRemoveSource', { name: source.name, defaultValue: `Remove "${source.name}" from profile? This will also remove it from all scenes.` }))) {
      try {
        // Stop any running preview for this source first
        try {
          await api.preview.stopSourcePreview(source.id);
        } catch {
          // Ignore errors - preview may not be running
        }

        await removeSource(profile.name, source.id);
        // Update local state without reloading entire profile to avoid overwriting local edits
        removeCurrentSource(source.id);
        toast.success(t('stream.sourceRemoved', { name: source.name, defaultValue: `Removed ${source.name}` }));
      } catch (err) {
        toast.error(t('stream.sourceRemoveFailed', { error: err instanceof Error ? err.message : String(err), defaultValue: `Failed to remove source: ${err instanceof Error ? err.message : String(err)}` }));
      }
    }
  }, [profile.name, removeSource, removeCurrentSource, t]);

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
      toast.error(t('stream.layerAddFailed', { error: err instanceof Error ? err.message : String(err), defaultValue: `Source added but failed to add to scene: ${err instanceof Error ? err.message : String(err)}` }));
    }
  }, [activeScene, profile.name, addLayer, addCurrentLayer, t]);

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
      toast.error(t('stream.reorderFailed', { error: err instanceof Error ? err.message : String(err), defaultValue: `Failed to reorder layers: ${err instanceof Error ? err.message : String(err)}` }));
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
              items={sortedLayers.map((l) => l.id)}
              strategy={verticalListSortingStrategy}
            >
              <div className="space-y-1">
                {sortedLayers.map((layer) => (
                  <SortableLayerItem
                    key={layer.id}
                    layer={layer}
                    source={sourceMap.get(layer.sourceId)}
                    onToggleVisibility={handleToggleVisibility}
                    onRemoveSource={handleRemoveSource}
                  />
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
    </Card>
  );
}
