/**
 * Scene Canvas
 * Central composition canvas showing layers with drag/resize
 *
 * Prevents layout flash by calculating initial dimensions synchronously
 * based on viewport size, then refining with ResizeObserver.
 */
import { useRef, useState, useLayoutEffect, useMemo, useEffect, useCallback } from 'react';
import { Card } from '@/components/ui/Card';
import type { Scene, SourceLayer } from '@/types/scene';
import type { Source } from '@/types/source';
import { api } from '@/lib/backend';

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
}

export function SceneCanvas({
  scene,
  sources,
  selectedLayerId,
  onSelectLayer,
}: SceneCanvasProps) {
  const containerRef = useRef<HTMLDivElement>(null);

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
          {/* Layers */}
          {sortedLayers.map((layer) => (
            <LayerPreview
              key={layer.id}
              layer={layer}
              scale={scale}
              sourceName={getSourceName(layer.sourceId)}
              source={getSource(layer.sourceId)}
              isSelected={layer.id === selectedLayerId}
              onClick={() => onSelectLayer(layer.id)}
            />
          ))}

          {/* Canvas size indicator */}
          <div className="absolute bottom-2 right-2 text-sm text-[var(--text-muted)] bg-[var(--bg-elevated)] px-2 py-1 rounded shadow">
            {scene.canvasWidth}x{scene.canvasHeight}
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

  // Calculate preview dimensions based on layer size (with reasonable limits)
  const previewWidth = Math.min(Math.max(Math.round(transform.width * scale), 160), 640);
  const previewHeight = Math.min(Math.max(Math.round(transform.height * scale), 90), 360);

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
      const url = api.preview.getSourceSnapshotUrl(source.id, previewWidth, previewHeight, 5);
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
        const url = api.preview.getSourceSnapshotUrl(source.id, previewWidth, previewHeight, 5);
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
        const url = api.preview.getSourceSnapshotUrl(source.id, previewWidth, previewHeight, 5);
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
