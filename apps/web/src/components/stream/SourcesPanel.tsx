/**
 * Sources Panel
 * OBS-style layer management panel showing layers in the active scene
 * Supports drag-and-drop reordering where top of list = highest zIndex (rendered on top)
 */
import { useState, useMemo } from 'react';
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
import { useSceneStore } from '@/stores/sceneStore';
import { useSourceStore } from '@/stores/sourceStore';
import { useProfileStore } from '@/stores/profileStore';
import { toast } from '@/hooks/useToast';
import { api } from '@/lib/backend';
import { useWebRTCPreview } from '@/hooks/useWebRTCPreview';
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

/**
 * Live thumbnail preview for a source using WebRTC
 * Uses the same WebRTC system as the scene canvas for consistency
 */
function SourceThumbnail({ sourceId, sourceType }: { sourceId: string; sourceType: Source['type'] }) {
  const { status, videoRef, retry } = useWebRTCPreview(sourceId);

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

  return (
    <div className="relative w-16 h-9 bg-[var(--bg-sunken)] rounded overflow-hidden flex-shrink-0">
      {/* Video element for WebRTC */}
      <video
        ref={videoRef}
        autoPlay
        muted
        playsInline
        className="w-full h-full object-cover"
        style={{ display: status === 'playing' ? 'block' : 'none' }}
      />

      {/* Loading state */}
      {(status === 'loading' || status === 'connecting') && (
        <div className="absolute inset-0 flex items-center justify-center">
          <div className="w-4 h-4 border-2 border-primary/30 border-t-primary rounded-full animate-spin" />
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
}

interface SortableLayerItemProps {
  layer: SourceLayer;
  source: Source | undefined;
  onToggleVisibility: (layerId: string, currentVisible: boolean) => void;
  onRemoveSource: (source: Source) => void;
}

function SortableLayerItem({
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
      {/* Live thumbnail preview */}
      <SourceThumbnail sourceId={source.id} sourceType={source.type} />

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
}

export function SourcesPanel({ profile, activeScene }: SourcesPanelProps) {
  const { t } = useTranslation();
  const { addLayer, updateLayer, reorderLayers } = useSceneStore();
  const { removeSource } = useSourceStore();
  const { reloadProfile, removeCurrentSource } = useProfileStore();
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

  // Get source info by ID
  const getSourceById = (sourceId: string): Source | undefined => {
    return profile.sources.find((s) => s.id === sourceId);
  };

  const handleToggleVisibility = async (layerId: string, currentVisible: boolean) => {
    if (!activeScene) return;

    try {
      await updateLayer(profile.name, activeScene.id, layerId, { visible: !currentVisible });
      await reloadProfile();
    } catch (err) {
      toast.error(t('stream.visibilityToggleFailed', { error: err instanceof Error ? err.message : String(err), defaultValue: `Failed to toggle visibility: ${err instanceof Error ? err.message : String(err)}` }));
    }
  };

  const handleRemoveSource = async (source: Source) => {
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
  };

  // When a source is added via the modal, also add it as a layer to the active scene
  const handleSourceAdded = async (source: SourceDef) => {
    if (!activeScene) return;

    try {
      await addLayer(profile.name, activeScene.id, source.id);
      await reloadProfile();
      toast.success(t('stream.sourceAdded', { name: source.name, defaultValue: `Added ${source.name} to scene` }));
    } catch (err) {
      // Source was added to profile but layer creation failed
      toast.error(t('stream.layerAddFailed', { error: err instanceof Error ? err.message : String(err), defaultValue: `Source added but failed to add to scene: ${err instanceof Error ? err.message : String(err)}` }));
    }
  };

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
      await reloadProfile();
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
                    source={getSourceById(layer.sourceId)}
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
