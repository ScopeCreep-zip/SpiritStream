/**
 * Projector View
 * Fullscreen output for external displays or streaming preview
 * Supports: scene, source, preview, program, and multiview projector types
 */
import { useEffect, useState, useCallback, useMemo } from 'react';
import { useSearchParams } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { X, Maximize2, Minimize2 } from 'lucide-react';
import { SceneCanvas } from '@/components/stream/SceneCanvas';
import { SharedWebRTCPlayer } from '@/components/stream/SharedWebRTCPlayer';
import { useProfileStore } from '@/stores/profileStore';
import { useStudioStore } from '@/stores/studioStore';
import { cn } from '@/lib/utils';
import { parseProjectorParams, getProjectorTypeLabel } from '@/types/projector';
import type { Scene } from '@/types/scene';
import type { Source } from '@/types/source';

export function Projector() {
  const { t } = useTranslation();
  const [searchParams] = useSearchParams();
  const [showControls, setShowControls] = useState(false);
  const [isFullscreen, setIsFullscreen] = useState(false);
  const [mouseTimer, setMouseTimer] = useState<NodeJS.Timeout | null>(null);

  // Parse projector configuration from URL
  const projectorConfig = useMemo(
    () => parseProjectorParams(searchParams),
    [searchParams]
  );

  const { type, profileName, targetId, hideCursor } = projectorConfig;

  const loadProfile = useProfileStore((s) => s.loadProfile);
  const current = useProfileStore((s) => s.current);

  // Studio mode state for preview/program projectors
  const previewSceneId = useStudioStore((s) => s.previewSceneId);
  const programSceneId = useStudioStore((s) => s.programSceneId);

  // Load profile if not already loaded
  useEffect(() => {
    if (profileName && (!current || current.name !== profileName)) {
      loadProfile(profileName);
    }
  }, [profileName, current, loadProfile]);

  // Handle fullscreen changes
  useEffect(() => {
    const handleFullscreenChange = () => {
      setIsFullscreen(!!document.fullscreenElement);
    };

    document.addEventListener('fullscreenchange', handleFullscreenChange);
    return () => document.removeEventListener('fullscreenchange', handleFullscreenChange);
  }, []);

  // Auto-hide controls after mouse inactivity
  const handleMouseMove = useCallback(() => {
    setShowControls(true);

    if (mouseTimer) {
      clearTimeout(mouseTimer);
    }

    const timer = setTimeout(() => {
      setShowControls(false);
    }, 3000);

    setMouseTimer(timer);
  }, [mouseTimer]);

  // Cleanup timer on unmount
  useEffect(() => {
    return () => {
      if (mouseTimer) {
        clearTimeout(mouseTimer);
      }
    };
  }, [mouseTimer]);

  // Handle keyboard shortcuts
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      switch (e.key) {
        case 'Escape':
          // Exit fullscreen or close projector
          if (document.fullscreenElement) {
            document.exitFullscreen();
          } else {
            handleClose();
          }
          break;
        case 'f':
        case 'F':
          // Toggle fullscreen
          toggleFullscreen();
          break;
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, []);

  const toggleFullscreen = async () => {
    try {
      if (!document.fullscreenElement) {
        await document.documentElement.requestFullscreen();
      } else {
        await document.exitFullscreen();
      }
    } catch (err) {
      console.error('[Projector] Fullscreen toggle failed:', err);
    }
  };

  const handleClose = () => {
    window.close();
  };

  // Determine which scene to display based on projector type
  const targetScene = useMemo((): Scene | undefined => {
    if (!current) return undefined;

    switch (type) {
      case 'scene':
        // Specific scene by ID
        return current.scenes.find((s) => s.id === targetId)
          ?? current.scenes.find((s) => s.id === current.activeSceneId);
      case 'preview':
        // Preview scene (Studio Mode)
        return current.scenes.find((s) => s.id === previewSceneId);
      case 'program':
        // Program scene (Studio Mode or active scene)
        return current.scenes.find((s) => s.id === programSceneId)
          ?? current.scenes.find((s) => s.id === current.activeSceneId);
      default:
        return undefined;
    }
  }, [current, type, targetId, previewSceneId, programSceneId]);

  // Get source for source projector type
  const targetSource = useMemo(() => {
    if (type !== 'source' || !current || !targetId) return undefined;
    return current.sources.find((s) => s.id === targetId);
  }, [current, type, targetId]);

  // Loading state
  if (!current) {
    return (
      <div className="fixed inset-0 bg-black flex items-center justify-center">
        <div className="text-white text-center">
          <p className="text-lg">{t('projector.loading', { defaultValue: 'Loading...' })}</p>
          {!profileName && (
            <p className="text-sm text-gray-400 mt-2">
              {t('projector.noProfile', { defaultValue: 'No profile specified' })}
            </p>
          )}
        </div>
      </div>
    );
  }

  // Render content based on projector type
  const renderContent = () => {
    switch (type) {
      case 'source':
        return <SourceProjector source={targetSource} sourceName={targetSource?.name ?? 'Unknown'} />;
      case 'scene':
      case 'preview':
      case 'program':
        return (
          <SceneProjector
            scene={targetScene}
            sources={current.sources}
            scenes={current.scenes}
            profileName={current.name}
            type={type as 'scene' | 'preview' | 'program'}
          />
        );
      case 'multiview':
        return (
          <MultiviewProjector
            scenes={current.scenes}
            sources={current.sources}
            profileName={current.name}
            activeSceneId={current.activeSceneId}
            previewSceneId={previewSceneId ?? undefined}
            programSceneId={programSceneId ?? undefined}
          />
        );
      default:
        return (
          <div className="flex items-center justify-center h-full text-white">
            <p>Unknown projector type: {type}</p>
          </div>
        );
    }
  };

  // Get display title based on type
  const getTitle = () => {
    switch (type) {
      case 'source':
        return targetSource?.name ?? 'Source';
      case 'scene':
        return targetScene?.name ?? 'Scene';
      case 'preview':
        return `Preview: ${targetScene?.name ?? '(none)'}`;
      case 'program':
        return `Program: ${targetScene?.name ?? '(none)'}`;
      case 'multiview':
        return 'Multiview';
      default:
        return type ? getProjectorTypeLabel(type) : 'Projector';
    }
  };

  return (
    <div
      className={cn(
        'fixed inset-0 bg-black overflow-hidden',
        hideCursor && 'cursor-none'
      )}
      onMouseMove={handleMouseMove}
      style={{ cursor: showControls || !hideCursor ? 'default' : 'none' }}
    >
      {/* Content */}
      <div className="absolute inset-0">
        {renderContent()}
      </div>

      {/* Controls overlay - minimal, corners only, shown on mouse movement */}
      {/* Top-left: Scene info (small) */}
      <div
        className={cn(
          'absolute top-2 left-2 transition-opacity duration-300',
          showControls ? 'opacity-100' : 'opacity-0 pointer-events-none'
        )}
      >
        <div className="px-2 py-1 rounded bg-black/50 backdrop-blur-sm">
          <span className="text-xs text-white/80">{getTitle()}</span>
        </div>
      </div>

      {/* Top-right: Control buttons (small) */}
      <div
        className={cn(
          'absolute top-2 right-2 flex items-center gap-1 transition-opacity duration-300',
          showControls ? 'opacity-100' : 'opacity-0 pointer-events-none'
        )}
      >
        <button
          onClick={toggleFullscreen}
          className="p-1.5 rounded bg-black/50 backdrop-blur-sm hover:bg-black/70 text-white/80 hover:text-white transition-colors"
          title={isFullscreen
            ? t('projector.exitFullscreen', { defaultValue: 'Exit Fullscreen (F)' })
            : t('projector.enterFullscreen', { defaultValue: 'Enter Fullscreen (F)' })
          }
        >
          {isFullscreen ? (
            <Minimize2 className="w-4 h-4" />
          ) : (
            <Maximize2 className="w-4 h-4" />
          )}
        </button>
        <button
          onClick={handleClose}
          className="p-1.5 rounded bg-black/50 backdrop-blur-sm hover:bg-black/70 text-white/80 hover:text-white transition-colors"
          title={t('projector.close', { defaultValue: 'Close Projector (Esc)' })}
        >
          <X className="w-4 h-4" />
        </button>
      </div>

      {/* Bottom-right: Keyboard hints (tiny) */}
      <div
        className={cn(
          'absolute bottom-2 right-2 transition-opacity duration-300',
          showControls ? 'opacity-100' : 'opacity-0 pointer-events-none'
        )}
      >
        <div className="px-2 py-1 rounded bg-black/50 backdrop-blur-sm">
          <span className="text-[10px] text-white/60">F: Fullscreen | ESC: Exit</span>
        </div>
      </div>
    </div>
  );
}

