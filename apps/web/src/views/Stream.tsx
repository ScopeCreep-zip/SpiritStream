/**
 * Stream Page
 * Multi-input streaming with scene composition
 *
 * Performance optimizations (2026):
 * - Lazy loading: Heavy components (MultiviewPanel, StudioModeLayout, PropertiesPanel)
 *   are code-split and loaded on demand to reduce initial bundle size
 * - Memoized handlers: useCallback for streaming handlers prevents child re-renders
 * - useShallow: Zustand best practice for selector optimization
 * - useTransition: React 19 concurrent feature for non-blocking UI mode switches
 */
import { useState, useEffect, useMemo, useCallback, lazy, Suspense, useTransition } from 'react';
import { useTranslation } from 'react-i18next';
import { Play, Square, AlertTriangle, Plus, LayoutGrid, Grid3X3 } from 'lucide-react';
import { useShallow } from 'zustand/shallow';
import { Button } from '@/components/ui/Button';
import { Alert } from '@/components/ui/Alert';
import { SourcesPanel } from '@/components/stream/SourcesPanel';
import { SceneCanvas } from '@/components/stream/SceneCanvas';
import { SceneBar } from '@/components/stream/SceneBar';
import { AudioMixerPanel } from '@/components/stream/AudioMixerPanel';
import { TransitionOverlay } from '@/components/stream/TransitionOverlay';
import { RecordingButton } from '@/components/stream/RecordingButton';
import { ReplayBufferButton } from '@/components/stream/ReplayBufferButton';

// Lazy-loaded components for code splitting
// These components are heavier and not needed on initial render
const PropertiesPanel = lazy(() => import('@/components/stream/PropertiesPanel').then(m => ({ default: m.PropertiesPanel })));
const StudioModeLayout = lazy(() => import('@/components/stream/StudioModeLayout').then(m => ({ default: m.StudioModeLayout })));
const MultiviewPanel = lazy(() => import('@/components/stream/MultiviewPanel').then(m => ({ default: m.MultiviewPanel })));

// Lightweight loading fallbacks for lazy components
const PanelSkeleton = () => (
  <div className="h-full bg-[var(--bg-surface)] rounded-lg animate-pulse">
    <div className="h-10 bg-[var(--bg-elevated)] rounded-t-lg" />
    <div className="p-4 space-y-3">
      <div className="h-4 bg-[var(--bg-elevated)] rounded w-3/4" />
      <div className="h-4 bg-[var(--bg-elevated)] rounded w-1/2" />
      <div className="h-4 bg-[var(--bg-elevated)] rounded w-2/3" />
    </div>
  </div>
);

const StudioLayoutSkeleton = () => (
  <div className="flex-1 bg-[var(--bg-surface)] rounded-lg animate-pulse flex items-center justify-center">
    <div className="text-[var(--text-muted)]">Loading Studio Mode...</div>
  </div>
);
import { useProfileStore } from '@/stores/profileStore';
import { useStreamStore } from '@/stores/streamStore';
import { useSceneStore } from '@/stores/sceneStore';
import { useSourceStore } from '@/stores/sourceStore';
import { useStudioStore } from '@/stores/studioStore';
import { cn } from '@/lib/utils';
import { toast } from '@/hooks/useToast';
import { useHotkeys } from '@/hooks/useHotkeys';
import { getIncomingUrl, migrateProfileIfNeeded } from '@/types/profile';
import { validateStreamConfig, displayValidationIssues } from '@/lib/streamValidation';
import { api } from '@/lib/backend/httpApi';
import { useAudioLevels } from '@/hooks/useAudioLevels';

