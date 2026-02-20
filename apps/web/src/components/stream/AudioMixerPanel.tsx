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
import React, { useState, useCallback, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { Plus, Volume2 } from 'lucide-react';
import { Card, CardBody } from '@/components/ui/Card';
import { AddSourceModal } from '@/components/modals/AddSourceModal';
import { UnifiedChannelStrip } from './UnifiedChannelStrip';
import { useAudioLevels } from '@/hooks/useAudioLevels';
import type { Profile, Scene, AudioTrack } from '@/types/profile';
import type { AudioFilter, Source } from '@/types/source';
import { useSceneStore } from '@/stores/sceneStore';
import { useProfileStore } from '@/stores/profileStore';
import { toast } from '@/hooks/useToast';
import { useShallow } from 'zustand/shallow';

/**
 * Wrapper that creates stable handler references for a single audio track.
 * Without this, inline arrow functions in .map() create new references every render,
 * defeating React.memo on UnifiedChannelStrip and causing unnecessary re-renders.
 */
interface AudioTrackStripProps {
  track: AudioTrack;
  label: string;
  sources: Source[];
  captureError?: string;
  onVolumeChange: (sourceId: string, volume: number) => void;
  onMuteToggle: (sourceId: string, muted: boolean) => void;
  onSoloToggle: (sourceId: string, solo: boolean) => void;
  onFiltersChange: (sourceId: string, filters: AudioFilter[]) => void;
}

const AudioTrackStrip = React.memo(function AudioTrackStrip({
  track,
  label,
  sources,
  captureError,
  onVolumeChange,
  onMuteToggle,
  onSoloToggle,
  onFiltersChange,
}: AudioTrackStripProps) {
  // Stable per-track handlers — these only change when the parent handler changes,
  // NOT on every parent render (because track.sourceId is captured in useCallback)
  const handleVolume = useCallback(
    (v: number) => onVolumeChange(track.sourceId, v),
    [track.sourceId, onVolumeChange]
  );
  const handleMute = useCallback(
    (m: boolean) => onMuteToggle(track.sourceId, m),
    [track.sourceId, onMuteToggle]
  );
  const handleSolo = useCallback(
    (s: boolean) => onSoloToggle(track.sourceId, s),
    [track.sourceId, onSoloToggle]
  );
  const handleFilters = useCallback(
    (f: AudioFilter[]) => onFiltersChange(track.sourceId, f),
    [track.sourceId, onFiltersChange]
  );

  return (
    <UnifiedChannelStrip
      trackId={track.sourceId}
      label={label}
      volume={track.volume}
      muted={track.muted}
      solo={track.solo}
      filters={track.audioFilters || []}
      availableSources={sources}
      captureError={captureError}
      onVolumeChange={handleVolume}
      onMuteToggle={handleMute}
      onSoloToggle={handleSolo}
      onFiltersChange={handleFilters}
    />
  );
});

interface AudioMixerPanelProps {
  profile: Profile;
  scene?: Scene;
}

// NOTE: DEFAULT_LEVEL removed - level data is read directly from audioLevelStore
// by each UnifiedChannelStrip in its RAF loop, not passed through props

export const AudioMixerPanel = React.memo(function AudioMixerPanel({ profile, scene }: AudioMixerPanelProps) {
  const { t } = useTranslation();
  const { setTrackVolume, setTrackMuted, setTrackSolo, setMasterVolume, setMasterMuted } = useSceneStore(
    useShallow(s => ({
      setTrackVolume: s.setTrackVolume,
      setTrackMuted: s.setTrackMuted,
      setTrackSolo: s.setTrackSolo,
      setMasterVolume: s.setMasterVolume,
      setMasterMuted: s.setMasterMuted
    }))
  );
  const { updateCurrentAudioTrack, updateCurrentMasterVolume, updateCurrentMasterMuted } = useProfileStore(
    useShallow(s => ({
      updateCurrentAudioTrack: s.updateCurrentAudioTrack,
      updateCurrentMasterVolume: s.updateCurrentMasterVolume,
      updateCurrentMasterMuted: s.updateCurrentMasterMuted
    }))
  );
  const [showAddModal, setShowAddModal] = useState(false);

  // Get connection status, capture status, and health status
  // NOTE: `levels` removed - channel strips read directly from audioLevelStore
  const { isConnected, isInitializing, healthStatus, captureStatus } = useAudioLevels();

  // Memoized source name map — O(1) lookup instead of O(n) .find() per track
  const sourceNameMap = useMemo(() => {
    const map = new Map<string, string>();
    for (const s of profile.sources) {
      map.set(s.id, s.name);
    }
    return map;
  }, [profile.sources]);

  const defaultSourceName = t('stream.unknownSource', { defaultValue: 'Unknown' });

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
      <CardBody className="py-3 px-4">
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
                    <AudioTrackStrip
                      key={track.sourceId}
                      track={track}
                      label={sourceNameMap.get(track.sourceId) ?? defaultSourceName}
                      sources={profile.sources}
                      captureError={captureError}
                      onVolumeChange={handleVolumeChange}
                      onMuteToggle={handleMuteToggle}
                      onSoloToggle={handleSoloToggle}
                      onFiltersChange={handleFiltersChange}
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
              <div className="flex-1 w-px bg-[var(--border-default)] mt-5" />
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
});