/**
 * Source Projector - Renders a single source fullscreen
 */
interface SourceProjectorProps {
  source: { id: string; name: string; type: string } | undefined;
  sourceName: string;
}

function SourceProjector({ source, sourceName }: SourceProjectorProps) {
  if (!source) {
    return (
      <div className="flex items-center justify-center h-full text-white text-center">
        <div>
          <p className="text-lg">Source not found</p>
          <p className="text-sm text-gray-400 mt-2">The requested source is not available</p>
        </div>
      </div>
    );
  }

  return (
    <div className="w-full h-full">
      <SharedWebRTCPlayer
        sourceId={source.id}
        sourceName={sourceName}
        sourceType={source.type as 'rtmp' | 'mediaFile' | 'screenCapture' | 'camera' | 'captureCard' | 'audioDevice'}
        width={window.innerWidth}
        height={window.innerHeight}
      />
    </div>
  );
}

/**
 * Scene Projector - Renders a scene fullscreen
 */
interface SceneProjectorProps {
  scene: Scene | undefined;
  sources: Source[];
  scenes: Scene[];
  profileName: string;
  type: 'scene' | 'preview' | 'program';
}

function SceneProjector({ scene, sources, scenes, profileName, type }: SceneProjectorProps) {
  if (!scene) {
    return (
      <div className="flex items-center justify-center h-full text-white text-center">
        <div>
          <p className="text-lg">
            {type === 'preview' ? 'No Preview Scene' : type === 'program' ? 'No Program Scene' : 'Scene not found'}
          </p>
          <p className="text-sm text-gray-400 mt-2">
            {type === 'preview' || type === 'program'
              ? 'Enable Studio Mode and select a scene'
              : 'The requested scene is not available'}
          </p>
        </div>
      </div>
    );
  }

  return (
    <SceneCanvas
      scene={scene}
      sources={sources}
      scenes={scenes}
      selectedLayerId={null}
      onSelectLayer={() => {}}
      profileName={profileName}
      studioMode="program"
      hideHeader
    />
  );
}

