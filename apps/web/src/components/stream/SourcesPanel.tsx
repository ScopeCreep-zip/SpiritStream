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
} from '@dnd-kit/core';
import {
  SortableContext,
  verticalListSortingStrategy,
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
import type { Profile, Scene } from '@/types/profile';
import { useSourcePanelActions } from '@/hooks/useSourcePanelActions';

interface SourcesPanelProps {
  profile: Profile;
  activeScene?: Scene;
}

export function SourcesPanel({ profile, activeScene }: SourcesPanelProps) {
  const { t } = useTranslation();
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

  // Business logic callbacks extracted into a custom hook
  const {
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
    selectedLayerIds,
    clearLayerSelection,
  } = useSourcePanelActions({
    activeScene,
    profileName: profile.name,
    sortedLayers,
    isMultiSelectMode,
  });

  // Hotkey handler stays local - it only manages modal UI state
  const handleSetHotkey = useCallback((layerId: string, layerName: string) => {
    setHotkeyTargetLayer({ id: layerId, name: layerName });
    setHotkeyModalOpen(true);
  }, []);

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
