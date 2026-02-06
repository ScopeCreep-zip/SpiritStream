/**
 * Scene Canvas
 * Central composition canvas showing layers with drag/resize
 *
 * Supports two view modes:
 * - Edit: Shows individual layer previews with resize handles
 * - Preview: Shows composed scene output from backend Compositor
 *
 * Features:
 * - Drag layers to reposition
 * - Resize layers via corner handles
 * - GPU-accelerated movement via CSS transforms
 * - Adaptive framerate preview polling (no rate limiting)
 */
import React, { useRef, useState, useLayoutEffect, useEffect, useCallback, useMemo } from 'react';
import { Card } from '@/components/ui/Card';
import type { Scene, SourceLayer, Transform } from '@/types/scene';
import type { Source } from '@/types/source';
import { useSceneStore } from '@/stores/sceneStore';
import { useProfileStore } from '@/stores/profileStore';
import { WorkerVideoPreview } from './WorkerVideoPreview';
import { StaticMediaPlayer } from './StaticMediaPlayer';
import { TextSourceRenderer } from './TextSourceRenderer';
import { BrowserSourceRenderer } from './BrowserSourceRenderer';
import { NestedSceneRenderer } from './NestedSceneRenderer';
import { MediaPlaylistRenderer } from './MediaPlaylistRenderer';
import { isStaticMediaFile, isImageFile } from '@/lib/mediaTypes';
import type { ColorSource, TextSource, BrowserSource, NestedSceneSource, MediaPlaylistSource } from '@/types/source';

type ViewMode = 'edit' | 'preview';
type ResizeDirection = 'nw' | 'ne' | 'sw' | 'se' | null;

// Calculate canvas dimensions that fit within available space
function calculateCanvasDimensions(
  canvasWidth: number,
  canvasHeight: number,
  availableWidth: number,
  availableHeight: number
): { width: number; height: number } {
  const aspectRatio = canvasWidth / canvasHeight;
  let width = availableWidth;
  let height = width / aspectRatio;

  if (height > availableHeight) {
    height = availableHeight;
    width = height * aspectRatio;
  }

  if (width > canvasWidth) {
    width = canvasWidth;
    height = canvasHeight;
  }

  return { width: Math.floor(width), height: Math.floor(height) };
}

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

