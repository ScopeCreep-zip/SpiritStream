/**
 * Sources Panel
 * OBS-style layer management panel showing layers in the active scene
 * Supports drag-and-drop reordering where top of list = highest zIndex (rendered on top)
 */
import { useState, useMemo, useCallback, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { Plus } from 'lucide-react';
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
  arrayMove,
} from '@dnd-kit/sortable';
import { Card, CardHeader, CardTitle, CardBody } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { AddSourceModal } from '@/components/modals/AddSourceModal';
import { HotkeyCaptureModal } from './HotkeyCaptureModal';
import {
  SortableLayerItem,
  LayerGroupSection,
  SourcesPanelToolbar,
} from './sources';
import type { Profile, Scene, Source } from '@/types/profile';
import type { SourceLayer } from '@/types/scene';
import { createDefaultTransform } from '@/types/scene';
import { useSceneStore } from '@/stores/sceneStore';
import { useProfileStore } from '@/stores/profileStore';
import { toast, createErrorHandler } from '@/hooks/useToast';
import { api } from '@/lib/backend';
import type { Source as SourceDef } from '@/types/source';
import { useShallow } from 'zustand/shallow';

interface SourcesPanelProps {
  profile: Profile;
  activeScene?: Scene;
}

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
  } = useSceneStore(
    useShallow(s => ({
      addLayer: s.addLayer,
      updateLayer: s.updateLayer,
      reorderLayers: s.reorderLayers,
      selectedLayerIds: s.selectedLayerIds,
      toggleLayerSelection: s.toggleLayerSelection,
      clearLayerSelection: s.clearLayerSelection,
      createGroup: s.createGroup,
      deleteGroup: s.deleteGroup,
      toggleGroupVisibility: s.toggleGroupVisibility,
      toggleGroupLock: s.toggleGroupLock,
      toggleGroupCollapsed: s.toggleGroupCollapsed,
      removeLayerFromGroup: s.removeLayerFromGroup
    }))
  );
  // Note: We use api.source.remove directly in handleRemoveSource for linked source confirmation flow
  const { removeCurrentSource, updateCurrentLayer, reorderCurrentLayers, addCurrentLayer, updateCurrentScene, setCurrentAudioTracks } = useProfileStore(
    useShallow(s => ({
      removeCurrentSource: s.removeCurrentSource,
      updateCurrentLayer: s.updateCurrentLayer,
      reorderCurrentLayers: s.reorderCurrentLayers,
      addCurrentLayer: s.addCurrentLayer,
      updateCurrentScene: s.updateCurrentScene,
      setCurrentAudioTracks: s.setCurrentAudioTracks,
    }))
  );
  const [showAddModal, setShowAddModal] = useState(false);
  const [hotkeyModalOpen, setHotkeyModalOpen] = useState(false);
  const [hotkeyTargetLayer, setHotkeyTargetLayer] = useState<{ id: string; name: string } | null>(null);
  const [isMultiSelectMode, setIsMultiSelectMode] = useState(false);

  // Sensors for drag and drop
  const sensors = useSensors(
    useSensor(PointerSensor, { activationConstraint: { distance: 6 } })
  );

  // Sort layers by zIndex descending (highest zIndex = top of list = rendered on top)
  const sortedLayers = useMemo(() => {
    if (!activeScene) return [];
    return [...activeScene.layers].sort((a, b) => b.zIndex - a.zIndex);
  }, [activeScene?.layers]);

  // Memoize the layer IDs array for SortableContext to prevent re-renders
  // SortableContext does shallow comparison on items array, so we need stable reference
  const sortedLayerIds = useMemo(
    () => sortedLayers.map((l) => l.id),
    [sortedLayers]
  );

  // Organize layers: ungrouped layers and groups with their children
  const organizedLayers = useMemo(() => {
    if (!activeScene) return { ungrouped: [], groups: [] };

    const groupedLayerIds = new Set(
      activeScene.groups?.flatMap((g) => g.layerIds) ?? []
    );

    // Ungrouped layers sorted by zIndex (descending)
    const ungrouped = sortedLayers.filter((l) => !groupedLayerIds.has(l.id));

    // Groups with their layers
    const groups = (activeScene.groups ?? []).map((group) => ({
      group,
      layers: sortedLayers.filter((l) => group.layerIds.includes(l.id)),
    }));

    return { ungrouped, groups };
  }, [activeScene, sortedLayers]);

  // Create source lookup map for O(1) access instead of O(n) find()
  const sourceMap = useMemo(
    () => new Map(profile.sources.map((s) => [s.id, s])),
    [profile.sources]
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
      // Add layer to backend - returns layerId + authoritative audioTracks
      const result = await addLayer(profile.name, activeScene.id, source.id);

      // Create the layer object for local state update
      // Calculate zIndex as max + 1 to place on top
      const maxZIndex = activeScene.layers.length > 0
        ? Math.max(...activeScene.layers.map(l => l.zIndex))
        : -1;

      const newLayer: SourceLayer = {
        id: result.layerId,
        sourceId: source.id,
        visible: true,
        locked: false,
        transform: createDefaultTransform(activeScene.canvasWidth, activeScene.canvasHeight),
        zIndex: maxZIndex + 1,
      };

      // Update local state instead of reloading entire profile
      addCurrentLayer(activeScene.id, newLayer);

      // Sync audio tracks from backend (authoritative source — eliminates dedup issues)
      if (result.audioTracks) {
        setCurrentAudioTracks(activeScene.id, result.audioTracks);
      }

      toast.success(t('stream.sourceAdded', { name: source.name, defaultValue: `Added ${source.name} to scene` }));
    } catch (err) {
      // Source was added to profile but layer creation failed
      handleLayerAddError(err);
    }
  }, [activeScene, profile.name, addLayer, addCurrentLayer, setCurrentAudioTracks, t, handleLayerAddError]);

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
        <CardHeader className="flex-shrink-0 py-3 px-4">
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
        <CardBody className="flex-1 overflow-y-auto p-3">
          <div className="text-center text-muted text-sm py-8">
            <p>{t('stream.noActiveScene', { defaultValue: 'No scene selected' })}</p>
          </div>
        </CardBody>
      </Card>
    );
  }

  return (
    <Card className="h-full flex flex-col">
      <CardHeader className="flex-shrink-0 py-3 px-4">
        <div className="flex items-center justify-between">
          <CardTitle className="text-sm">{t('stream.sources', { defaultValue: 'Sources' })}</CardTitle>
          <SourcesPanelToolbar
            hasActiveScene={!!activeScene}
            selectedLayerCount={selectedLayerIds.length}
            isMultiSelectMode={isMultiSelectMode}
            onAddSource={() => setShowAddModal(true)}
            onCreateGroup={handleCreateGroup}
            onClearSelection={clearLayerSelection}
          />
        </div>
      </CardHeader>
      <CardBody className="flex-1 overflow-y-auto p-3">
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
                    <LayerGroupSection
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
