/**
 * useSourcePanelActions
 * Extracts business logic callbacks from SourcesPanel into a reusable hook.
 * All returned handlers are stable references via useCallback.
 */
import { useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { arrayMove } from '@dnd-kit/sortable';
import type { DragEndEvent } from '@dnd-kit/core';
import { useSceneStore } from '@/stores/sceneStore';
import { useProfileStore } from '@/stores/profileStore';
import { toast, createErrorHandler } from '@/hooks/useToast';
import { api } from '@/lib/backend';
import type { Scene, SourceLayer } from '@/types/scene';
import { createDefaultTransform } from '@/types/scene';
import type { Source } from '@/types/profile';
import type { Source as SourceDef } from '@/types/source';
import { useShallow } from 'zustand/shallow';

interface UseSourcePanelActionsParams {
  activeScene: Scene | undefined;
  profileName: string;
  sortedLayers: SourceLayer[];
  isMultiSelectMode: boolean;
}

export function useSourcePanelActions({
  activeScene,
  profileName,
  sortedLayers,
  isMultiSelectMode,
}: UseSourcePanelActionsParams) {
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
    useShallow((s) => ({
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
      removeLayerFromGroup: s.removeLayerFromGroup,
    }))
  );

  const {
    removeCurrentSource,
    updateCurrentLayer,
    reorderCurrentLayers,
    addCurrentLayer,
    updateCurrentScene,
    setAllSourceAudioConfigs,
  } = useProfileStore(
    useShallow((s) => ({
      removeCurrentSource: s.removeCurrentSource,
      updateCurrentLayer: s.updateCurrentLayer,
      reorderCurrentLayers: s.reorderCurrentLayers,
      addCurrentLayer: s.addCurrentLayer,
      updateCurrentScene: s.updateCurrentScene,
      setAllSourceAudioConfigs: s.setAllSourceAudioConfigs,
    }))
  );

  // Create reusable error handlers
  const handleVisibilityError = createErrorHandler(t, 'stream.visibilityToggleFailed', 'Failed to toggle visibility');
  const handleRemoveSourceError = createErrorHandler(t, 'stream.sourceRemoveFailed', 'Failed to remove source');
  const handleGroupCreateError = createErrorHandler(t, 'stream.groupCreateFailed', 'Failed to create group');
  const handleReorderError = createErrorHandler(t, 'stream.reorderFailed', 'Failed to reorder layers');
  const handleLayerAddError = createErrorHandler(t, 'stream.layerAddFailed', 'Source added but failed to add to scene');

  const handleToggleVisibility = useCallback(
    async (layerId: string, currentVisible: boolean) => {
      if (!activeScene) return;

      try {
        await updateLayer(profileName, activeScene.id, layerId, { visible: !currentVisible });
        updateCurrentLayer(activeScene.id, layerId, { visible: !currentVisible });
      } catch (err) {
        handleVisibilityError(err);
      }
    },
    [activeScene, profileName, updateLayer, updateCurrentLayer, handleVisibilityError]
  );

  const handleRemoveSource = useCallback(
    async (source: Source) => {
      if (
        !confirm(
          t('stream.confirmRemoveSource', {
            name: source.name,
            defaultValue: `Remove "${source.name}" from profile? This will also remove it from all scenes.`,
          })
        )
      ) {
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
        const result = await api.source.remove(profileName, source.id, false);

        if ('requiresConfirmation' in result && result.requiresConfirmation) {
          // Source has linked audio - ask user what to do
          const linkedNames = result.linkedSourceNames.join(', ');
          const removeLinked = confirm(
            t('stream.confirmRemoveLinked', {
              name: source.name,
              linkedNames,
              defaultValue: `"${source.name}" has linked audio source(s): ${linkedNames}\n\nClick OK to remove both, or Cancel to remove only the video source.`,
            })
          );

          // Call again with user's choice
          const finalResult = await api.source.remove(profileName, source.id, removeLinked);

          if ('removed' in finalResult && finalResult.removed) {
            removeCurrentSource(source.id);
            if (removeLinked && finalResult.linkedRemoved) {
              finalResult.linkedRemoved.forEach((id) => removeCurrentSource(id));
            }
            toast.success(
              t('stream.sourceRemoved', { name: source.name, defaultValue: `Removed ${source.name}` })
            );
          }
        } else if ('removed' in result && result.removed) {
          // No linked sources, already removed
          removeCurrentSource(source.id);
          if (result.linkedRemoved) {
            result.linkedRemoved.forEach((id) => removeCurrentSource(id));
          }
          toast.success(
            t('stream.sourceRemoved', { name: source.name, defaultValue: `Removed ${source.name}` })
          );
        }
      } catch (err) {
        handleRemoveSourceError(err);
      }
    },
    [profileName, removeCurrentSource, t, handleRemoveSourceError]
  );

  const handleCreateGroup = useCallback(async () => {
    if (!activeScene || selectedLayerIds.length < 2) return;

    try {
      const newGroup = await createGroup(
        profileName,
        activeScene,
        selectedLayerIds,
        t('stream.newGroup', { defaultValue: 'New Group' })
      );
      updateCurrentScene(activeScene.id, {
        groups: [...(activeScene.groups || []), newGroup],
      });
      toast.success(t('stream.groupCreated', { defaultValue: 'Group created' }));
    } catch (err) {
      handleGroupCreateError(err);
    }
  }, [activeScene, selectedLayerIds, createGroup, profileName, t, updateCurrentScene, handleGroupCreateError]);

  const handleToggleGroupVisibility = useCallback(
    async (groupId: string) => {
      if (!activeScene) return;

      try {
        await toggleGroupVisibility(profileName, activeScene, groupId);
        const group = activeScene.groups?.find((g) => g.id === groupId);
        if (group) {
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
        toast.error(
          t('stream.groupVisibilityFailed', { defaultValue: 'Failed to toggle group visibility' })
        );
      }
    },
    [activeScene, profileName, toggleGroupVisibility, t, updateCurrentScene]
  );

  const handleToggleGroupLock = useCallback(
    async (groupId: string) => {
      if (!activeScene) return;

      try {
        await toggleGroupLock(profileName, activeScene, groupId);
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
    },
    [activeScene, profileName, toggleGroupLock, t, updateCurrentScene]
  );

  const handleToggleGroupCollapsed = useCallback(
    async (groupId: string) => {
      if (!activeScene) return;

      try {
        await toggleGroupCollapsed(profileName, activeScene, groupId);
        updateCurrentScene(activeScene.id, {
          groups: activeScene.groups?.map((g) =>
            g.id === groupId ? { ...g, collapsed: !g.collapsed } : g
          ),
        });
      } catch (_err) {
        // Silently fail - this is just a UI preference
      }
    },
    [activeScene, profileName, toggleGroupCollapsed, updateCurrentScene]
  );

  const handleUngroup = useCallback(
    async (groupId: string) => {
      if (!activeScene) return;

      try {
        await deleteGroup(profileName, activeScene, groupId);
        updateCurrentScene(activeScene.id, {
          groups: activeScene.groups?.filter((g) => g.id !== groupId),
        });
        toast.success(t('stream.ungrouped', { defaultValue: 'Layers ungrouped' }));
      } catch {
        toast.error(t('stream.ungroupFailed', { defaultValue: 'Failed to ungroup' }));
      }
    },
    [activeScene, profileName, deleteGroup, t, updateCurrentScene]
  );

  const handleLayerClick = useCallback(
    (layerId: string, e: React.MouseEvent) => {
      if (isMultiSelectMode || e.ctrlKey || e.metaKey) {
        e.preventDefault();
        e.stopPropagation();
        toggleLayerSelection(layerId);
      }
    },
    [isMultiSelectMode, toggleLayerSelection]
  );

  const handleRemoveLayerFromGroup = useCallback(
    async (layerId: string) => {
      if (!activeScene) return;

      try {
        await removeLayerFromGroup(profileName, activeScene, layerId);
        updateCurrentScene(activeScene.id, {
          groups: activeScene.groups?.map((g) => ({
            ...g,
            layerIds: g.layerIds.filter((id) => id !== layerId),
          })),
        });
      } catch {
        toast.error(
          t('stream.removeFromGroupFailed', { defaultValue: 'Failed to remove from group' })
        );
      }
    },
    [activeScene, profileName, removeLayerFromGroup, t, updateCurrentScene]
  );

  const handleSourceAdded = useCallback(
    async (source: SourceDef) => {
      if (!activeScene) return;

      try {
        // Add layer to backend - returns layerId + authoritative audioTracks
        const result = await addLayer(profileName, activeScene.id, source.id);

        // Create the layer object for local state update
        // Calculate zIndex as max + 1 to place on top
        const maxZIndex =
          activeScene.layers.length > 0
            ? Math.max(...activeScene.layers.map((l) => l.zIndex))
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

        // Sync source audio configs from backend (authoritative source)
        if (result.sourceAudioConfigs) {
          setAllSourceAudioConfigs(result.sourceAudioConfigs);
        }

        toast.success(
          t('stream.sourceAdded', { name: source.name, defaultValue: `Added ${source.name} to scene` })
        );
      } catch (err) {
        // Source was added to profile but layer creation failed
        handleLayerAddError(err);
      }
    },
    [activeScene, profileName, addLayer, addCurrentLayer, setAllSourceAudioConfigs, t, handleLayerAddError]
  );

  const handleDragEnd = useCallback(
    async (event: DragEndEvent) => {
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
        await reorderLayers(profileName, activeScene.id, serverOrder);
        reorderCurrentLayers(activeScene.id, serverOrder);
      } catch (err) {
        handleReorderError(err);
      }
    },
    [activeScene, profileName, sortedLayers, reorderLayers, reorderCurrentLayers, handleReorderError]
  );

  return {
    // Handlers
    handleToggleVisibility,
    handleRemoveSource,
    handleCreateGroup,
    handleToggleGroupVisibility,
    handleToggleGroupLock,
    handleToggleGroupCollapsed,
    handleUngroup,
    handleLayerClick,
    handleRemoveLayerFromGroup,
    handleSourceAdded,
    handleDragEnd,

    // Passthrough from sceneStore for UI usage
    selectedLayerIds,
    clearLayerSelection,
  };
}
