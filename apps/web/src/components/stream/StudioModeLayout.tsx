/**
 * Studio Mode Layout
 * Dual-pane layout with Preview (editable) and Program (live) canvases
 */
import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { SceneCanvas } from './SceneCanvas';
import { TakeButton } from './TakeButton';
import { QuickTransitionPicker } from './QuickTransitionPicker';
import { StudioModeSettings } from './StudioModeSettings';
import { TBar } from './TBar';
import { useStudioStore } from '@/stores/studioStore';
import { useTransitionStore } from '@/stores/transitionStore';
import type { Profile, Source } from '@/types/profile';

interface StudioModeLayoutProps {
  profile: Profile;
  sources: Source[];
  selectedLayerId: string | null;
  onSelectLayer: (layerId: string | null) => void;
}

export function StudioModeLayout({
  profile,
  sources,
  selectedLayerId,
  onSelectLayer,
}: StudioModeLayoutProps) {
  const { t } = useTranslation();
  const { previewSceneId, programSceneId, executeTake } = useStudioStore();
  const { isTransitioning } = useTransitionStore();

  // Find the scenes
  const previewScene = useMemo(
    () => profile.scenes.find((s) => s.id === previewSceneId),
    [profile.scenes, previewSceneId]
  );

  const programScene = useMemo(
    () => profile.scenes.find((s) => s.id === programSceneId),
    [profile.scenes, programSceneId]
  );

  // Can take only if preview and program are different
  const canTake = previewSceneId !== programSceneId && !isTransitioning;

  return (
    <div className="flex flex-1 gap-2 min-h-0 min-w-0 overflow-hidden">
      {/* Preview Pane - Editable */}
      <div className="flex-1 min-w-0 min-h-0 h-full overflow-hidden">
        <SceneCanvas
          scene={previewScene}
          sources={sources}
          selectedLayerId={selectedLayerId}
          onSelectLayer={onSelectLayer}
          profileName={profile.name}
          studioMode="preview"
        />
      </div>

      {/* Controls Column */}
      <div className="w-20 flex flex-col items-center justify-center gap-3 py-4 flex-shrink-0">
        <TakeButton onClick={executeTake} disabled={!canTake} />
        <StudioModeSettings />
        <TBar disabled={!canTake} />
        <QuickTransitionPicker />
        <div className="text-xs text-[var(--text-muted)] text-center">
          {isTransitioning ? t('stream.transitioning', { defaultValue: 'Transitioning...' }) : null}
        </div>
      </div>

      {/* Program Pane - Read-only (Live) */}
      <div className="flex-1 min-w-0 min-h-0 h-full overflow-hidden pointer-events-none">
        <SceneCanvas
          scene={programScene}
          sources={sources}
          selectedLayerId={null}
          onSelectLayer={() => {}}
          profileName={profile.name}
          studioMode="program"
        />
      </div>
    </div>
  );
}