export function SceneCanvas({
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
  // In Studio Mode: Program pane always shows composed preview, Preview pane defaults to preview but can toggle
  // In Normal Mode: defaults to edit view
  const [viewMode, setViewMode] = useState<ViewMode>(studioMode ? 'preview' : 'edit');
  const { updateLayer } = useSceneStore();
  const { updateCurrentLayer } = useProfileStore();

  // Force preview mode in Program pane (can't edit live output)
  const effectiveViewMode = studioMode === 'program' ? 'preview' : viewMode;

  // Start with small default dimensions - the ResizeObserver will correct them
  // This prevents the initial render from causing layout expansion
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

      // If container hasn't been laid out yet, schedule a retry
      if (availableWidth <= 0 || availableHeight <= 0) {
        if (retryTimeout) clearTimeout(retryTimeout);
        retryTimeout = setTimeout(updateDimensions, 16); // Retry next frame
        return;
      }

      const newDims = calculateCanvasDimensions(
        scene.canvasWidth,
        scene.canvasHeight,
        availableWidth,
        availableHeight
      );

      // Only update if dimensions actually changed (prevents unnecessary re-renders)
      setDimensions(prev => {
        if (prev.width === newDims.width && prev.height === newDims.height) {
          return prev;
        }
        return newDims;
      });
    };

    // Calculate dimensions synchronously on mount - useLayoutEffect runs before paint
    // so this ensures the correct size on the first visible render
    updateDimensions();

    // Use ResizeObserver for subsequent changes, with requestAnimationFrame for batching
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

  // Handle layer transform updates (debounced save to backend)
  const handleLayerTransformChange = useCallback(
    async (layerId: string, newTransform: Partial<Transform>) => {
      if (!profileName || !scene) return;

      const layer = scene.layers.find((l) => l.id === layerId);
      if (!layer) return;

      const updatedTransform = { ...layer.transform, ...newTransform };

      try {
        await updateLayer(profileName, scene.id, layerId, { transform: updatedTransform });
        // Update local state instead of reloading entire profile
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
      {/* Header with view mode toggle - hidden for projector/fullscreen */}
      {!hideHeader && (
        <div className="flex items-center justify-between px-4 py-3 border-b border-[var(--border-subtle)] bg-[var(--bg-elevated)]">
          {/* Left side: Title/indicator */}
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

          {/* Right side: Edit/Preview toggle (hidden in Studio Mode - both panes render layers) */}
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
          {/* Render layers - read-only in Preview mode (outside Studio) or Program mode (Studio) */}
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

          {/* Canvas size indicator */}
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
}

interface LayerPreviewProps {
  layer: SourceLayer;
  scale: number;
  canvasWidth: number;
  canvasHeight: number;
  sourceName: string;
  source: Source | undefined;
  isSelected: boolean;
  onClick: () => void;
  onTransformChange: (transform: Partial<Transform>) => void;
  /** All scenes in the profile (for nested scene rendering) */
  scenes: Scene[];
  /** All sources in the profile (for nested scene rendering) */
  sources: Source[];
  /** Read-only mode (no selection ring, no drag handles) - used in Program pane */
  readOnly?: boolean;
}

/**
 * LayerPreview - Live preview for a layer with drag/resize support
 * Uses fixed preview dimensions for stable polling (no rate limiting)
 * Uses CSS transforms for GPU-accelerated drag/resize
 * Memoized to prevent unnecessary re-renders
 */
const LayerPreview = React.memo(function LayerPreview({
  layer,
  scale,
  canvasWidth,
  canvasHeight,
  sourceName,
  source,
  isSelected,
  onClick,
  onTransformChange,
  scenes,
  sources,
  readOnly = false,
}: LayerPreviewProps) {
  const { transform, visible } = layer;

  // Drag/resize state
  const [isDragging, setIsDragging] = useState(false);
  const [isResizing, setIsResizing] = useState<ResizeDirection>(null);
  const [dragOffset, setDragOffset] = useState({ x: 0, y: 0 });
  const [resizeOffset, setResizeOffset] = useState({ width: 0, height: 0, x: 0, y: 0 });
  const dragStartRef = useRef({ mouseX: 0, mouseY: 0, layerX: 0, layerY: 0, width: 0, height: 0 });

  // Use refs to track dragging state for the reset effect
  // This allows us to check the state without including it in dependencies
  const isDraggingRef = useRef(false);
  const isResizingRef = useRef<ResizeDirection>(null);

  // Refs for values read inside the global mouse event handlers
  // Prevents useEffect from re-registering listeners 30-60x/sec during drag
  const dragOffsetRef = useRef(dragOffset);
  const resizeOffsetRef = useRef(resizeOffset);
  const transformRef = useRef(transform);
  const onTransformChangeRef = useRef(onTransformChange);

  // Keep refs in sync with state (runs synchronously during render)
  isDraggingRef.current = isDragging;
  isResizingRef.current = isResizing;
  dragOffsetRef.current = dragOffset;
  resizeOffsetRef.current = resizeOffset;
  transformRef.current = transform;
  onTransformChangeRef.current = onTransformChange;

  // Reset offsets only when transform actually changes from server
  // Using refs to check dragging state prevents the effect from running when dragging state changes
  useEffect(() => {
    if (!isDraggingRef.current && !isResizingRef.current) {
      setDragOffset({ x: 0, y: 0 });
      setResizeOffset({ width: 0, height: 0, x: 0, y: 0 });
    }
  }, [transform.x, transform.y, transform.width, transform.height]);

  const hasVideo = source?.type !== 'audioDevice';

  // Drag handlers
  const handleDragStart = useCallback(
    (e: React.MouseEvent) => {
      if (!isSelected || isResizing || layer.locked) return;
      e.preventDefault();
      e.stopPropagation();

      // Capture the current visual position (including any pending offset from previous drag)
      // This prevents jumps when starting a new drag before server responds
      const visualX = transform.x + dragOffset.x;
      const visualY = transform.y + dragOffset.y;

      setIsDragging(true);
      dragStartRef.current = {
        mouseX: e.clientX,
        mouseY: e.clientY,
        layerX: visualX,
        layerY: visualY,
        width: transform.width + resizeOffset.width,
        height: transform.height + resizeOffset.height,
      };
    },
    [isSelected, isResizing, transform, layer.locked, dragOffset, resizeOffset]
  );

  const handleResizeStart = useCallback(
    (e: React.MouseEvent, direction: ResizeDirection) => {
      if (layer.locked) return;
      e.preventDefault();
      e.stopPropagation();

      // Capture the current visual position and size (including any pending offsets)
      const visualX = transform.x + dragOffset.x + resizeOffset.x;
      const visualY = transform.y + dragOffset.y + resizeOffset.y;
      const visualWidth = transform.width + resizeOffset.width;
      const visualHeight = transform.height + resizeOffset.height;

      setIsResizing(direction);
      dragStartRef.current = {
        mouseX: e.clientX,
        mouseY: e.clientY,
        layerX: visualX,
        layerY: visualY,
        width: visualWidth,
        height: visualHeight,
      };
    },
    [transform, layer.locked, dragOffset, resizeOffset]
  );

  // Global mouse move/up handlers with RAF throttling
  // Reads transform/offset values from refs so this effect only re-runs when
  // isDragging/isResizing change (not every frame during drag)
  useEffect(() => {
    if (!isDragging && !isResizing) return;

    // RAF-based throttling to limit processing to display refresh rate
    let rafPending = false;
    let lastClientX = 0;
    let lastClientY = 0;

    const processMouseMove = () => {
      const deltaX = (lastClientX - dragStartRef.current.mouseX) / scale;
      const deltaY = (lastClientY - dragStartRef.current.mouseY) / scale;
      const t = transformRef.current;

      if (isDraggingRef.current) {
        // Calculate new position with bounds checking
        const newX = Math.max(0, Math.min(canvasWidth - t.width, dragStartRef.current.layerX + deltaX));
        const newY = Math.max(0, Math.min(canvasHeight - t.height, dragStartRef.current.layerY + deltaY));
        setDragOffset({ x: newX - t.x, y: newY - t.y });
      } else if (isResizingRef.current) {
        let newWidth = dragStartRef.current.width;
        let newHeight = dragStartRef.current.height;
        let newX = dragStartRef.current.layerX;
        let newY = dragStartRef.current.layerY;

        // Calculate resize based on direction
        switch (isResizingRef.current) {
          case 'se':
            newWidth = Math.max(50, dragStartRef.current.width + deltaX);
            newHeight = Math.max(50, dragStartRef.current.height + deltaY);
            break;
          case 'sw':
            newWidth = Math.max(50, dragStartRef.current.width - deltaX);
            newHeight = Math.max(50, dragStartRef.current.height + deltaY);
            newX = dragStartRef.current.layerX + (dragStartRef.current.width - newWidth);
            break;
          case 'ne':
            newWidth = Math.max(50, dragStartRef.current.width + deltaX);
            newHeight = Math.max(50, dragStartRef.current.height - deltaY);
            newY = dragStartRef.current.layerY + (dragStartRef.current.height - newHeight);
            break;
          case 'nw':
            newWidth = Math.max(50, dragStartRef.current.width - deltaX);
            newHeight = Math.max(50, dragStartRef.current.height - deltaY);
            newX = dragStartRef.current.layerX + (dragStartRef.current.width - newWidth);
            newY = dragStartRef.current.layerY + (dragStartRef.current.height - newHeight);
            break;
        }

        // Clamp to canvas bounds
        newX = Math.max(0, Math.min(canvasWidth - 50, newX));
        newY = Math.max(0, Math.min(canvasHeight - 50, newY));
        newWidth = Math.min(newWidth, canvasWidth - newX);
        newHeight = Math.min(newHeight, canvasHeight - newY);

        setResizeOffset({
          width: newWidth - t.width,
          height: newHeight - t.height,
          x: newX - t.x,
          y: newY - t.y,
        });
      }
      rafPending = false;
    };

    const handleMouseMove = (e: MouseEvent) => {
      lastClientX = e.clientX;
      lastClientY = e.clientY;
      if (rafPending) return;
      rafPending = true;
      requestAnimationFrame(processMouseMove);
    };

    const handleMouseUp = () => {
      const t = transformRef.current;

      if (isDraggingRef.current) {
        const dOff = dragOffsetRef.current;
        const newX = t.x + dOff.x;
        const newY = t.y + dOff.y;
        if (dOff.x !== 0 || dOff.y !== 0) {
          onTransformChangeRef.current({ x: Math.round(newX), y: Math.round(newY) });
        }
        // Don't reset dragOffset here - the useEffect will reset it when transform updates
        setIsDragging(false);
      }

      if (isResizingRef.current) {
        const rOff = resizeOffsetRef.current;
        const newWidth = t.width + rOff.width;
        const newHeight = t.height + rOff.height;
        const newX = t.x + rOff.x;
        const newY = t.y + rOff.y;
        if (rOff.width !== 0 || rOff.height !== 0) {
          onTransformChangeRef.current({
            x: Math.round(newX),
            y: Math.round(newY),
            width: Math.round(newWidth),
            height: Math.round(newHeight),
          });
        }
        // Don't reset resizeOffset here - the useEffect will reset it when transform updates
        setIsResizing(null);
      }
    };

    window.addEventListener('mousemove', handleMouseMove);
    window.addEventListener('mouseup', handleMouseUp);

    return () => {
      window.removeEventListener('mousemove', handleMouseMove);
      window.removeEventListener('mouseup', handleMouseUp);
    };
  }, [isDragging, isResizing, scale, canvasWidth, canvasHeight]);

  if (!visible) return null;

  // Calculate display position with drag/resize offsets (GPU-accelerated via transform)
  const displayX = (transform.x + dragOffset.x + resizeOffset.x) * scale;
  const displayY = (transform.y + dragOffset.y + resizeOffset.y) * scale;
  const displayWidth = (transform.width + resizeOffset.width) * scale;
  const displayHeight = (transform.height + resizeOffset.height) * scale;

  return (
    <div
      className={`absolute transition-shadow ${
        readOnly ? 'cursor-default' : layer.locked ? 'cursor-not-allowed' : 'cursor-move'
      } ${
        !readOnly && isSelected ? 'ring-2 ring-primary shadow-lg' : !readOnly ? 'hover:ring-1 hover:ring-primary/30' : ''
      } ${!readOnly && (isDragging || isResizing) ? 'cursor-grabbing' : ''}`}
      style={{
        left: 0,
        top: 0,
        width: displayWidth,
        height: displayHeight,
        transform: `translate(${displayX}px, ${displayY}px) ${transform.rotation ? `rotate(${transform.rotation}deg)` : ''}`,
        willChange: isDragging || isResizing ? 'transform, width, height' : 'auto',
      }}
      onClick={(e) => {
        if (readOnly) return;
        e.stopPropagation();
        onClick();
      }}
      onMouseDown={readOnly ? undefined : handleDragStart}
    >
      {/* Live preview via shared WebRTC, CSS rendering, or static rendering for images/HTML */}
      <div className="w-full h-full bg-[var(--bg-sunken)] overflow-hidden pointer-events-none">
        {hasVideo && source ? (
          // Color source - pure CSS rendering
          source.type === 'color' ? (
            <div
              style={{
                backgroundColor: (source as ColorSource).color,
                opacity: (source as ColorSource).opacity,
                width: '100%',
                height: '100%',
              }}
            />
          ) : // Text source - CSS rendering
          source.type === 'text' ? (
            <TextSourceRenderer
              source={source as TextSource}
              width={displayWidth}
              height={displayHeight}
            />
          ) : // Browser source - iframe rendering
          source.type === 'browser' ? (
            <BrowserSourceRenderer
              source={source as BrowserSource}
              width={displayWidth}
              height={displayHeight}
            />
          ) : // Nested scene - recursive scene rendering
          source.type === 'nestedScene' ? (
            <NestedSceneRenderer
              source={source as NestedSceneSource}
              scenes={scenes}
              sources={sources}
              width={displayWidth}
              height={displayHeight}
            />
          ) : // Media playlist - client-side playlist playback
          source.type === 'mediaPlaylist' ? (
            <MediaPlaylistRenderer
              source={source as MediaPlaylistSource}
              isLayerPreview
            />
          ) : // Static media file (image/HTML) - no WebRTC needed
          source.type === 'mediaFile' && 'filePath' in source && isStaticMediaFile(source.filePath) ? (
            <StaticMediaPlayer
              filePath={source.filePath}
              isImage={isImageFile(source.filePath)}
              width={displayWidth}
              height={displayHeight}
              sourceName={sourceName}
              nativeWidth={canvasWidth}
              nativeHeight={canvasHeight}
            />
          ) : (
            // All other sources use WebRTC via worker for low-latency rendering
            <WorkerVideoPreview
              sourceId={source.id}
              sourceName={sourceName}
              sourceType={source.type}
              width={displayWidth}
              height={displayHeight}
            />
          )
        ) : (
          <div className="w-full h-full bg-gradient-to-br from-[var(--bg-elevated)] to-[var(--bg-sunken)] flex items-center justify-center">
            <span className="text-[var(--text-muted)] text-xs text-center px-2 truncate">
              {sourceName}
            </span>
          </div>
        )}
      </div>

      {/* Resize handles - only show when selected, not locked, and not read-only */}
      {isSelected && !layer.locked && !readOnly && (
        <>
          <div
            className="absolute -top-2 -left-2 w-6 h-6 flex items-center justify-center cursor-nw-resize z-10"
            onMouseDown={(e) => handleResizeStart(e, 'nw')}
          >
            <div className="w-4 h-4 bg-primary rounded-full border-2 border-primary-foreground shadow-md" />
          </div>
          <div
            className="absolute -top-2 -right-2 w-6 h-6 flex items-center justify-center cursor-ne-resize z-10"
            onMouseDown={(e) => handleResizeStart(e, 'ne')}
          >
            <div className="w-4 h-4 bg-primary rounded-full border-2 border-primary-foreground shadow-md" />
          </div>
          <div
            className="absolute -bottom-2 -left-2 w-6 h-6 flex items-center justify-center cursor-sw-resize z-10"
            onMouseDown={(e) => handleResizeStart(e, 'sw')}
          >
            <div className="w-4 h-4 bg-primary rounded-full border-2 border-primary-foreground shadow-md" />
          </div>
          <div
            className="absolute -bottom-2 -right-2 w-6 h-6 flex items-center justify-center cursor-se-resize z-10"
            onMouseDown={(e) => handleResizeStart(e, 'se')}
          >
            <div className="w-4 h-4 bg-primary rounded-full border-2 border-primary-foreground shadow-md" />
          </div>
        </>
      )}
    </div>
  );
});

