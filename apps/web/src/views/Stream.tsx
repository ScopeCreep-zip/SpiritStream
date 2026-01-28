/**
 * Stream Page
 * Multi-input streaming with scene composition
 */
import { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { Play, Square, AlertTriangle, Plus } from 'lucide-react';
import { Button } from '@/components/ui/Button';
import { Alert } from '@/components/ui/Alert';
import { SourcesPanel } from '@/components/stream/SourcesPanel';
import { SceneCanvas } from '@/components/stream/SceneCanvas';
import { PropertiesPanel } from '@/components/stream/PropertiesPanel';
import { SceneBar } from '@/components/stream/SceneBar';
import { AudioMixerPanel } from '@/components/stream/AudioMixerPanel';
import { useProfileStore } from '@/stores/profileStore';
import { useStreamStore } from '@/stores/streamStore';
import { useSceneStore } from '@/stores/sceneStore';
import { useSourceStore } from '@/stores/sourceStore';
import { toast } from '@/hooks/useToast';
import { getIncomingUrl, migrateProfileIfNeeded } from '@/types/profile';
import { validateStreamConfig, displayValidationIssues } from '@/lib/streamValidation';

export function Stream() {
  const { t } = useTranslation();
  const { current, loading, error, updateProfile, saveProfile } = useProfileStore();
  const { isStreaming, activeGroups, startAllGroups, stopAllGroups } = useStreamStore();
  const { selectedLayerId, selectLayer } = useSceneStore();
  const { discoverDevices } = useSourceStore();
  const [isValidating, setIsValidating] = useState(false);

  // Discover devices on mount
  useEffect(() => {
    discoverDevices();
  }, [discoverDevices]);

  // Migrate profile if needed (on first load)
  useEffect(() => {
    if (current && current.sources.length === 0 && (current.input || current.scenes.length === 0)) {
      const migrated = migrateProfileIfNeeded(current);
      if (migrated !== current) {
        updateProfile(migrated);
        saveProfile();
      }
    }
  }, [current, updateProfile, saveProfile]);

  // Get active scene
  const activeScene = current?.scenes.find((s) => s.id === current.activeSceneId);
  const selectedLayer = activeScene?.layers.find((l) => l.id === selectedLayerId);

  const handleStartStreaming = async () => {
    if (!current) return;

    setIsValidating(true);

    try {
      const result = await validateStreamConfig(current, {
        checkFfmpeg: true,
        checkEnabledTargetsOnly: false,
      });

      if (!result.valid) {
        displayValidationIssues(result.issues, toast);
        return;
      }

      const incomingUrl = getIncomingUrl(current);
      if (!incomingUrl) {
        toast.error(t('errors.noIncomingUrl'));
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
  };

  const handleStopStreaming = async () => {
    await stopAllGroups();
    toast.success(t('toast.streamStopped'));
  };

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
          {isStreaming ? (
            <Button variant="destructive" onClick={handleStopStreaming}>
              <Square className="w-4 h-4 mr-2" />
              {t('streams.stopStreaming')}
            </Button>
          ) : (
            <Button
              variant="primary"
              onClick={handleStartStreaming}
              disabled={isValidating || current.outputGroups.every((g) => g.streamTargets.length === 0)}
              title={
                current.outputGroups.every((g) => g.streamTargets.length === 0)
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
      <div className="flex flex-1 gap-4 min-h-0">
        {/* Sources panel (left) */}
        <div className="w-56 lg:w-64 flex-shrink-0">
          <SourcesPanel
            profile={current}
            activeScene={activeScene}
          />
        </div>

        {/* Scene canvas (center) */}
        <div className="flex-1 min-w-0">
          <SceneCanvas
            scene={activeScene}
            sources={current.sources}
            selectedLayerId={selectedLayerId}
            onSelectLayer={selectLayer}
            profileName={current.name}
          />
        </div>

        {/* Properties panel (right) */}
        <div className="w-56 lg:w-64 flex-shrink-0">
          <PropertiesPanel
            profile={current}
            scene={activeScene}
            layer={selectedLayer}
            source={selectedLayer ? current.sources.find((s) => s.id === selectedLayer.sourceId) : undefined}
          />
        </div>
      </div>

      {/* Scene bar */}
      <SceneBar
        profile={current}
        activeSceneId={current.activeSceneId}
      />

      {/* Audio mixer (bottom) */}
      <AudioMixerPanel
        profile={current}
        scene={activeScene}
      />
    </div>
  );
}

export default Stream;