export function Stream() {
  const { t } = useTranslation();

  // Use useShallow to reduce re-renders by doing shallow comparison of the selected state
  // This is a 2026 Zustand best practice - previously 10+ separate selectors caused excessive re-renders
  const { current, loading, error, updateProfile, saveProfile } = useProfileStore(
    useShallow((s) => ({
      current: s.current,
      loading: s.loading,
      error: s.error,
      updateProfile: s.updateProfile,
      saveProfile: s.saveProfile,
    }))
  );

  const { isStreaming, activeGroups, startAllGroups, stopAllGroups } = useStreamStore(
    useShallow((s) => ({
      isStreaming: s.isStreaming,
      activeGroups: s.activeGroups,
      startAllGroups: s.startAllGroups,
      stopAllGroups: s.stopAllGroups,
    }))
  );

  const { selectedLayerId, selectLayer } = useSceneStore(
    useShallow((s) => ({
      selectedLayerId: s.selectedLayerId,
      selectLayer: s.selectLayer,
    }))
  );

  const discoverDevices = useSourceStore((s) => s.discoverDevices);

  const { enabled: studioEnabled, toggleStudioMode } = useStudioStore(
    useShallow((s) => ({
      enabled: s.enabled,
      toggleStudioMode: s.toggleStudioMode,
    }))
  );

  // Activate global hotkeys for the Stream view
  useHotkeys();

  // Get setCaptureStatus from the audio levels hook (single source of truth)
  const { setCaptureStatus } = useAudioLevels();

  const [isValidating, setIsValidating] = useState(false);
  const [showMultiview, setShowMultiview] = useState(false);

  // React 19 useTransition for non-blocking UI mode switches
  // This prevents audio meters and other real-time elements from stuttering
  // when switching between Studio Mode and normal mode
  const [isStudioTransitioning, startStudioTransition] = useTransition();
  const [isMultiviewTransitioning, startMultiviewTransition] = useTransition();

  // Wrap studio mode toggle in transition for non-blocking UI update
  const handleToggleStudioMode = useCallback(() => {
    startStudioTransition(() => {
      toggleStudioMode();
    });
  }, [toggleStudioMode]);

  // Toggle multiview with keyboard shortcut (Ctrl+M)
  const handleToggleMultiview = useCallback(() => {
    startMultiviewTransition(() => {
      setShowMultiview((prev) => !prev);
    });
  }, []);

  // Register Ctrl+M keyboard shortcut for Multiview
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'm') {
        e.preventDefault();
        handleToggleMultiview();
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [handleToggleMultiview]);

  // Discover devices on mount
  useEffect(() => {
    discoverDevices().catch((err) => {
      console.error('[Stream] Device discovery failed:', err);
      // Silent failure - non-critical, user can manually refresh
    });
  }, [discoverDevices]);

  // Migrate profile if needed (on first load)
  useEffect(() => {
    const migrateIfNeeded = async () => {
      if (current && current.sources.length === 0 && (current.input || current.scenes.length === 0)) {
        const migrated = migrateProfileIfNeeded(current);
        if (migrated !== current) {
          try {
            updateProfile(migrated);
            await saveProfile();
          } catch (err) {
            console.error('[Stream] Profile migration failed:', err);
            toast.error(t('errors.profileMigrationFailed', {
              defaultValue: 'Failed to save migrated profile'
            }));
          }
        }
      }
    };
    migrateIfNeeded();
  }, [current, updateProfile, saveProfile, t]);

  // Get active scene
  const activeScene = current?.scenes.find((s) => s.id === current.activeSceneId);
  const selectedLayer = activeScene?.layers.find((l) => l.id === selectedLayerId);

  // Create stable string key from track source IDs to prevent infinite loop
  // (array reference comparison always fails, causing constant re-renders)
  const trackSourceIdsKey = useMemo(
    () => activeScene?.audioMixer.tracks.map((t) => t.sourceId).join(',') ?? '',
    [activeScene?.audioMixer.tracks]
  );

  // Sync audio monitor sources with backend when scene changes
  useEffect(() => {
    if (!activeScene || !current) {
      // No scene selected, clear audio monitoring
      api.audio.setMonitorSources([]).then(() => {
        setCaptureStatus({});
      }).catch(console.error);
      return;
    }

    // Get audio track source IDs from the active scene
    const sourceIds = activeScene.audioMixer.tracks.map((t) => t.sourceId);
    // Pass profile name so backend can start real audio capture for device sources
    api.audio.setMonitorSources(sourceIds, current.name).then((result) => {
      // Store capture status in the hook (single source of truth for AudioMixerPanel)
      if (result.captureResults) {
        setCaptureStatus(result.captureResults);

        // Log actual failures (exclude expected non-audio sources and known limitations)
        const expectedReasons = [
          'noAudio',           // Source type doesn't support audio (Color, Text, etc.)
          'platformLimitation', // Platform doesn't support this audio capture
          'extractionUnavailable', // Audio metering unavailable but audio works in output
          'noCurrentItem',     // Playlist has no current item
          'unsupportedFormat', // File is not a media file (e.g., HTML used as MediaFile)
        ];
        const actualFailures = Object.entries(result.captureResults).filter(
          ([_, status]) => !status.success && !expectedReasons.includes(status.reason || '')
        );
        if (actualFailures.length > 0) {
          console.warn('[Stream] Audio capture failures:');
          actualFailures.forEach(([sourceId, status]) => {
            console.warn(`  - Source ${sourceId}: ${status.reason} - ${status.message} (type: ${status.sourceType})`);
          });
        }
      }
    }).catch(console.error);
  }, [activeScene?.id, trackSourceIdsKey, current?.name, setCaptureStatus]);

  // Memoize whether streaming is possible (has at least one target configured)
  const canStream = useMemo(
    () => current?.outputGroups.some((g) => g.streamTargets.length > 0) ?? false,
    [current?.outputGroups]
  );

  // Memoize streaming handlers to prevent child component re-renders
  // These functions are passed to Button onClick, and recreating them causes Button to re-render
  const handleStartStreaming = useCallback(async () => {
    if (!current) return;

    setIsValidating(true);

    try {
      const result = await validateStreamConfig(current, {
        checkFfmpeg: true,
        checkEnabledTargetsOnly: false,
      });

      if (!result.valid) {
        displayValidationIssues(result.issues, toast);
        setIsValidating(false);
        return;
      }

      const incomingUrl = getIncomingUrl(current);
      if (!incomingUrl) {
        toast.error(t('errors.noIncomingUrl'));
        setIsValidating(false);
        return;
      }

      await startAllGroups(current.outputGroups, incomingUrl);
      toast.success(t('toast.streamStarted'));
    } catch (err) {
      console.error('[Stream] startAllGroups failed:', err);
      toast.error(`Failed to start stream: ${err instanceof Error ? err.message : String(err)}`);
    } finally {
      setIsValidating(false);
    }
  }, [current, startAllGroups, t]);

  const handleStopStreaming = useCallback(async () => {
    await stopAllGroups();
    toast.success(t('toast.streamStopped'));
  }, [stopAllGroups, t]);

  // Count active streams
  const activeStreamCount = activeGroups.size;

  if (loading) {
    return (
      <div className="flex items-center justify-center h-full">
        <div className="text-muted">{t('common.loading')}</div>
      </div>
    );
  }

  if (error) {
    return (
      <Alert variant="error">
        <AlertTriangle className="w-4 h-4" />
        <span>{error}</span>
      </Alert>
    );
  }

  if (!current) {
    return (
      <div className="flex flex-col items-center justify-center h-full gap-4">
        <Plus className="w-12 h-12 text-muted" />
        <p className="text-muted">{t('common.selectProfile')}</p>
        <p className="text-sm text-muted">{t('profiles.createFirstProfile')}</p>
      </div>
    );
  }

  return (
    <div className="flex flex-col h-full gap-4">
      {/* Transition overlay for fadeToColor transitions */}
      <TransitionOverlay />

      {/* Top bar with Go Live button */}
      <div className="flex items-center justify-between gap-4">
        <div className="min-w-0 flex-1">
          <h1 className="text-xl font-semibold truncate" title={current.name}>{current.name}</h1>
          <p className="text-sm text-muted">
            {current.sources.length} {t('stream.sources', { defaultValue: 'sources' })}, {current.scenes.length} {t('stream.scenes', { defaultValue: 'scenes' })}
            {activeStreamCount > 0 && (
              <span className="ml-2 text-green-500">
                • {activeStreamCount} {t('stream.activeStreams', { defaultValue: 'active' })}
              </span>
            )}
          </p>
        </div>
        <div className="flex items-center gap-2 flex-shrink-0">
          {/* Studio Mode toggle - uses useTransition for non-blocking UI update */}
          <button
            onClick={handleToggleStudioMode}
            disabled={isStudioTransitioning}
            className={cn(
              'flex items-center gap-2 px-3 py-1.5 rounded-lg text-sm font-medium transition-colors',
              studioEnabled
                ? 'bg-[var(--primary)] text-white'
                : 'bg-[var(--bg-elevated)] text-[var(--text-secondary)] hover:bg-[var(--bg-hover)]',
              isStudioTransitioning && 'opacity-70'
            )}
            title={t('stream.studioMode', { defaultValue: 'Studio Mode' })}
          >
            <LayoutGrid className="w-4 h-4" />
            <span className="hidden sm:inline">{t('stream.studio', { defaultValue: 'Studio' })}</span>
          </button>

          {/* Recording button */}
          <RecordingButton />

          {/* Replay Buffer button */}
          <ReplayBufferButton />

          {/* Multiview toggle - uses useTransition for non-blocking UI update */}
          <button
            onClick={handleToggleMultiview}
            disabled={isMultiviewTransitioning}
            className={cn(
              'flex items-center gap-2 px-3 py-1.5 rounded-lg text-sm font-medium transition-colors',
              showMultiview
                ? 'bg-[var(--primary)] text-white'
                : 'bg-[var(--bg-elevated)] text-[var(--text-secondary)] hover:bg-[var(--bg-hover)]',
              isMultiviewTransitioning && 'opacity-70'
            )}
            title={t('stream.multiview', { defaultValue: 'Multiview (Ctrl+M)' })}
          >
            <Grid3X3 className="w-4 h-4" />
          </button>

          {isStreaming ? (
            <Button variant="destructive" onClick={handleStopStreaming}>
              <Square className="w-4 h-4 mr-2" />
              {t('streams.stopStreaming')}
            </Button>
          ) : (
            <Button
              variant="primary"
              onClick={handleStartStreaming}
              disabled={isValidating || !canStream}
              title={
                !canStream
                  ? t('stream.noTargetsConfigured', { defaultValue: 'Configure output targets in profile settings before streaming' })
                  : undefined
              }
            >
              <Play className="w-4 h-4 mr-2" />
              {isValidating ? t('streams.validating') : t('stream.goLive', { defaultValue: 'Go Live' })}
            </Button>
          )}
        </div>
      </div>

      {/* Main content area */}
      {studioEnabled ? (
        // Studio Mode: Dual-pane layout
        <div className="flex flex-1 gap-2 min-h-0">
          {/* Sources panel (left) */}
          <div className="w-56 lg:w-64 flex-shrink-0">
            <SourcesPanel profile={current} activeScene={activeScene} />
          </div>

          {/* Studio Mode Layout (center) - Lazy loaded */}
          <Suspense fallback={<StudioLayoutSkeleton />}>
            <StudioModeLayout
              profile={current}
              sources={current.sources}
              selectedLayerId={selectedLayerId}
              onSelectLayer={selectLayer}
            />
          </Suspense>

          {/* Properties panel (right) - Lazy loaded */}
          <div className="w-56 lg:w-64 flex-shrink-0">
            <Suspense fallback={<PanelSkeleton />}>
              <PropertiesPanel
                profile={current}
                scene={activeScene}
                layer={selectedLayer}
                source={selectedLayer ? current.sources.find((s) => s.id === selectedLayer.sourceId) : undefined}
              />
            </Suspense>
          </div>
        </div>
      ) : (
        // Normal Mode: Single canvas
        <div className="flex flex-1 gap-4 min-h-0">
          {/* Sources panel (left) */}
          <div className="w-56 lg:w-64 flex-shrink-0">
            <SourcesPanel profile={current} activeScene={activeScene} />
          </div>

          {/* Scene canvas (center) */}
          <div className="flex-1 min-w-0">
            <SceneCanvas
              scene={activeScene}
              sources={current.sources}
              scenes={current.scenes}
              selectedLayerId={selectedLayerId}
              onSelectLayer={selectLayer}
              profileName={current.name}
            />
          </div>

          {/* Properties panel (right) - Lazy loaded */}
          <div className="w-56 lg:w-64 flex-shrink-0">
            <Suspense fallback={<PanelSkeleton />}>
              <PropertiesPanel
                profile={current}
                scene={activeScene}
                layer={selectedLayer}
                source={selectedLayer ? current.sources.find((s) => s.id === selectedLayer.sourceId) : undefined}
              />
            </Suspense>
          </div>
        </div>
      )}

      {/* Scene bar */}
      <SceneBar
        profile={current}
        activeSceneId={current.activeSceneId}
      />

      {/* Audio section (bottom) - Unified mixer with built-in VU meters */}
      <AudioMixerPanel
        profile={current}
        scene={activeScene}
      />

      {/* Multiview panel (overlay) - Lazy loaded */}
      {showMultiview && (
        <Suspense fallback={null}>
          <MultiviewPanel
            profile={current}
            onClose={() => setShowMultiview(false)}
          />
        </Suspense>
      )}
    </div>
  );
}

export default Stream;
