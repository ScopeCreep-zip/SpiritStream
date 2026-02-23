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
import { useState, useEffect, useRef, useMemo, useCallback, lazy, Suspense, useTransition } from 'react';
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
import { useProfileStore } from '@/stores/profileStore';
import { useStreamStore } from '@/stores/streamStore';
import { useSceneStore } from '@/stores/sceneStore';
import { useStudioStore } from '@/stores/studioStore';
import { cn } from '@/lib/utils';
import { toast } from '@/hooks/useToast';
import { useHotkeys } from '@/hooks/useHotkeys';
import { getIncomingUrl, migrateProfileIfNeeded } from '@/types/profile';
import { sourceHasAudio } from '@/types/source';
import { createDefaultAudioTrack } from '@/types/scene';
import { validateStreamConfig, displayValidationIssues } from '@/lib/streamValidation';
import { api } from '@/lib/backend/httpApi';
import { events } from '@/lib/backend';
import { useAudioLevels } from '@/hooks/useAudioLevels';
import { useAppVisibility } from '@/hooks/useAppVisibility';
import { PanelSkeleton, StudioLayoutSkeleton } from '@/components/stream/Skeletons';

// Lazy-loaded components for code splitting
// These components are heavier and not needed on initial render
const propertiesPanelImport = () => import('@/components/stream/PropertiesPanel').then(m => ({ default: m.PropertiesPanel }));
const studioModeImport = () => import('@/components/stream/StudioModeLayout').then(m => ({ default: m.StudioModeLayout }));
const multiviewImport = () => import('@/components/stream/MultiviewPanel').then(m => ({ default: m.MultiviewPanel }));

const PropertiesPanel = lazy(propertiesPanelImport);
const StudioModeLayout = lazy(studioModeImport);
const MultiviewPanel = lazy(multiviewImport);

// Preload commonly-used chunks after initial render to avoid skeleton flashes.
// This runs once on module load and populates the browser's module cache.
if (typeof window !== 'undefined') {
  const preload = () => { propertiesPanelImport(); studioModeImport(); };
  if (typeof requestIdleCallback === 'function') {
    requestIdleCallback(preload);
  } else {
    setTimeout(preload, 100);
  }
}


