/**
 * Studio Mode Layout
 * Dual-pane layout with Preview (editable) and Program (live) canvases
 */
import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { SceneCanvas } from './SceneCanvas';
import { TakeButton } from './TakeButton';
import { QuickTransitionPicker } from './QuickTransitionPicker';
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
    <div className="flex flex-1 gap-2 min-h-0">
      {/* Preview Pane - Editable */}
      <div className="flex-1 flex flex-col min-w-0">
        <div className="px-3 py-1.5 bg-[var(--bg-elevated)] border-b border-[var(--border-default)] flex items-center justify-between rounded-t-lg">
          <span className="text-sm font-medium text-green-500 flex items-center gap-1.5">
            <span className="w-2 h-2 bg-green-500 rounded-full" />
            {t('stream.preview', { defaultValue: 'Preview' })}
          </span>
          <span className="text-xs text-[var(--text-muted)]">{previewScene?.name}</span>
        </div>
        <div className="flex-1 min-h-0">
          <SceneCanvas
            scene={previewScene}
            sources={sources}
            selectedLayerId={selectedLayerId}
            onSelectLayer={onSelectLayer}
            profileName={profile.name}
          />
        </div>
      </div>

      {/* Controls Column */}
      <div className="w-20 flex flex-col items-center justify-center gap-3 py-4">
        <TakeButton onClick={executeTake} disabled={!canTake} />
        <QuickTransitionPicker />
        <div className="text-xs text-[var(--text-muted)] text-center">
          {isTransitioning ? t('stream.transitioning', { defaultValue: 'Transitioning...' }) : null}
        </div>
      </div>

      {/* Program Pane - Read-only (Live) */}
      <div className="flex-1 flex flex-col min-w-0">
        <div className="px-3 py-1.5 bg-[var(--bg-elevated)] border-b border-[var(--border-default)] flex items-center justify-between rounded-t-lg">
          <span className="text-sm font-medium text-red-500 flex items-center gap-1.5">
            <span className="w-2 h-2 bg-red-500 rounded-full animate-pulse" />
            {t('stream.program', { defaultValue: 'Program' })}
          </span>
          <span className="text-xs text-[var(--text-muted)]">{programScene?.name}</span>
        </div>
        <div className="flex-1 min-h-0 pointer-events-none">
          <SceneCanvas
            scene={programScene}
            sources={sources}
            selectedLayerId={null}
            onSelectLayer={() => {}}
            profileName={profile.name}
          />
        </div>
      </div>
    </div>
  );
}
