/**
 * Scene Canvas
 * Central composition canvas showing layers with drag/resize
 *
 * Supports two view modes:
 * - Edit: Shows individual layer previews with resize handles
 * - Preview: Shows composed scene output from backend Compositor
 *
 * Prevents layout flash by calculating initial dimensions synchronously
 * based on viewport size, then refining with ResizeObserver.
 */
import { useRef, useState, useLayoutEffect, useMemo, useEffect, useCallback } from 'react';
import { Card } from '@/components/ui/Card';
import type { Scene, SourceLayer } from '@/types/scene';
import type { Source } from '@/types/source';
import { api } from '@/lib/backend';

type ViewMode = 'edit' | 'preview';

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

  // If too tall, constrain by height
  if (height > availableHeight) {
    height = availableHeight;
    width = height * aspectRatio;
  }

  // Don't exceed native resolution
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

  // Calculate initial dimensions synchronously based on viewport
  // This prevents flash by having a reasonable size immediately
  const initialDimensions = useMemo(() => {
    if (!scene) return { width: 640, height: 360 };
    // Estimate available space: viewport minus sidebar (260px), panels (2*224px), gaps, padding
    const estimatedWidth = Math.max(400, window.innerWidth - 260 - 448 - 64);
    const estimatedHeight = Math.max(300, window.innerHeight - 300); // Header, bars, mixer
    return calculateCanvasDimensions(
      scene.canvasWidth,
      scene.canvasHeight,
      estimatedWidth,
      estimatedHeight
    );
  }, [scene?.canvasWidth, scene?.canvasHeight]);

  const [dimensions, setDimensions] = useState(initialDimensions);

  // Reset dimensions when scene changes (initialDimensions recalculated by useMemo)
  useLayoutEffect(() => {
    setDimensions(initialDimensions);
  }, [initialDimensions]);

  // Refine dimensions when container is measured
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

  const getSource = (sourceId: string) => {
    return sources.find((s) => s.id === sourceId);
  };

  const getSourceName = (sourceId: string) => {
    return getSource(sourceId)?.name ?? 'Unknown Source';
  };

  // Calculate scale for layer positioning
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

      {/* Container for measurement */}
      <div
        ref={containerRef}
        className="flex-1 flex items-center justify-center bg-[var(--bg-sunken)] cursor-default overflow-hidden"
        onClick={() => onSelectLayer(null)}
      >
        {/*
          Canvas with explicit dimensions.
          Initial dimensions are calculated synchronously from viewport,
          then refined by ResizeObserver. This prevents the flash because
          we always have reasonable dimensions, never 0x0.
        */}
        <div
          className="relative bg-[var(--bg-base)] shadow-2xl"
          style={{
            width: dimensions.width,
            height: dimensions.height,
          }}
        >
          {viewMode === 'preview' && profileName ? (
            /* Preview mode: composed scene from backend */
            <ComposedPreview
              profileName={profileName}
              sceneId={scene.id}
              width={dimensions.width}
              height={dimensions.height}
              canvasWidth={scene.canvasWidth}
              canvasHeight={scene.canvasHeight}
            />
          ) : (
            /* Edit mode: individual layer previews */
            sortedLayers.map((layer) => (
              <LayerPreview
                key={layer.id}
                layer={layer}
                scale={scale}
                sourceName={getSourceName(layer.sourceId)}
                source={getSource(layer.sourceId)}
                isSelected={layer.id === selectedLayerId}
                onClick={() => onSelectLayer(layer.id)}
              />
            ))
          )}

          {/* Canvas size indicator */}
          <div className="absolute bottom-2 right-2 text-sm text-[var(--text-muted)] bg-[var(--bg-elevated)] px-2 py-1 rounded shadow">
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
  sourceName: string;
  source: Source | undefined;
  isSelected: boolean;
  onClick: () => void;
}

/**
 * LayerPreview - Live preview for a layer in the scene canvas
 * Uses snapshot polling (like SourcesPanel) for reliable WebKit compatibility
 */
