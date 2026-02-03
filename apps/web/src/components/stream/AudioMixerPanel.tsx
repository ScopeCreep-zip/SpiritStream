/**
 * Audio Mixer Panel
 * Unified audio mixer with combined VU meters and volume controls
 *
 * PERFORMANCE OPTIMIZATION:
 * Audio level data is no longer passed as props to channel strips.
 * Each UnifiedChannelStrip reads levels directly from a pure JS store
 * (audioLevelStore) in a RAF loop, bypassing React's render cycle.
 * This eliminates ~30 re-renders per second across all channel strips.
 */
import { useState, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { Plus, Volume2 } from 'lucide-react';
import { Card, CardBody } from '@/components/ui/Card';
import { AddSourceModal } from '@/components/modals/AddSourceModal';
import { UnifiedChannelStrip } from './UnifiedChannelStrip';
import { useAudioLevels } from '@/hooks/useAudioLevels';
import type { Profile, Scene } from '@/types/profile';
import type { AudioFilter } from '@/types/source';
import { useSceneStore } from '@/stores/sceneStore';
import { useProfileStore } from '@/stores/profileStore';
import { toast } from '@/hooks/useToast';

interface AudioMixerPanelProps {
  profile: Profile;
  scene?: Scene;
}

// NOTE: DEFAULT_LEVEL removed - level data is read directly from audioLevelStore
// by each UnifiedChannelStrip in its RAF loop, not passed through props

export function AudioMixerPanel({ profile, scene }: AudioMixerPanelProps) {
  const { t } = useTranslation();
  const { setTrackVolume, setTrackMuted, setTrackSolo, setMasterVolume, setMasterMuted } = useSceneStore();
  const { updateCurrentAudioTrack, updateCurrentMasterVolume, updateCurrentMasterMuted } = useProfileStore();
  const [showAddModal, setShowAddModal] = useState(false);

  // Get connection status, capture status, and health status
  // NOTE: `levels` removed - channel strips read directly from audioLevelStore
  const { isConnected, isInitializing, healthStatus, captureStatus } = useAudioLevels();

  // Memoize source name lookup function
  const getSourceName = useCallback((sourceId: string) => {
    return profile.sources.find((s) => s.id === sourceId)?.name ?? t('stream.unknownSource', { defaultValue: 'Unknown' });
  }, [profile.sources, t]);

  // Memoized handlers that use local state updates instead of reloading profile
  const handleVolumeChange = useCallback(async (sourceId: string, volume: number) => {
    if (!scene) return;
    try {
      await setTrackVolume(profile.name, scene.id, sourceId, volume);
      updateCurrentAudioTrack(scene.id, sourceId, { volume });
    } catch (err) {
      toast.error(t('stream.volumeFailed', { error: err instanceof Error ? err.message : String(err), defaultValue: `Failed to set volume: ${err instanceof Error ? err.message : String(err)}` }));
    }
  }, [profile.name, scene, setTrackVolume, updateCurrentAudioTrack, t]);

  const handleMuteToggle = useCallback(async (sourceId: string, muted: boolean) => {
    if (!scene) return;
    try {
      await setTrackMuted(profile.name, scene.id, sourceId, muted);
      updateCurrentAudioTrack(scene.id, sourceId, { muted });
    } catch (err) {
      toast.error(t('stream.muteFailed', { error: err instanceof Error ? err.message : String(err), defaultValue: `Failed to toggle mute: ${err instanceof Error ? err.message : String(err)}` }));
    }
  }, [profile.name, scene, setTrackMuted, updateCurrentAudioTrack, t]);

  const handleSoloToggle = useCallback(async (sourceId: string, solo: boolean) => {
    if (!scene) return;
    try {
      await setTrackSolo(profile.name, scene.id, sourceId, solo);
      updateCurrentAudioTrack(scene.id, sourceId, { solo });
    } catch (err) {
      toast.error(t('stream.soloFailed', { error: err instanceof Error ? err.message : String(err), defaultValue: `Failed to toggle solo: ${err instanceof Error ? err.message : String(err)}` }));
    }
  }, [profile.name, scene, setTrackSolo, updateCurrentAudioTrack, t]);

  const handleFiltersChange = useCallback(async (sourceId: string, filters: AudioFilter[]) => {
    if (!scene) return;
    try {
      updateCurrentAudioTrack(scene.id, sourceId, { audioFilters: filters });
    } catch (err) {
      toast.error(t('stream.filtersFailed', { error: err instanceof Error ? err.message : String(err), defaultValue: `Failed to update filters: ${err instanceof Error ? err.message : String(err)}` }));
    }
  }, [scene, updateCurrentAudioTrack, t]);

  const handleMasterVolumeChange = useCallback(async (volume: number) => {
    if (!scene) return;
    try {
      await setMasterVolume(profile.name, scene.id, volume);
      updateCurrentMasterVolume(scene.id, volume);
    } catch (err) {
      toast.error(t('stream.masterVolumeFailed', { error: err instanceof Error ? err.message : String(err), defaultValue: `Failed to set master volume: ${err instanceof Error ? err.message : String(err)}` }));
    }
  }, [profile.name, scene, setMasterVolume, updateCurrentMasterVolume, t]);

  const handleMasterMuteToggle = useCallback(async (muted: boolean) => {
    if (!scene) return;
    try {
      await setMasterMuted(profile.name, scene.id, muted);
      updateCurrentMasterMuted(scene.id, muted);
    } catch (err) {
      toast.error(t('stream.masterMuteFailed', { error: err instanceof Error ? err.message : String(err), defaultValue: `Failed to toggle master mute: ${err instanceof Error ? err.message : String(err)}` }));
    }
  }, [profile.name, scene, setMasterMuted, updateCurrentMasterMuted, t]);

  // NOTE: getTrackLevel and getMasterLevel functions removed
  // UnifiedChannelStrip now reads levels directly from audioLevelStore
  // in its RAF loop, bypassing React's render cycle entirely

  if (!scene) {
    return (
      <Card>
        <CardBody className="py-4 text-center">
          <p className="text-muted text-sm">{t('stream.noSceneSelected', { defaultValue: 'No scene selected' })}</p>
        </CardBody>
      </Card>
    );
  }

  return (
    <Card>
      <CardBody style={{ padding: '12px 16px' }}>
        {/* Header */}
        <div className="flex items-center justify-between mb-3">
          <div className="flex items-center gap-2">
            <Volume2 className="w-4 h-4 text-[var(--text-muted)]" />
            <h4 className="text-sm font-medium text-[var(--text-secondary)]">
              {t('stream.audioMixer', { defaultValue: 'Audio Mixer' })}
            </h4>
            {isInitializing && (
              <span className="text-[10px] text-blue-400 px-1.5 py-0.5 rounded bg-blue-500/10 animate-pulse">
                {t('stream.audioInitializing', { defaultValue: 'initializing...' })}
              </span>
            )}
            {!isConnected && !isInitializing && (
              <span className="text-[10px] text-yellow-500 px-1.5 py-0.5 rounded bg-yellow-500/10">
                {t('stream.audioMonitorDisconnected', { defaultValue: 'disconnected' })}
              </span>
            )}
          </div>
          <button
            type="button"
            className="w-6 h-6 flex items-center justify-center rounded bg-[var(--bg-sunken)] hover:bg-[var(--bg-elevated)] text-[var(--text-muted)] hover:text-[var(--text-secondary)] transition-colors"
            onClick={() => setShowAddModal(true)}
            title={t('stream.addAudioSource', { defaultValue: 'Add Audio Source' })}
          >
            <Plus className="w-4 h-4" />
          </button>
        </div>

        <div className="flex items-end overflow-x-auto pb-2 gap-6">
          {/* INPUT SECTION */}
          <div className="flex flex-col min-w-0">
            <div className="text-[10px] text-[var(--text-muted)] uppercase tracking-wide mb-2 pl-1">
              {t('stream.input', { defaultValue: 'Input' })}
            </div>
            <div className="flex items-end gap-6">
              {scene.audioMixer.tracks.length > 0 ? (
                scene.audioMixer.tracks.map((track) => {
                  const trackCaptureStatus = captureStatus[track.sourceId];
                  const isSourceHealthy = healthStatus[track.sourceId] ?? true; // assume healthy if not tracked yet

                  // Determine capture error message
                  // Priority: capture failure > unhealthy (no data)
                  let captureError: string | undefined;
                  if (trackCaptureStatus && !trackCaptureStatus.success) {
                    captureError = trackCaptureStatus.message;
                  } else if (!isSourceHealthy) {
                    captureError = t('audio.noSignal', { defaultValue: 'No signal received' });
                  }

                  return (
                    <UnifiedChannelStrip
                      key={track.sourceId}
                      trackId={track.sourceId}
                      label={getSourceName(track.sourceId)}
                      // NOTE: Level props removed - component reads from audioLevelStore
                      volume={track.volume}
                      muted={track.muted}
                      solo={track.solo}
                      filters={track.audioFilters || []}
                      availableSources={profile.sources}
                      captureError={captureError}
                      onVolumeChange={(v) => handleVolumeChange(track.sourceId, v)}
                      onMuteToggle={(m) => handleMuteToggle(track.sourceId, m)}
                      onSoloToggle={(s) => handleSoloToggle(track.sourceId, s)}
                      onFiltersChange={(f) => handleFiltersChange(track.sourceId, f)}
                    />
                  );
                })
              ) : (
                <div className="flex items-center justify-center h-[200px] px-6">
                  <p className="text-[var(--text-muted)] text-xs text-center">
                    {t('stream.noAudioTracks', { defaultValue: 'No audio tracks' })}
                  </p>
                </div>
              )}
            </div>
          </div>

          {/* Divider */}
          {scene.audioMixer.tracks.length > 0 && (
            <div className="flex flex-col mx-2 self-stretch">
              <div className="flex-1 w-px bg-[var(--border-default)]" style={{ marginTop: 20 }} />
            </div>
          )}

          {/* OUTPUT SECTION - Master (pushed to far right) */}
          <div className="flex flex-col ml-auto">
            <div className="text-[10px] text-[var(--text-muted)] uppercase tracking-wide mb-2 pl-1">
              {t('stream.output', { defaultValue: 'Output' })}
            </div>
            <div className="flex items-end">
              <UnifiedChannelStrip
                label={t('stream.master', { defaultValue: 'Master' })}
                // NOTE: Level props removed - component reads from audioLevelStore
                volume={scene.audioMixer.masterVolume}
                muted={scene.audioMixer.masterMuted ?? false}
                solo={false}
                isMaster
                onVolumeChange={handleMasterVolumeChange}
                onMuteToggle={handleMasterMuteToggle}
                onSoloToggle={() => {}}
              />
            </div>
          </div>
        </div>
      </CardBody>

      {/* Add Audio Source Modal */}
      <AddSourceModal
        open={showAddModal}
        onClose={() => setShowAddModal(false)}
        profileName={profile.name}
        filterType="audioDevice"
      />
    </Card>
  );
}
