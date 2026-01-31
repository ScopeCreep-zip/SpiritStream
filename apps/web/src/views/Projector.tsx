/**
 * Projector View
 * Fullscreen scene output for external displays or streaming preview
 */
import { useEffect, useState, useCallback } from 'react';
import { useSearchParams } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { X, Maximize2, Minimize2 } from 'lucide-react';
import { SceneCanvas } from '@/components/stream/SceneCanvas';
import { useProfileStore } from '@/stores/profileStore';
import { useProjectorStore } from '@/stores/projectorStore';
import { cn } from '@/lib/utils';

export function Projector() {
  const { t } = useTranslation();
  const [searchParams] = useSearchParams();
  const [showControls, setShowControls] = useState(false);
  const [isFullscreen, setIsFullscreen] = useState(false);
  const [mouseTimer, setMouseTimer] = useState<NodeJS.Timeout | null>(null);

  const profileName = searchParams.get('profileName');
  const sceneId = searchParams.get('sceneId');

  const loadProfile = useProfileStore((s) => s.loadProfile);
  const current = useProfileStore((s) => s.current);
  const clearProjectedScene = useProjectorStore((s) => s.clearProjectedScene);

  // Load profile if not already loaded
  useEffect(() => {
    if (profileName && (!current || current.name !== profileName)) {
      loadProfile(profileName);
    }
  }, [profileName, current, loadProfile]);

  // Get the scene to display
  const scene = current?.scenes.find((s) => s.id === sceneId) ?? current?.scenes.find((s) => s.id === current.activeSceneId);

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
    clearProjectedScene();
    window.close();
  };

  if (!current || !scene) {
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

  return (
    <div
      className="fixed inset-0 bg-black overflow-hidden cursor-none"
      onMouseMove={handleMouseMove}
      style={{ cursor: showControls ? 'default' : 'none' }}
    >
      {/* Scene canvas - fills entire screen */}
      <div className="absolute inset-0">
        <SceneCanvas
          scene={scene}
          sources={current.sources}
          scenes={current.scenes}
          selectedLayerId={null}
          onSelectLayer={() => {}}
          profileName={current.name}
        />
      </div>

      {/* Controls overlay - shown on mouse movement */}
      <div
        className={cn(
          'absolute inset-x-0 top-0 p-4 bg-gradient-to-b from-black/80 to-transparent transition-opacity duration-300',
          showControls ? 'opacity-100' : 'opacity-0 pointer-events-none'
        )}
      >
        <div className="flex items-center justify-between max-w-screen-lg mx-auto">
          {/* Scene info */}
          <div className="text-white">
            <h1 className="text-lg font-semibold">{scene.name}</h1>
            <p className="text-sm text-gray-400">{current.name}</p>
          </div>

          {/* Control buttons */}
          <div className="flex items-center gap-2">
            <button
              onClick={toggleFullscreen}
              className="p-2 rounded-lg bg-white/10 hover:bg-white/20 text-white transition-colors"
              title={isFullscreen
                ? t('projector.exitFullscreen', { defaultValue: 'Exit Fullscreen (F)' })
                : t('projector.enterFullscreen', { defaultValue: 'Enter Fullscreen (F)' })
              }
            >
              {isFullscreen ? (
                <Minimize2 className="w-5 h-5" />
              ) : (
                <Maximize2 className="w-5 h-5" />
              )}
            </button>
            <button
              onClick={handleClose}
              className="p-2 rounded-lg bg-white/10 hover:bg-white/20 text-white transition-colors"
              title={t('projector.close', { defaultValue: 'Close Projector (Esc)' })}
            >
              <X className="w-5 h-5" />
            </button>
          </div>
        </div>
      </div>

      {/* Bottom info bar */}
      <div
        className={cn(
          'absolute inset-x-0 bottom-0 p-4 bg-gradient-to-t from-black/80 to-transparent transition-opacity duration-300',
          showControls ? 'opacity-100' : 'opacity-0 pointer-events-none'
        )}
      >
        <div className="flex items-center justify-center text-xs text-gray-400 gap-4">
          <span>F - {t('projector.toggleFullscreen', { defaultValue: 'Toggle Fullscreen' })}</span>
          <span>ESC - {t('projector.exit', { defaultValue: 'Exit' })}</span>
        </div>
      </div>
    </div>
  );
}

export default Projector;
