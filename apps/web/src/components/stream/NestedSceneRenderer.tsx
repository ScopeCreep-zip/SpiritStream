/**
 * Nested Scene Renderer
 * Renders a scene within another scene as a source
 *
 * This component displays nested scene content using a simplified rendering approach.
 * For full edit capabilities, the parent SceneCanvas handles the actual layer rendering.
 */
import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { Layers, AlertTriangle } from 'lucide-react';
import type { Scene } from '@/types/scene';
import type { Source, NestedSceneSource, ColorSource } from '@/types/source';
import { WorkerVideoPreview } from './WorkerVideoPreview';

interface NestedSceneRendererProps {
  source: NestedSceneSource;
  scenes: Scene[];
  sources: Source[];
  width: number;
  height: number;
  /** Current nesting depth to prevent infinite recursion */
  depth?: number;
  /** Maximum allowed nesting depth */
  maxDepth?: number;
}

const MAX_NESTING_DEPTH = 5;

export function NestedSceneRenderer({
  source,
  scenes,
  sources,
  width,
  height,
  depth = 0,
  maxDepth = MAX_NESTING_DEPTH,
}: NestedSceneRendererProps) {
  const { t } = useTranslation();

  // Find the referenced scene
  const referencedScene = useMemo(
    () => scenes.find((s) => s.id === source.referencedSceneId),
    [scenes, source.referencedSceneId]
  );

  // Check if we've exceeded max nesting depth
  if (depth >= maxDepth) {
    return (
      <div className="w-full h-full flex items-center justify-center bg-yellow-500/20 border-2 border-yellow-500/50 rounded">
        <div className="text-center text-yellow-500 p-4">
          <AlertTriangle className="w-8 h-8 mx-auto mb-2" />
          <p className="text-sm font-medium">
            {t('stream.maxNestingDepth', { defaultValue: 'Maximum nesting depth reached' })}
          </p>
          <p className="text-xs opacity-75">
            {t('stream.maxNestingDepthHint', {
              max: maxDepth,
              defaultValue: `Nested scenes are limited to ${maxDepth} levels`,
            })}
          </p>
        </div>
      </div>
    );
  }

  // Scene not found
  if (!referencedScene) {
    return (
      <div className="w-full h-full flex items-center justify-center bg-red-500/20 border-2 border-red-500/50 rounded">
        <div className="text-center text-red-500 p-4">
          <AlertTriangle className="w-8 h-8 mx-auto mb-2" />
          <p className="text-sm font-medium">
            {t('stream.sceneNotFound', { defaultValue: 'Scene not found' })}
          </p>
          <p className="text-xs opacity-75">
            {t('stream.sceneNotFoundHint', {
              id: source.referencedSceneId,
              defaultValue: `Referenced scene "${source.referencedSceneId}" does not exist`,
            })}
          </p>
        </div>
      </div>
    );
  }

  // Empty scene
  if (referencedScene.layers.length === 0) {
    return (
      <div className="w-full h-full flex items-center justify-center bg-muted/20 border-2 border-dashed border-muted rounded">
        <div className="text-center text-muted p-4">
          <Layers className="w-8 h-8 mx-auto mb-2 opacity-50" />
          <p className="text-sm">
            {t('stream.emptyScene', { name: referencedScene.name, defaultValue: `"${referencedScene.name}" is empty` })}
          </p>
        </div>
      </div>
    );
  }

  // Render the nested scene's layers
  // Sort layers by zIndex (lowest first = back, highest last = front)
  const sortedLayers = [...referencedScene.layers].sort((a, b) => a.zIndex - b.zIndex);

  // Calculate scale factors for nested layers
  const scaleX = width / referencedScene.canvasWidth;
  const scaleY = height / referencedScene.canvasHeight;

  return (
    <div className="relative w-full h-full overflow-hidden bg-black">
      {sortedLayers.map((layer) => {
        if (!layer.visible) return null;

        const layerSource = sources.find((s) => s.id === layer.sourceId);
        if (!layerSource) return null;

        // Calculate scaled dimensions
        const layerWidth = layer.transform.width * scaleX;
        const layerHeight = layer.transform.height * scaleY;
        const layerX = layer.transform.x * scaleX;
        const layerY = layer.transform.y * scaleY;

        return (
          <div
            key={layer.id}
            className="absolute overflow-hidden"
            style={{
              left: layerX,
              top: layerY,
              width: layerWidth,
              height: layerHeight,
              transform: layer.transform.rotation ? `rotate(${layer.transform.rotation}deg)` : undefined,
              zIndex: layer.zIndex,
            }}
          >
            {/* Render based on source type */}
            {layerSource.type === 'color' ? (
              <div
                style={{
                  backgroundColor: (layerSource as ColorSource).color,
                  opacity: (layerSource as ColorSource).opacity,
                  width: '100%',
                  height: '100%',
                }}
              />
            ) : layerSource.type === 'nestedScene' ? (
              // Recursively render nested scenes
              <NestedSceneRenderer
                source={layerSource as NestedSceneSource}
                scenes={scenes}
                sources={sources}
                width={layerWidth}
                height={layerHeight}
                depth={depth + 1}
                maxDepth={maxDepth}
              />
            ) : (
              // Use WebRTC via worker for low-latency rendering
              <WorkerVideoPreview
                sourceId={layerSource.id}
                sourceName={layerSource.name}
                sourceType={layerSource.type}
                width={layerWidth}
                height={layerHeight}
              />
            )}
          </div>
        );
      })}
    </div>
  );
}