function LayerPreview({
  layer,
  scale,
  sourceName,
  source,
  isSelected,
  onClick,
}: LayerPreviewProps) {
  const { transform, visible } = layer;
  const [previewError, setPreviewError] = useState(false);
  const [previewLoading, setPreviewLoading] = useState(true);
  const [snapshotUrl, setSnapshotUrl] = useState<string | null>(null);

  const timeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const errorCountRef = useRef(0);
  const isPendingRef = useRef(false);
  const backoffDelayRef = useRef(100); // Start faster for canvas (100ms = 10fps)
  const mountedRef = useRef(true);

  // Check if source has video
  const hasVideo = source?.type !== 'audioDevice';

  // Calculate preview dimensions based on layer size
  // Use higher resolution for better quality (browser will downscale smoothly)
  // Request 2x the display size for retina/HiDPI displays, capped at 1280x720
  const previewWidth = Math.min(Math.max(Math.round(transform.width * scale * 2), 320), 1280);
  const previewHeight = Math.min(Math.max(Math.round(transform.height * scale * 2), 180), 720);

  // Snapshot polling with pending tracking and exponential backoff
  useEffect(() => {
    if (!hasVideo || !source) return;

    // Reset state on mount
    mountedRef.current = true;
    setPreviewError(false);
    setPreviewLoading(true);
    errorCountRef.current = 0;
    backoffDelayRef.current = 100;
    isPendingRef.current = false;

    const fetchSnapshot = () => {
      // Don't start new request if one is already pending
      if (isPendingRef.current || !mountedRef.current) {
        timeoutRef.current = setTimeout(fetchSnapshot, backoffDelayRef.current);
        return;
      }

      // Generate URL with timestamp to prevent caching
      const url = api.preview.getSourceSnapshotUrl(source.id, previewWidth, previewHeight, 2);
      isPendingRef.current = true;
      setSnapshotUrl(url);
    };

    // Fetch first snapshot immediately
    fetchSnapshot();

    return () => {
      mountedRef.current = false;
      isPendingRef.current = false;
      if (timeoutRef.current) {
        clearTimeout(timeoutRef.current);
        timeoutRef.current = null;
      }
    };
  }, [source?.id, hasVideo, previewWidth, previewHeight]);

  const handleLoad = useCallback(() => {
    if (!mountedRef.current || !source) return;

    isPendingRef.current = false;
    setPreviewLoading(false);
    setPreviewError(false);
    errorCountRef.current = 0;
    // Reset backoff on success
    backoffDelayRef.current = 100;

    // Schedule next fetch
    if (timeoutRef.current) clearTimeout(timeoutRef.current);
    timeoutRef.current = setTimeout(() => {
      if (mountedRef.current && source) {
        const url = api.preview.getSourceSnapshotUrl(source.id, previewWidth, previewHeight, 2);
        isPendingRef.current = true;
        setSnapshotUrl(url);
      }
    }, backoffDelayRef.current);
  }, [source?.id, previewWidth, previewHeight]);

  const handleError = useCallback(() => {
    if (!mountedRef.current || !source) return;

    isPendingRef.current = false;
    errorCountRef.current += 1;

    // Exponential backoff on errors (max 3 seconds for canvas)
    backoffDelayRef.current = Math.min(backoffDelayRef.current * 1.5, 3000);

    // Only show error after 5 consecutive failures
    if (errorCountRef.current >= 5) {
      setPreviewLoading(false);
      setPreviewError(true);
      // Stop polling on persistent error
      if (timeoutRef.current) {
        clearTimeout(timeoutRef.current);
        timeoutRef.current = null;
      }
      return;
    }

    // Schedule retry with backoff
    if (timeoutRef.current) clearTimeout(timeoutRef.current);
    timeoutRef.current = setTimeout(() => {
      if (mountedRef.current && source) {
        const url = api.preview.getSourceSnapshotUrl(source.id, previewWidth, previewHeight, 2);
        isPendingRef.current = true;
        setSnapshotUrl(url);
      }
    }, backoffDelayRef.current);
  }, [source?.id, previewWidth, previewHeight]);

  if (!visible) return null;

  return (
    <div
      className={`absolute cursor-pointer transition-shadow ${
        isSelected ? 'ring-2 ring-primary shadow-lg' : 'hover:ring-1 hover:ring-primary/30'
      }`}
      style={{
        left: transform.x * scale,
        top: transform.y * scale,
        width: transform.width * scale,
        height: transform.height * scale,
        transform: transform.rotation ? `rotate(${transform.rotation}deg)` : undefined,
      }}
      onClick={(e) => {
        e.stopPropagation();
        onClick();
      }}
    >
      {/* Live preview or fallback */}
      <div className="w-full h-full bg-[var(--bg-sunken)] overflow-hidden">
        {hasVideo && !previewError ? (
          <>
            {previewLoading && (
              <div className="absolute inset-0 flex items-center justify-center bg-gradient-to-br from-[var(--bg-elevated)] to-[var(--bg-sunken)]">
                <div className="w-6 h-6 border-2 border-primary/30 border-t-primary rounded-full animate-spin" />
              </div>
            )}
            {snapshotUrl && (
              <img
                src={snapshotUrl}
                alt={sourceName}
                className="w-full h-full object-cover"
                onLoad={handleLoad}
                onError={handleError}
              />
            )}
          </>
        ) : (
          // Fallback placeholder for errors or audio-only sources
          <div className="w-full h-full bg-gradient-to-br from-[var(--bg-elevated)] to-[var(--bg-sunken)] flex items-center justify-center">
            <span className="text-[var(--text-muted)] text-xs text-center px-2 truncate">
              {sourceName}
            </span>
          </div>
        )}
      </div>

      {/* Selection handles - larger for better grabbing (16px visible, 24px hit area) */}
      {isSelected && (
        <>
          {/* Top-left handle */}
          <div className="absolute -top-2 -left-2 w-6 h-6 flex items-center justify-center cursor-nw-resize">
            <div className="w-4 h-4 bg-primary rounded-full border-2 border-primary-foreground shadow-md" />
          </div>
          {/* Top-right handle */}
          <div className="absolute -top-2 -right-2 w-6 h-6 flex items-center justify-center cursor-ne-resize">
            <div className="w-4 h-4 bg-primary rounded-full border-2 border-primary-foreground shadow-md" />
          </div>
          {/* Bottom-left handle */}
          <div className="absolute -bottom-2 -left-2 w-6 h-6 flex items-center justify-center cursor-sw-resize">
            <div className="w-4 h-4 bg-primary rounded-full border-2 border-primary-foreground shadow-md" />
          </div>
          {/* Bottom-right handle */}
          <div className="absolute -bottom-2 -right-2 w-6 h-6 flex items-center justify-center cursor-se-resize">
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
 * Uses snapshot polling similar to LayerPreview for reliable rendering
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

  // Request preview at display resolution (2x for retina), capped at 1920x1080
  const previewWidth = Math.min(Math.max(width * 2, 640), 1920);
  const previewHeight = Math.min(Math.max(height * 2, 360), 1080);

  // Snapshot polling with exponential backoff on errors
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

      // Generate URL with timestamp to prevent caching
      const url = api.preview.getSceneSnapshotUrl(
        profileName,
        sceneId,
        previewWidth,
        previewHeight,
        3 // Quality
      );
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
  }, [profileName, sceneId, previewWidth, previewHeight]);

  const handleLoad = useCallback(() => {
    if (!mountedRef.current) return;

    isPendingRef.current = false;
    setPreviewLoading(false);
    setPreviewError(false);
    errorCountRef.current = 0;
    backoffDelayRef.current = 150;

    // Schedule next fetch
    if (timeoutRef.current) clearTimeout(timeoutRef.current);
    timeoutRef.current = setTimeout(() => {
      if (mountedRef.current) {
        const url = api.preview.getSceneSnapshotUrl(
          profileName,
          sceneId,
          previewWidth,
          previewHeight,
          3
        );
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

    // Show error after 5 consecutive failures
    if (errorCountRef.current >= 5) {
      setPreviewLoading(false);
      setPreviewError(true);
      if (timeoutRef.current) {
        clearTimeout(timeoutRef.current);
        timeoutRef.current = null;
      }
      return;
    }

    // Retry with backoff
    if (timeoutRef.current) clearTimeout(timeoutRef.current);
    timeoutRef.current = setTimeout(() => {
      if (mountedRef.current) {
        const url = api.preview.getSceneSnapshotUrl(
          profileName,
          sceneId,
          previewWidth,
          previewHeight,
          3
        );
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
            const url = api.preview.getSceneSnapshotUrl(
              profileName,
              sceneId,
              previewWidth,
              previewHeight,
              3
            );
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