/**
 * Multiview Projector - Grid of all scenes with tally indicators
 */
interface MultiviewProjectorProps {
  scenes: Scene[];
  sources: Source[];
  profileName: string;
  activeSceneId?: string;
  previewSceneId?: string;
  programSceneId?: string;
}

function MultiviewProjector({
  scenes,
  sources,
  profileName,
  activeSceneId,
  previewSceneId,
  programSceneId,
}: MultiviewProjectorProps) {
  // Calculate grid dimensions based on scene count
  const gridCols = scenes.length <= 4 ? 2 : scenes.length <= 9 ? 3 : 4;

  return (
    <div className="w-full h-full p-4">
      <div
        className="w-full h-full grid gap-2"
        style={{ gridTemplateColumns: `repeat(${gridCols}, 1fr)` }}
      >
        {scenes.map((scene) => {
          const isPreview = scene.id === previewSceneId;
          const isProgram = scene.id === programSceneId;
          const isActive = scene.id === activeSceneId;

          return (
            <div
              key={scene.id}
              className={cn(
                'relative rounded-lg overflow-hidden',
                isProgram && 'ring-4 ring-red-500',
                isPreview && !isProgram && 'ring-4 ring-green-500',
                !isPreview && !isProgram && isActive && 'ring-2 ring-primary'
              )}
            >
              <SceneCanvas
                scene={scene}
                sources={sources}
                scenes={scenes}
                selectedLayerId={null}
                onSelectLayer={() => {}}
                profileName={profileName}
                studioMode="program"
                hideHeader
              />

              {/* Scene label overlay */}
              <div className="absolute inset-x-0 bottom-0 p-2 bg-gradient-to-t from-black/80 to-transparent">
                <div className="flex items-center justify-between">
                  <span className="text-sm font-medium text-white truncate">
                    {scene.name}
                  </span>
                  <div className="flex items-center gap-1">
                    {isProgram && (
                      <span className="px-1.5 py-0.5 text-[10px] font-bold bg-red-600 text-white rounded flex items-center gap-1">
                        <span className="w-1.5 h-1.5 bg-white rounded-full animate-pulse" />
                        LIVE
                      </span>
                    )}
                    {isPreview && !isProgram && (
                      <span className="px-1.5 py-0.5 text-[10px] font-bold bg-green-600 text-white rounded">
                        PVW
                      </span>
                    )}
                  </div>
                </div>
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}

export default Projector;
