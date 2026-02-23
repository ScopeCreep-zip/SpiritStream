/**
 * Audio Mixer Panel
 * Unified audio mixer with combined VU meters and volume controls.
 *
 * OBS pattern: Audio config lives on Profile.sourceAudioConfigs (source-level),
 * not per-scene. The mixer renders one UnifiedChannelStrip per source that
 * has audio capabilities, reading volume/mute/solo from the source-level config.
 *
 * PERFORMANCE OPTIMIZATION:
 * Audio level data is not passed as props to channel strips.
 * Each UnifiedChannelStrip reads levels directly from a pure JS store
 * (audioLevelStore) in a RAF loop, bypassing React's render cycle.
 */
import React, { useState, useCallback, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { Plus, Volume2, Settings2 } from 'lucide-react';
import { Card, CardBody } from '@/components/ui/Card';
import { AddSourceModal } from '@/components/modals/AddSourceModal';
import { AdvancedAudioProperties } from '@/components/modals/AdvancedAudioProperties';
import { UnifiedChannelStrip } from './UnifiedChannelStrip';
import { AudioSourceStrip } from './AudioSourceStrip';
import { useAudioLevels, type CaptureStatus } from '@/hooks/useAudioLevels';
import type { Profile, Scene } from '@/types/profile';
import type { AudioFilter } from '@/types/source';
import { sourceHasAudio } from '@/types/source';
import { createDefaultSourceAudioConfig } from '@/types/source';
import { useSceneStore } from '@/stores/sceneStore';
import { useProfileStore } from '@/stores/profileStore';
import { toast } from '@/hooks/useToast';
import { useShallow } from 'zustand/shallow';

interface AudioMixerPanelProps {
  profile: Profile;
  scene?: Scene;
  captureStatus?: CaptureStatus;
}

export const AudioMixerPanel = React.memo(function AudioMixerPanel({ profile, scene, captureStatus: captureStatusProp }: AudioMixerPanelProps) {
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
  const { setSourceAudioConfig, updateCurrentMasterVolume, updateCurrentMasterMuted } = useProfileStore(
    useShallow(s => ({
      setSourceAudioConfig: s.setSourceAudioConfig,
      updateCurrentMasterVolume: s.updateCurrentMasterVolume,
      updateCurrentMasterMuted: s.updateCurrentMasterMuted
    }))
  );
  const [showAddModal, setShowAddModal] = useState(false);
  const [showAdvancedAudio, setShowAdvancedAudio] = useState(false);

  // Get connection status and health status (captureStatus comes via prop from Stream.tsx)
  const { isConnected, isInitializing, healthStatus } = useAudioLevels();
  const captureStatus = captureStatusProp ?? {};

  // Source-level audio configs from profile (OBS pattern)
  const sourceAudioConfigs = profile.sourceAudioConfigs ?? {};

  // Filter to sources that have audio capabilities
  const audioSources = useMemo(() => {
    return profile.sources.filter(s => sourceHasAudio(s));
  }, [profile.sources]);

  // Memoized source name map — O(1) lookup instead of O(n) .find() per source
  const sourceNameMap = useMemo(() => {
    const map = new Map<string, string>();
    for (const s of profile.sources) {
      map.set(s.id, s.name);
    }
    return map;
  }, [profile.sources]);

  const defaultSourceName = t('stream.unknownSource', { defaultValue: 'Unknown' });

  // Memoized handlers — per-source audio config (no sceneId needed for per-source ops)
  const handleVolumeChange = useCallback(async (sourceId: string, volume: number) => {
    try {
      await setTrackVolume(profile.name, scene?.id ?? '', sourceId, volume);
      setSourceAudioConfig(sourceId, { volume });
    } catch (err) {
      toast.error(t('stream.volumeFailed', { error: err instanceof Error ? err.message : String(err), defaultValue: `Failed to set volume: ${err instanceof Error ? err.message : String(err)}` }));
    }
  }, [profile.name, scene?.id, setTrackVolume, setSourceAudioConfig, t]);

  const handleMuteToggle = useCallback(async (sourceId: string, muted: boolean) => {
    try {
      await setTrackMuted(profile.name, scene?.id ?? '', sourceId, muted);
      setSourceAudioConfig(sourceId, { muted });
    } catch (err) {
      toast.error(t('stream.muteFailed', { error: err instanceof Error ? err.message : String(err), defaultValue: `Failed to toggle mute: ${err instanceof Error ? err.message : String(err)}` }));
    }
  }, [profile.name, scene?.id, setTrackMuted, setSourceAudioConfig, t]);

  const handleSoloToggle = useCallback(async (sourceId: string, solo: boolean) => {
    try {
      await setTrackSolo(profile.name, scene?.id ?? '', sourceId, solo);
      setSourceAudioConfig(sourceId, { solo });
    } catch (err) {
      toast.error(t('stream.soloFailed', { error: err instanceof Error ? err.message : String(err), defaultValue: `Failed to toggle solo: ${err instanceof Error ? err.message : String(err)}` }));
    }
  }, [profile.name, scene?.id, setTrackSolo, setSourceAudioConfig, t]);

  const handleFiltersChange = useCallback(async (sourceId: string, filters: AudioFilter[]) => {
    try {
      setSourceAudioConfig(sourceId, { audioFilters: filters });
    } catch (err) {
      toast.error(t('stream.filtersFailed', { error: err instanceof Error ? err.message : String(err), defaultValue: `Failed to update filters: ${err instanceof Error ? err.message : String(err)}` }));
    }
  }, [setSourceAudioConfig, t]);

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
          <div className="flex items-center gap-1">
            <button
              type="button"
              className="w-6 h-6 flex items-center justify-center rounded bg-[var(--bg-sunken)] hover:bg-[var(--bg-elevated)] text-[var(--text-muted)] hover:text-[var(--text-secondary)] transition-colors"
              onClick={() => setShowAdvancedAudio(true)}
              title={t('audio.advancedProperties', { defaultValue: 'Advanced Audio Properties' })}
            >
              <Settings2 className="w-4 h-4" />
            </button>
            <button
              type="button"
              className="w-6 h-6 flex items-center justify-center rounded bg-[var(--bg-sunken)] hover:bg-[var(--bg-elevated)] text-[var(--text-muted)] hover:text-[var(--text-secondary)] transition-colors"
              onClick={() => setShowAddModal(true)}
              title={t('stream.addAudioSource', { defaultValue: 'Add Audio Source' })}
            >
              <Plus className="w-4 h-4" />
            </button>
          </div>
        </div>

        <div className="flex items-end overflow-x-auto pb-2 gap-6">
          {/* INPUT SECTION */}
          <div className="flex flex-col min-w-0">
            <div className="text-[10px] text-[var(--text-muted)] uppercase tracking-wide mb-2 pl-1">
              {t('stream.input', { defaultValue: 'Input' })}
            </div>
            <div className="flex items-end gap-6">
              {audioSources.length > 0 ? (
                audioSources.map((source) => {
                  const config = sourceAudioConfigs[source.id] ?? createDefaultSourceAudioConfig();
                  const trackCaptureStatus = captureStatus[source.id];
                  const isSourceHealthy = healthStatus[source.id] ?? true;

                  let captureError: string | undefined;
                  if (trackCaptureStatus && !trackCaptureStatus.success) {
                    captureError = trackCaptureStatus.message;
                  } else if (!isSourceHealthy) {
                    captureError = t('audio.noSignal', { defaultValue: 'No signal received' });
                  }

                  return (
                    <AudioSourceStrip
                      key={source.id}
                      sourceId={source.id}
                      config={config}
                      label={sourceNameMap.get(source.id) ?? defaultSourceName}
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

          {/* Divider before master */}
          {audioSources.length > 0 && (
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

      {/* Advanced Audio Properties Modal */}
      <AdvancedAudioProperties
        open={showAdvancedAudio}
        onClose={() => setShowAdvancedAudio(false)}
        profile={profile}
      />
    </Card>
  );
});
