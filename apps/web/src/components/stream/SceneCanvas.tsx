/**
 * Scene Canvas
 * Central composition canvas showing layers with drag/resize
 *
 * Supports two view modes:
 * - Edit: Shows individual layer previews with resize handles
 * - Preview: Shows composed scene output from backend Compositor
 */
import React, { useRef, useState, useLayoutEffect, useCallback, useMemo } from 'react';
import { Card } from '@/components/ui/Card';
import type { Scene, Transform } from '@/types/scene';
import type { Source } from '@/types/source';
import { useSceneStore } from '@/stores/sceneStore';
import { useProfileStore } from '@/stores/profileStore';
import { LayerPreview, calculateCanvasDimensions } from './canvas';

type ViewMode = 'edit' | 'preview';

interface SceneCanvasProps {
  scene?: Scene;
  sources: Source[];
  selectedLayerId: string | null;
  onSelectLayer: (layerId: string | null) => void;
  profileName?: string;
  /** Studio mode: 'preview' (green, editable), 'program' (red, read-only), or undefined for normal */
  studioMode?: 'preview' | 'program';
  /** All scenes in the profile (for nested scene rendering) */
  scenes?: Scene[];
  /** Hide header bar (for projector/fullscreen use) */
  hideHeader?: boolean;
}