export function Stream() {
  const { t } = useTranslation();

  // Use useShallow to reduce re-renders by doing shallow comparison of the selected state
  // This is a 2026 Zustand best practice - previously 10+ separate selectors caused excessive re-renders
  const { current, loading, error, updateProfile, saveProfile, setCurrentAudioTracks } = useProfileStore(
    useShallow((s) => ({
      current: s.current,
      loading: s.loading,
      error: s.error,
      updateProfile: s.updateProfile,
      saveProfile: s.saveProfile,
      setCurrentAudioTracks: s.setCurrentAudioTracks,
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

  const { enabled: studioEnabled, toggleStudioMode } = useStudioStore(
    useShallow((s) => ({
      enabled: s.enabled,
      toggleStudioMode: s.toggleStudioMode,
    }))
  );

  // Activate global hotkeys for the Stream view
  useHotkeys();

  // Notify backend when tab visibility changes to throttle preview encoding
  useAppVisibility();

  // Subscribe to thermal state changes from backend — Promise chain for StrictMode safety
  useEffect(() => {
    const subPromise = events.on<{ state: string; throttled: boolean }>('thermal_state_changed', (payload) => {
      if (payload.throttled) {
        toast.info(`System thermal pressure: ${payload.state}. Preview quality reduced.`);
      }
    });
    return () => { subPromise.then((unsub) => unsub()); };
  }, []);

  // Get setCaptureStatus + captureStatus from the audio levels hook (single source of truth)
  const { setCaptureStatus, captureStatus } = useAudioLevels();

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

  // Refs for migration and monitor dedup
  const migratedProfileRef = useRef<string>('');
  const migratedScenesRef = useRef<Set<string>>(new Set());
  const lastMonitorKeyRef = useRef<string>('');

  // Subscribe to device hotplug events — toast on disconnect, re-trigger capture on reconnect
  useEffect(() => {
    const disconnectSub = events.on<{ deviceName: string; type: string }>('device_disconnected', (payload) => {
      toast.info(t('stream.deviceDisconnected', {
        name: payload.deviceName,
        defaultValue: `Audio device disconnected: ${payload.deviceName}`,
      }));
    });
    const reconnectSub = events.on<{ deviceName: string; type: string }>('device_reconnected', (payload) => {
      toast.info(t('stream.deviceReconnected', {
        name: payload.deviceName,
        defaultValue: `Audio device reconnected: ${payload.deviceName}`,
      }));
      // Reset monitor key to force re-trigger of audio capture effect
      lastMonitorKeyRef.current = '';
    });
    return () => {
      disconnectSub.then((unsub) => unsub());
      reconnectSub.then((unsub) => unsub());
    };
  }, [t]);

  // Migrate profile if needed (runs once per profile load, not on every current change)
  useEffect(() => {
    if (!current) return;
    if (migratedProfileRef.current === current.name) return;
    migratedProfileRef.current = current.name;

    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    if (current.sources.length === 0 && ((current as any).input || current.scenes.length === 0)) {
      const migrated = migrateProfileIfNeeded(current);
      if (migrated !== current) {
        updateProfile(migrated).catch((err) => {
          console.error('[Stream] Profile migration failed:', err);
          toast.error(t('errors.profileMigrationFailed', {
            defaultValue: 'Failed to save migrated profile'
          }));
        });
      }
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [current?.name, current?.sources.length, updateProfile, t]);

  // Get active scene — memoized to prevent new reference on every parent render
  // Without this, every profile property change (layer transforms, visibility, etc.)
  // would cascade a new scene reference to AudioMixerPanel, PropertiesPanel, etc.
  const activeScene = useMemo(
    () => current?.scenes.find((s) => s.id === current.activeSceneId),
    [current?.scenes, current?.activeSceneId]
  );

  const selectedLayer = useMemo(
    () => activeScene?.layers.find((l) => l.id === selectedLayerId),
    [activeScene?.layers, selectedLayerId]
  );

  // Memoize source lookup for PropertiesPanel to avoid inline .find() in JSX
  const selectedSource = useMemo(
    () => selectedLayer ? current?.sources.find((s) => s.id === selectedLayer.sourceId) : undefined,
    [selectedLayer?.sourceId, current?.sources]
  );

  // Create stable string key from track source IDs to prevent infinite loop
  // (array reference comparison always fails, causing constant re-renders)
  const trackSourceIdsKey = useMemo(
    () => (activeScene?.audioMixer.tracks ?? []).map((t) => t.sourceId).join(',') ?? '',
    [activeScene?.audioMixer.tracks]
  );

  // Combined effect: audio track migration + monitor source sync
  // Merging these prevents the race where setMonitorSources fires before migration completes
  useEffect(() => {
    if (!activeScene || !current) {
      // No scene selected, clear audio monitoring
      if (lastMonitorKeyRef.current !== '') {
        lastMonitorKeyRef.current = '';
        api.audio.setMonitorSources([]).then(() => {
          setCaptureStatus({});
        }).catch(console.error);
      }
      return;
    }

    // Step 1: Migrate missing audio tracks (if not already done for this scene)
    let tracks = activeScene.audioMixer.tracks ?? [];
    let addedAny = false;

    if (!migratedScenesRef.current.has(activeScene.id)) {
      const newTracks = [...tracks];
      for (const layer of activeScene.layers) {
        const source = current.sources.find(s => s.id === layer.sourceId);
        if (source && sourceHasAudio(source) && !newTracks.some(t => t.sourceId === source.id)) {
          newTracks.push(createDefaultAudioTrack(source.id));
          addedAny = true;
        }
      }

      // Audio-only sources (audioDevice) aren't placed in scene layers but should
      // appear in every scene's mixer (matches OBS behavior for global audio sources)
      for (const source of current.sources) {
        if (source.type === 'audioDevice' && !newTracks.some(t => t.sourceId === source.id)) {
          newTracks.push(createDefaultAudioTrack(source.id));
          addedAny = true;
        }
      }

      migratedScenesRef.current.add(activeScene.id);

      if (addedAny) {
        tracks = newTracks;
        setCurrentAudioTracks(activeScene.id, newTracks);
        setTimeout(() => saveProfile(), 0);
      }
    }

    // Step 2: Sync monitor sources with backend (skip if unchanged)
    const monitorKey = `${current.name}|${tracks.map(t => t.sourceId).join(',')}`;
    if (monitorKey === lastMonitorKeyRef.current) return;
    lastMonitorKeyRef.current = monitorKey;

    const sourceIds = tracks.map(t => t.sourceId);
    api.audio.setMonitorSources(sourceIds, current.name).then((result) => {
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
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [activeScene?.id, trackSourceIdsKey, activeScene?.layers.length, current?.sources, current?.name, setCaptureStatus, setCurrentAudioTracks, saveProfile]);

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
      <div className="flex flex-col h-full gap-4">
        {/* Title bar skeleton */}
        <div className="flex items-center justify-between gap-4 animate-pulse">
          <div className="flex flex-col gap-1">
            <div className="h-5 w-40 bg-[var(--bg-elevated)] rounded" />
            <div className="h-3 w-60 bg-[var(--bg-elevated)] rounded" />
          </div>
          <div className="h-9 w-24 bg-[var(--bg-elevated)] rounded" />
        </div>
        {/* Main 3-column skeleton */}
        <div className="flex flex-1 gap-4 min-h-0">
          <div className="w-56 lg:w-64 flex-shrink-0"><PanelSkeleton /></div>
          <div className="flex-1 min-w-0 bg-[var(--bg-surface)] rounded-lg animate-pulse" />
          <div className="w-56 lg:w-64 flex-shrink-0"><PanelSkeleton /></div>
        </div>
        {/* Scene tabs skeleton */}
        <div className="h-10 bg-[var(--bg-surface)] rounded-lg animate-pulse" />
        {/* Audio mixer skeleton */}
        <div className="h-[260px] bg-[var(--bg-surface)] rounded-lg animate-pulse" />
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
              <span className="ml-2 text-[var(--success)]">
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
                source={selectedSource}
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
                source={selectedSource}
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
        captureStatus={captureStatus}
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
