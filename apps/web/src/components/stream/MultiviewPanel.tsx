/**
 * Multiview Panel
 * Grid view of all scenes for quick switching
 */
import { useState, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { X, Grid2X2, Grid3X3, LayoutGrid } from 'lucide-react';
import { SceneCard } from './SceneCard';
import { useStudioStore } from '@/stores/studioStore';
import { useProfileStore } from '@/stores/profileStore';
import type { Profile } from '@/types/profile';

interface MultiviewPanelProps {
  profile: Profile;
  onClose: () => void;
}

type GridSize = 2 | 3 | 4;

export function MultiviewPanel({ profile, onClose }: MultiviewPanelProps) {
  const { t } = useTranslation();
  const [gridSize, setGridSize] = useState<GridSize>(2);

  const studioEnabled = useStudioStore((s) => s.enabled);
  const previewSceneId = useStudioStore((s) => s.previewSceneId);
  const programSceneId = useStudioStore((s) => s.programSceneId);
  const setPreviewScene = useStudioStore((s) => s.setPreviewScene);
  const setCurrentActiveScene = useProfileStore((s) => s.setCurrentActiveScene);

  const handleSceneClick = useCallback((sceneId: string) => {
    if (studioEnabled) {
      // In Studio Mode, clicking sends to Preview
      setPreviewScene(sceneId);
    } else {
      // In Normal Mode, clicking switches directly to the scene
      setCurrentActiveScene(sceneId);
    }
  }, [studioEnabled, setPreviewScene, setCurrentActiveScene]);

  const gridClasses = {
    2: 'grid-cols-2 gap-4',
    3: 'grid-cols-3 gap-3',
    4: 'grid-cols-4 gap-2',
  };

  const cardSize = gridSize === 2 ? 'lg' : gridSize === 3 ? 'md' : 'sm';

  return (
    <div className="fixed inset-0 z-50 bg-black/80 flex items-center justify-center p-6">
      <div className="bg-[var(--bg-surface)] rounded-xl w-full max-w-5xl max-h-[90vh] flex flex-col overflow-hidden">
        {/* Header */}
        <div className="flex items-center justify-between px-4 py-3 border-b border-[var(--border-default)]">
          <h2 className="text-lg font-semibold">
            {t('stream.multiview', { defaultValue: 'Multiview' })}
          </h2>
          <div className="flex items-center gap-2">
            {/* Grid size selector */}
            <div className="flex items-center gap-1 bg-[var(--bg-sunken)] rounded-lg p-1">
              <button
                type="button"
                onClick={() => setGridSize(2)}
                className={`p-1.5 rounded-md transition-colors ${
                  gridSize === 2
                    ? 'bg-primary text-white'
                    : 'text-[var(--text-muted)] hover:text-[var(--text-secondary)]'
                }`}
                title="2x2"
              >
                <Grid2X2 className="w-4 h-4" />
              </button>
              <button
                type="button"
                onClick={() => setGridSize(3)}
                className={`p-1.5 rounded-md transition-colors ${
                  gridSize === 3
                    ? 'bg-primary text-white'
                    : 'text-[var(--text-muted)] hover:text-[var(--text-secondary)]'
                }`}
                title="3x3"
              >
                <Grid3X3 className="w-4 h-4" />
              </button>
              <button
                type="button"
                onClick={() => setGridSize(4)}
                className={`p-1.5 rounded-md transition-colors ${
                  gridSize === 4
                    ? 'bg-primary text-white'
                    : 'text-[var(--text-muted)] hover:text-[var(--text-secondary)]'
                }`}
                title="4x4"
              >
                <LayoutGrid className="w-4 h-4" />
              </button>
            </div>
            <button
              type="button"
              onClick={onClose}
              className="p-1.5 rounded-md text-[var(--text-muted)] hover:text-[var(--text-primary)] hover:bg-[var(--bg-hover)] transition-colors"
              title={t('common.close', { defaultValue: 'Close' })}
            >
              <X className="w-5 h-5" />
            </button>
          </div>
        </div>

        {/* Grid content */}
        <div className="flex-1 overflow-y-auto p-4">
          {profile.scenes.length === 0 ? (
            <div className="flex items-center justify-center h-full text-[var(--text-muted)]">
              {t('stream.noScenes', { defaultValue: 'No scenes available' })}
            </div>
          ) : (
            <div className={`grid ${gridClasses[gridSize]}`}>
              {profile.scenes.map((scene) => (
                <SceneCard
                  key={scene.id}
                  scene={scene}
                  profile={profile}
                  sources={profile.sources}
                  isPreview={studioEnabled && scene.id === previewSceneId}
                  isProgram={studioEnabled && scene.id === programSceneId}
                  onClick={() => handleSceneClick(scene.id)}
                  size={cardSize}
                />
              ))}
            </div>
          )}
        </div>

        {/* Footer hint */}
        <div className="px-4 py-2 border-t border-[var(--border-default)] text-xs text-[var(--text-muted)] text-center">
          {studioEnabled
            ? t('stream.multiviewHintStudio', {
                defaultValue: 'Click a scene to send it to Preview',
              })
            : t('stream.multiviewHintNormal', {
                defaultValue: 'Click a scene to switch to it',
              })}
        </div>
      </div>
    </div>
  );
}
