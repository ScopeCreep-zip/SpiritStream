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
import { useRef, useState, useLayoutEffect, useMemo, useEffect, useCallback } from 'react';
import { Card } from '@/components/ui/Card';
import type { Scene, SourceLayer, Transform } from '@/types/scene';
import type { Source } from '@/types/source';
import { api } from '@/lib/backend';
import { useSceneStore } from '@/stores/sceneStore';
import { useProfileStore } from '@/stores/profileStore';
import { WebRTCPlayer } from './WebRTCPlayer';

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
}

export function SceneCanvas({
  scene,
  sources,
  selectedLayerId,
  onSelectLayer,
  profileName,
}: SceneCanvasProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const [viewMode, setViewMode] = useState<ViewMode>('edit');
  const { updateLayer } = useSceneStore();
  const { reloadProfile } = useProfileStore();

  // Calculate initial dimensions synchronously based on viewport
  const initialDimensions = useMemo(() => {
    if (!scene) return { width: 640, height: 360 };
    const estimatedWidth = Math.max(400, window.innerWidth - 260 - 448 - 64);
    const estimatedHeight = Math.max(300, window.innerHeight - 300);
    return calculateCanvasDimensions(
      scene.canvasWidth,
      scene.canvasHeight,
      estimatedWidth,
      estimatedHeight
    );
  }, [scene?.canvasWidth, scene?.canvasHeight]);

  const [dimensions, setDimensions] = useState(initialDimensions);

  useLayoutEffect(() => {
    setDimensions(initialDimensions);
  }, [initialDimensions]);

  useLayoutEffect(() => {
    if (!containerRef.current || !scene) return;

    const updateDimensions = () => {
      const container = containerRef.current;
      if (!container) return;

      const availableWidth = container.clientWidth;
      const availableHeight = container.clientHeight;

      if (availableWidth <= 0 || availableHeight <= 0) return;

      const newDims = calculateCanvasDimensions(
        scene.canvasWidth,
        scene.canvasHeight,
        availableWidth,
        availableHeight
      );

      setDimensions(newDims);
    };

    updateDimensions();
    const observer = new ResizeObserver(updateDimensions);
    observer.observe(containerRef.current);

    return () => observer.disconnect();
  }, [scene?.canvasWidth, scene?.canvasHeight]);

  // Handle layer transform updates (debounced save to backend)
  const handleLayerTransformChange = useCallback(
    async (layerId: string, newTransform: Partial<Transform>) => {
      if (!profileName || !scene) return;

      const layer = scene.layers.find((l) => l.id === layerId);
      if (!layer) return;

      try {
        await updateLayer(profileName, scene.id, layerId, {
          transform: { ...layer.transform, ...newTransform },
        });
        await reloadProfile();
      } catch (err) {
        console.error('Failed to update layer transform:', err);
      }
    },
    [profileName, scene, updateLayer, reloadProfile]
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

  const sortedLayers = [...scene.layers].sort((a, b) => a.zIndex - b.zIndex);

  const getSource = (sourceId: string) => sources.find((s) => s.id === sourceId);
  const getSourceName = (sourceId: string) => getSource(sourceId)?.name ?? 'Unknown Source';

  const scale = dimensions.width / scene.canvasWidth;

  return (
    <Card className="h-full flex flex-col overflow-hidden">
      {/* View mode toggle */}
      <div className="flex items-center justify-between px-4 py-3 border-b border-[var(--border-subtle)] bg-[var(--bg-elevated)]">
        <span className="text-sm font-medium text-[var(--text-secondary)]">Canvas</span>
        <div className="flex items-center gap-1 bg-[var(--bg-sunken)] p-1 rounded-lg">
          <button
            type="button"
            className={`px-3 py-1 text-xs font-medium rounded-md transition-colors ${
              viewMode === 'edit'
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
              viewMode === 'preview'
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
      </div>

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
          {viewMode === 'preview' && profileName ? (
            <ComposedPreview
              profileName={profileName}
              sceneId={scene.id}
              width={dimensions.width}
              height={dimensions.height}
              canvasWidth={scene.canvasWidth}
              canvasHeight={scene.canvasHeight}
            />
          ) : (
            sortedLayers.map((layer) => (
              <LayerPreview
                key={layer.id}
                layer={layer}
                scale={scale}
                canvasWidth={scene.canvasWidth}
                canvasHeight={scene.canvasHeight}
                sourceName={getSourceName(layer.sourceId)}
                source={getSource(layer.sourceId)}
                isSelected={layer.id === selectedLayerId}
                onClick={() => onSelectLayer(layer.id)}
                onTransformChange={(transform) => handleLayerTransformChange(layer.id, transform)}
              />
            ))
          )}

          {/* Canvas size indicator */}
          <div className="absolute bottom-2 right-2 text-sm text-[var(--text-muted)] bg-[var(--bg-elevated)] px-2 py-1 rounded shadow pointer-events-none">
            {scene.canvasWidth}x{scene.canvasHeight}
            {viewMode === 'preview' && (
              <span className="ml-2 text-[var(--status-live)]">Live</span>
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
}

/**
 * LayerPreview - Live preview for a layer with drag/resize support
 * Uses fixed preview dimensions for stable polling (no rate limiting)
 * Uses CSS transforms for GPU-accelerated drag/resize
 */
function LayerPreview({
  layer,
  scale,
  canvasWidth,
  canvasHeight,
  sourceName,
  source,
  isSelected,
  onClick,
  onTransformChange,
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

  // Keep refs in sync with state (runs synchronously during render)
  isDraggingRef.current = isDragging;
  isResizingRef.current = isResizing;

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

  // Global mouse move/up handlers
  useEffect(() => {
    if (!isDragging && !isResizing) return;

    const handleMouseMove = (e: MouseEvent) => {
      const deltaX = (e.clientX - dragStartRef.current.mouseX) / scale;
      const deltaY = (e.clientY - dragStartRef.current.mouseY) / scale;

      if (isDragging) {
        // Calculate new position with bounds checking
        const newX = Math.max(0, Math.min(canvasWidth - transform.width, dragStartRef.current.layerX + deltaX));
        const newY = Math.max(0, Math.min(canvasHeight - transform.height, dragStartRef.current.layerY + deltaY));
        setDragOffset({ x: newX - transform.x, y: newY - transform.y });
      } else if (isResizing) {
        let newWidth = dragStartRef.current.width;
        let newHeight = dragStartRef.current.height;
        let newX = dragStartRef.current.layerX;
        let newY = dragStartRef.current.layerY;

        // Calculate resize based on direction
        switch (isResizing) {
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
          width: newWidth - transform.width,
          height: newHeight - transform.height,
          x: newX - transform.x,
          y: newY - transform.y,
        });
      }
    };

    const handleMouseUp = () => {
      if (isDragging) {
        const newX = transform.x + dragOffset.x;
        const newY = transform.y + dragOffset.y;
        if (dragOffset.x !== 0 || dragOffset.y !== 0) {
          onTransformChange({ x: Math.round(newX), y: Math.round(newY) });
        }
        // Don't reset dragOffset here - the useEffect will reset it when transform updates
        setIsDragging(false);
      }

      if (isResizing) {
        const newWidth = transform.width + resizeOffset.width;
        const newHeight = transform.height + resizeOffset.height;
        const newX = transform.x + resizeOffset.x;
        const newY = transform.y + resizeOffset.y;
        if (resizeOffset.width !== 0 || resizeOffset.height !== 0) {
          onTransformChange({
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
  }, [isDragging, isResizing, scale, transform, dragOffset, resizeOffset, canvasWidth, canvasHeight, onTransformChange]);

  if (!visible) return null;

  // Calculate display position with drag/resize offsets (GPU-accelerated via transform)
  const displayX = (transform.x + dragOffset.x + resizeOffset.x) * scale;
  const displayY = (transform.y + dragOffset.y + resizeOffset.y) * scale;
  const displayWidth = (transform.width + resizeOffset.width) * scale;
  const displayHeight = (transform.height + resizeOffset.height) * scale;

  return (
    <div
      className={`absolute transition-shadow ${
        layer.locked ? 'cursor-not-allowed' : 'cursor-move'
      } ${
        isSelected ? 'ring-2 ring-primary shadow-lg' : 'hover:ring-1 hover:ring-primary/30'
      } ${isDragging || isResizing ? 'cursor-grabbing' : ''}`}
      style={{
        left: 0,
        top: 0,
        width: displayWidth,
        height: displayHeight,
        transform: `translate(${displayX}px, ${displayY}px) ${transform.rotation ? `rotate(${transform.rotation}deg)` : ''}`,
        willChange: isDragging || isResizing ? 'transform, width, height' : 'auto',
      }}
      onClick={(e) => {
        e.stopPropagation();
        onClick();
      }}
      onMouseDown={handleDragStart}
    >
      {/* Live preview via WebRTC */}
      <div className="w-full h-full bg-[var(--bg-sunken)] overflow-hidden pointer-events-none">
        {hasVideo && source ? (
          <WebRTCPlayer
            sourceId={source.id}
            sourceName={sourceName}
            width={displayWidth}
            height={displayHeight}
            refreshKey={'deviceId' in source ? source.deviceId : 'displayId' in source ? source.displayId : undefined}
          />
        ) : (
          <div className="w-full h-full bg-gradient-to-br from-[var(--bg-elevated)] to-[var(--bg-sunken)] flex items-center justify-center">
            <span className="text-[var(--text-muted)] text-xs text-center px-2 truncate">
              {sourceName}
            </span>
          </div>
        )}
      </div>

      {/* Resize handles - only show when selected and not locked */}
      {isSelected && !layer.locked && (
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
}

interface ComposedPreviewProps {
  profileName: string;
  sceneId: string;
  width: number;
  height: number;
  canvasWidth: number;
  canvasHeight: number;
}

/**
 * ComposedPreview - Shows the composed scene output from the backend
 * Uses fixed preview dimensions for stable polling
 */
function ComposedPreview({
  profileName,
  sceneId,
  width,
  height,
  canvasWidth,
  canvasHeight,
}: ComposedPreviewProps) {
  const [previewError, setPreviewError] = useState(false);
  const [previewLoading, setPreviewLoading] = useState(true);
  const [snapshotUrl, setSnapshotUrl] = useState<string | null>(null);

  const timeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const errorCountRef = useRef(0);
  const isPendingRef = useRef(false);
  const backoffDelayRef = useRef(150);
  const mountedRef = useRef(true);

  // Use fixed preview dimensions
  const previewWidth = Math.min(Math.max(width * 2, 640), 1920);
  const previewHeight = Math.min(Math.max(height * 2, 360), 1080);

  useEffect(() => {
    mountedRef.current = true;
    setPreviewError(false);
    setPreviewLoading(true);
    errorCountRef.current = 0;
    backoffDelayRef.current = 150;
    isPendingRef.current = false;

    const fetchSnapshot = () => {
      if (isPendingRef.current || !mountedRef.current) {
        timeoutRef.current = setTimeout(fetchSnapshot, backoffDelayRef.current);
        return;
      }

      const url = api.preview.getSceneSnapshotUrl(profileName, sceneId, previewWidth, previewHeight, 3);
      isPendingRef.current = true;
      setSnapshotUrl(url);
    };

    fetchSnapshot();

    return () => {
      mountedRef.current = false;
      isPendingRef.current = false;
      if (timeoutRef.current) {
        clearTimeout(timeoutRef.current);
        timeoutRef.current = null;
      }
    };
  }, [profileName, sceneId]); // Only depend on profile/scene, not dimensions

  const handleLoad = useCallback(() => {
    if (!mountedRef.current) return;

    isPendingRef.current = false;
    setPreviewLoading(false);
    setPreviewError(false);
    errorCountRef.current = 0;
    backoffDelayRef.current = 150;

    if (timeoutRef.current) clearTimeout(timeoutRef.current);
    timeoutRef.current = setTimeout(() => {
      if (mountedRef.current) {
        const url = api.preview.getSceneSnapshotUrl(profileName, sceneId, previewWidth, previewHeight, 3);
        isPendingRef.current = true;
        setSnapshotUrl(url);
      }
    }, backoffDelayRef.current);
  }, [profileName, sceneId, previewWidth, previewHeight]);

  const handleError = useCallback(() => {
    if (!mountedRef.current) return;

    isPendingRef.current = false;
    errorCountRef.current += 1;
    backoffDelayRef.current = Math.min(backoffDelayRef.current * 1.5, 5000);

    if (errorCountRef.current >= 5) {
      setPreviewLoading(false);
      setPreviewError(true);
      if (timeoutRef.current) {
        clearTimeout(timeoutRef.current);
        timeoutRef.current = null;
      }
      return;
    }

    if (timeoutRef.current) clearTimeout(timeoutRef.current);
    timeoutRef.current = setTimeout(() => {
      if (mountedRef.current) {
        const url = api.preview.getSceneSnapshotUrl(profileName, sceneId, previewWidth, previewHeight, 3);
        isPendingRef.current = true;
        setSnapshotUrl(url);
      }
    }, backoffDelayRef.current);
  }, [profileName, sceneId, previewWidth, previewHeight]);

  if (previewError) {
    return (
      <div className="w-full h-full bg-gradient-to-br from-[var(--bg-elevated)] to-[var(--bg-sunken)] flex flex-col items-center justify-center gap-2">
        <svg
          xmlns="http://www.w3.org/2000/svg"
          className="w-8 h-8 text-[var(--text-muted)]"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth="2"
        >
          <path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z" />
          <line x1="12" y1="9" x2="12" y2="13" />
          <line x1="12" y1="17" x2="12.01" y2="17" />
        </svg>
        <span className="text-[var(--text-muted)] text-sm">Preview unavailable</span>
        <span className="text-[var(--text-muted)] text-xs opacity-75">
          {canvasWidth}x{canvasHeight}
        </span>
        <button
          type="button"
          className="mt-2 px-3 py-1 text-xs bg-[var(--bg-elevated)] rounded-md hover:bg-[var(--bg-base)] transition-colors"
          onClick={() => {
            setPreviewError(false);
            setPreviewLoading(true);
            errorCountRef.current = 0;
            backoffDelayRef.current = 150;
            const url = api.preview.getSceneSnapshotUrl(profileName, sceneId, previewWidth, previewHeight, 3);
            isPendingRef.current = true;
            setSnapshotUrl(url);
          }}
        >
          Retry
        </button>
      </div>
    );
  }

  return (
    <>
      {previewLoading && (
        <div className="absolute inset-0 flex items-center justify-center bg-gradient-to-br from-[var(--bg-elevated)] to-[var(--bg-sunken)]">
          <div className="flex flex-col items-center gap-2">
            <div className="w-8 h-8 border-2 border-primary/30 border-t-primary rounded-full animate-spin" />
            <span className="text-[var(--text-muted)] text-xs">Loading composed preview...</span>
          </div>
        </div>
      )}
      {snapshotUrl && (
        <img
          src={snapshotUrl}
          alt="Composed scene preview"
          className="w-full h-full object-contain"
          onLoad={handleLoad}
          onError={handleError}
        />
      )}
    </>
  );
}
