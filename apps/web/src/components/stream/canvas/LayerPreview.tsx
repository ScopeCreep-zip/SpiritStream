/**
 * LayerPreview — Live preview for a layer with drag/resize support
 * Extracted from SceneCanvas
 *
 * Uses fixed preview dimensions for stable polling (no rate limiting)
 * Uses CSS transforms for GPU-accelerated drag/resize
 * Memoized to prevent unnecessary re-renders
 */
import React, { useRef, useState, useEffect, useCallback } from 'react';
import type { Scene, SourceLayer, Transform } from '@/types/scene';
import type { Source } from '@/types/source';
import type { ColorSource, TextSource, BrowserSource, NestedSceneSource, MediaPlaylistSource } from '@/types/source';
import { SharedWebRTCPlayer } from '../SharedWebRTCPlayer';
import { StaticMediaPlayer } from '../StaticMediaPlayer';
import { TextSourceRenderer } from '../TextSourceRenderer';
import { BrowserSourceRenderer } from '../BrowserSourceRenderer';
import { NestedSceneRenderer } from '../NestedSceneRenderer';
import { MediaPlaylistRenderer } from '../MediaPlaylistRenderer';
import { isStaticMediaFile, isImageFile } from '@/lib/mediaTypes';

type ResizeDirection = 'nw' | 'ne' | 'sw' | 'se' | null;

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
  scenes: Scene[];
  sources: Source[];
  readOnly?: boolean;
}

export const LayerPreview = React.memo(function LayerPreview({
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

  // Refs to track dragging state for the reset effect
  const isDraggingRef = useRef(false);
  const isResizingRef = useRef<ResizeDirection>(null);

  // Keep refs in sync with state
  isDraggingRef.current = isDragging;
  isResizingRef.current = isResizing;

  // Reset offsets only when transform actually changes from server
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
  useEffect(() => {
    if (!isDragging && !isResizing) return;

    let rafPending = false;
    let lastClientX = 0;
    let lastClientY = 0;

    const processMouseMove = () => {
      const deltaX = (lastClientX - dragStartRef.current.mouseX) / scale;
      const deltaY = (lastClientY - dragStartRef.current.mouseY) / scale;

      if (isDragging) {
        const newX = Math.max(0, Math.min(canvasWidth - transform.width, dragStartRef.current.layerX + deltaX));
        const newY = Math.max(0, Math.min(canvasHeight - transform.height, dragStartRef.current.layerY + deltaY));
        setDragOffset({ x: newX - transform.x, y: newY - transform.y });
      } else if (isResizing) {
        let newWidth = dragStartRef.current.width;
        let newHeight = dragStartRef.current.height;
        let newX = dragStartRef.current.layerX;
        let newY = dragStartRef.current.layerY;

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
      if (isDragging) {
        const newX = transform.x + dragOffset.x;
        const newY = transform.y + dragOffset.y;
        if (dragOffset.x !== 0 || dragOffset.y !== 0) {
          onTransformChange({ x: Math.round(newX), y: Math.round(newY) });
        }
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
      {/* Live preview via shared WebRTC, CSS rendering, or static rendering */}
      <div className="w-full h-full bg-[var(--bg-sunken)] overflow-hidden pointer-events-none">
        {hasVideo && source ? (
          source.type === 'color' ? (
            <div
              style={{
                backgroundColor: (source as ColorSource).color,
                opacity: (source as ColorSource).opacity,
                width: '100%',
                height: '100%',
              }}
            />
          ) : source.type === 'text' ? (
            <TextSourceRenderer source={source as TextSource} width={displayWidth} height={displayHeight} />
          ) : source.type === 'browser' ? (
            <BrowserSourceRenderer source={source as BrowserSource} width={displayWidth} height={displayHeight} />
          ) : source.type === 'nestedScene' ? (
            <NestedSceneRenderer source={source as NestedSceneSource} scenes={scenes} sources={sources} width={displayWidth} height={displayHeight} />
          ) : source.type === 'mediaPlaylist' ? (
            <MediaPlaylistRenderer source={source as MediaPlaylistSource} isLayerPreview />
          ) : source.type === 'mediaFile' && 'filePath' in source && isStaticMediaFile(source.filePath) ? (
            <StaticMediaPlayer filePath={source.filePath} isImage={isImageFile(source.filePath)} width={displayWidth} height={displayHeight} sourceName={sourceName} nativeWidth={canvasWidth} nativeHeight={canvasHeight} />
          ) : (
            <SharedWebRTCPlayer sourceId={source.id} sourceName={sourceName} sourceType={source.type} width={displayWidth} height={displayHeight} />
          )
        ) : (
          <div className="w-full h-full bg-gradient-to-br from-[var(--bg-elevated)] to-[var(--bg-sunken)] flex items-center justify-center">
            <span className="text-[var(--text-muted)] text-xs text-center px-2 truncate">{sourceName}</span>
          </div>
        )}
      </div>

      {/* Resize handles */}
      {isSelected && !layer.locked && !readOnly && (
        <>
          {(['nw', 'ne', 'sw', 'se'] as const).map((dir) => (
            <div
              key={dir}
              className={`absolute w-6 h-6 flex items-center justify-center cursor-${dir}-resize z-10 ${
                dir.includes('n') ? '-top-2' : '-bottom-2'
              } ${dir.includes('w') ? '-left-2' : '-right-2'}`}
              onMouseDown={(e) => handleResizeStart(e, dir)}
            >
              <div className="w-4 h-4 bg-primary rounded-full border-2 border-primary-foreground shadow-md" />
            </div>
          ))}
        </>
      )}
    </div>
  );
});