export const SceneCanvas = React.memo(function SceneCanvas({
  scene,
  sources,
  selectedLayerId,
  onSelectLayer,
  profileName,
  studioMode,
  scenes = [],
  hideHeader = false,
}: SceneCanvasProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const [viewMode, setViewMode] = useState<ViewMode>(studioMode ? 'preview' : 'edit');
  const { updateLayer } = useSceneStore();
  const { updateCurrentLayer } = useProfileStore();

  // Force preview mode in Program pane (can't edit live output)
  const effectiveViewMode = studioMode === 'program' ? 'preview' : viewMode;

  const [dimensions, setDimensions] = useState({ width: 320, height: 180 });

  useLayoutEffect(() => {
    if (!containerRef.current || !scene) return;

    let retryTimeout: ReturnType<typeof setTimeout> | null = null;
    let rafId: number | null = null;

    const updateDimensions = () => {
      const container = containerRef.current;
      if (!container) return;

      const availableWidth = container.clientWidth;
      const availableHeight = container.clientHeight;

      if (availableWidth <= 0 || availableHeight <= 0) {
        if (retryTimeout) clearTimeout(retryTimeout);
        retryTimeout = setTimeout(updateDimensions, 16);
        return;
      }

      const newDims = calculateCanvasDimensions(
        scene.canvasWidth,
        scene.canvasHeight,
        availableWidth,
        availableHeight
      );

      setDimensions(prev => {
        if (prev.width === newDims.width && prev.height === newDims.height) return prev;
        return newDims;
      });
    };

    updateDimensions();

    const observer = new ResizeObserver(() => {
      if (rafId) cancelAnimationFrame(rafId);
      rafId = requestAnimationFrame(updateDimensions);
    });
    observer.observe(containerRef.current);

    return () => {
      if (retryTimeout) clearTimeout(retryTimeout);
      if (rafId) cancelAnimationFrame(rafId);
      observer.disconnect();
    };
  }, [scene?.canvasWidth, scene?.canvasHeight]);

  const handleLayerTransformChange = useCallback(
    async (layerId: string, newTransform: Partial<Transform>) => {
      if (!profileName || !scene) return;

      const layer = scene.layers.find((l) => l.id === layerId);
      if (!layer) return;

      const updatedTransform = { ...layer.transform, ...newTransform };

      try {
        await updateLayer(profileName, scene.id, layerId, { transform: updatedTransform });
        updateCurrentLayer(scene.id, layerId, { transform: updatedTransform });
      } catch (err) {
        console.error('Failed to update layer transform:', err);
      }
    },
    [profileName, scene, updateLayer, updateCurrentLayer]
  );

  if (!scene) {
    return (
      <Card className="h-full flex items-center justify-center">
        <div className="text-muted text-center">
          <p>No scene selected</p>
          <p className="text-sm">Create or select a scene to start compositing</p>
        </div>
      </Card>
    );
  }

  const sortedLayers = useMemo(
    () => [...scene.layers].sort((a, b) => a.zIndex - b.zIndex),
    [scene.layers]
  );

  const getSource = (sourceId: string) => sources.find((s) => s.id === sourceId);
  const getSourceName = (sourceId: string) => getSource(sourceId)?.name ?? 'Unknown Source';

  const scale = dimensions.width / scene.canvasWidth;

  return (
    <Card className={`h-full flex flex-col overflow-hidden ${hideHeader ? 'border-0 rounded-none bg-transparent' : ''}`}>
      {/* Header with view mode toggle */}
      {!hideHeader && (
        <div className="flex items-center justify-between px-4 py-3 border-b border-[var(--border-muted)] bg-[var(--bg-elevated)]">
          {studioMode === 'preview' ? (
            <div className="flex items-center gap-2">
              <span className="text-sm font-medium text-green-500 flex items-center gap-1.5">
                <span className="w-2 h-2 bg-green-500 rounded-full" />
                Preview
              </span>
              <span className="text-xs text-[var(--text-muted)]">{scene?.name}</span>
            </div>
          ) : studioMode === 'program' ? (
            <div className="flex items-center gap-2">
              <span className="text-sm font-medium text-red-500 flex items-center gap-1.5">
                <span className="w-2 h-2 bg-red-500 rounded-full animate-pulse" />
                Program
              </span>
              <span className="text-xs text-[var(--text-muted)]">{scene?.name}</span>
            </div>
          ) : (
            <span className="text-sm font-medium text-[var(--text-secondary)]">Canvas</span>
          )}

          {!studioMode && (
            <div className="flex items-center gap-1 bg-[var(--bg-sunken)] p-1 rounded-lg">
              <button
                type="button"
                className={`px-3 py-1 text-xs font-medium rounded-md transition-colors ${
                  effectiveViewMode === 'edit'
                    ? 'bg-[var(--bg-base)] text-[var(--text-primary)] shadow-sm'
                    : 'text-[var(--text-muted)] hover:text-[var(--text-secondary)]'
                }`}
                onClick={() => setViewMode('edit')}
              >
                Edit
              </button>
              <button
                type="button"
                className={`px-3 py-1 text-xs font-medium rounded-md transition-colors ${
                  effectiveViewMode === 'preview'
                    ? 'bg-[var(--bg-base)] text-[var(--text-primary)] shadow-sm'
                    : 'text-[var(--text-muted)] hover:text-[var(--text-secondary)]'
                }`}
                onClick={() => setViewMode('preview')}
                disabled={!profileName}
                title={!profileName ? 'Save profile first to enable preview' : undefined}
              >
                Preview
              </button>
            </div>
          )}
        </div>
      )}

      {/* Canvas container */}
      <div
        ref={containerRef}
        className="flex-1 flex items-center justify-center bg-[var(--bg-sunken)] cursor-default overflow-hidden"
        onClick={() => onSelectLayer(null)}
      >
        <div
          className="relative bg-[var(--bg-base)] shadow-2xl"
          style={{ width: dimensions.width, height: dimensions.height }}
        >
          {sortedLayers.map((layer) => {
            const isReadOnly = studioMode === 'program' || (effectiveViewMode === 'preview' && !studioMode);
            return (
              <LayerPreview
                key={layer.id}
                layer={layer}
                scale={scale}
                canvasWidth={scene.canvasWidth}
                canvasHeight={scene.canvasHeight}
                sourceName={getSourceName(layer.sourceId)}
                source={getSource(layer.sourceId)}
                isSelected={isReadOnly ? false : layer.id === selectedLayerId}
                onClick={isReadOnly ? () => {} : () => onSelectLayer(layer.id)}
                onTransformChange={isReadOnly ? () => {} : (transform) => handleLayerTransformChange(layer.id, transform)}
                scenes={scenes}
                sources={sources}
                readOnly={isReadOnly}
              />
            );
          })}

          <div className="absolute bottom-2 right-2 text-sm text-[var(--text-muted)] bg-[var(--bg-elevated)] px-2 py-1 rounded shadow pointer-events-none">
            {scene.canvasWidth}x{scene.canvasHeight}
            {effectiveViewMode === 'preview' && !studioMode && (
              <span className="ml-2 text-primary">Preview</span>
            )}
          </div>
        </div>
      </div>
    </Card>
  );
});
